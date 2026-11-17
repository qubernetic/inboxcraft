// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask assets`: every asset file has a row in `ATTRIBUTION.md`.
//!
//! An asset file is a picture, icon, font or sound (by extension, see [`EXTENSIONS`]) or anything under the top-level
//! `assets/` directory. A row is a table line whose first cell is a path in backticks; `<name>` in it stands for one
//! or more characters within one directory (`docs/images/<shot>.png`). Besides the directories every walk skips, the
//! gitignored local directories in [`LOCAL`] are not checked.
//!
//! It also checks the files pinned in [`MANIFEST`]: every file under one of its `roots` has a `[[file]]` whose
//! `sha256` it matches, and every `[[file]]` exists. See docs/development.md, "Pinned asset files".

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{Error, walk};

/// File extensions of asset files, lower case.
pub(crate) const EXTENSIONS: &[&str] = &[
    "avif", "bmp", "flac", "gif", "icns", "ico", "jpeg", "jpg", "mp3", "oga", "ogg", "opus", "otf", "png", "svg", "tif", "tiff", "ttf", "wav",
    "webp", "woff", "woff2",
];

/// Gitignored top-level directories with local data or downloads, never committed.
const LOCAL: &[&str] = &["corpus", "craft-fonts", "craft-i18n", "dist", "log"];

/// The longest row path that is matched at all; longer ones are ignored.
pub(crate) const MAX_PATTERN: usize = 256;

fn top(rel: &Path) -> Option<&str> {
    rel.iter().next().and_then(|name| name.to_str())
}

/// Whether `rel` (relative to the workspace root) is an asset file.
pub(crate) fn is_asset(rel: &Path) -> bool {
    let by_extension = rel.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
    by_extension || (top(rel) == Some("assets") && rel.components().count() > 1)
}

/// The paths in the first cells of the table rows of `text`.
pub(crate) fn rows(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix('|'))
        .filter_map(|rest| rest.split_once('|'))
        .filter_map(|(cell, _)| cell.trim().strip_prefix('`')?.strip_suffix('`'))
        .filter(|path| !path.is_empty())
        .collect()
}

enum Token {
    Char(char),
    /// One or more characters other than `/`.
    Any,
}

fn tokens(pattern: &str) -> Vec<Token> {
    let mut found = Vec::new();
    let mut rest = pattern;
    while let Some(c) = rest.chars().next() {
        let placeholder = rest
            .strip_prefix('<')
            .and_then(|inner| inner.find('>').map(|end| (inner, end)))
            .filter(|(inner, end)| *end > 0 && !inner.get(..*end).unwrap_or("/").contains('/'));
        if let Some((inner, end)) = placeholder {
            found.push(Token::Any);
            rest = inner.get(end.saturating_add(1)..).unwrap_or_default();
        } else {
            found.push(Token::Char(c));
            rest = rest.get(c.len_utf8()..).unwrap_or_default();
        }
    }
    found
}

/// Whether the row path `pattern` names `path`. Runs in time proportional to the product of the two lengths.
pub(crate) fn matches(pattern: &str, path: &str) -> bool {
    if pattern.len() > MAX_PATTERN {
        return false;
    }
    let text: Vec<char> = path.chars().collect();
    // reach[j]: the tokens so far match the first j characters of the path.
    let mut reach = vec![false; text.len().saturating_add(1)];
    if let Some(first) = reach.first_mut() {
        *first = true;
    }
    for token in tokens(pattern) {
        let mut next = vec![false; reach.len()];
        let mut open = false;
        for (j, c) in text.iter().enumerate() {
            let before = reach.get(j).copied().unwrap_or(false);
            let matched = match token {
                Token::Char(want) => before && *c == want,
                Token::Any if *c == '/' => {
                    open = false;
                    false
                }
                Token::Any => {
                    open |= before;
                    open
                }
            };
            if let Some(slot) = next.get_mut(j.saturating_add(1)) {
                *slot = matched;
            }
        }
        reach = next;
    }
    reach.last().copied().unwrap_or(false)
}

/// Checks every asset file under `root` against `ATTRIBUTION.md`, reports to `out` and returns whether all have a row.
pub(crate) fn gate(root: &Path, out: &mut dyn Write) -> Result<bool, Error> {
    let path = root.join("ATTRIBUTION.md");
    let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
    let patterns = rows(&text);
    let files = walk::files(root, &|rel| is_asset(rel) && !top(rel).is_some_and(|dir| LOCAL.contains(&dir)))?;
    let mut missing = 0_usize;
    for rel in &files {
        let name = rel.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        if !patterns.iter().any(|pattern| matches(pattern, &name)) {
            missing = missing.saturating_add(1);
            writeln!(out, "assets: {name} has no row in ATTRIBUTION.md")?;
        }
    }
    if missing == 0 {
        writeln!(out, "assets: {} asset files, all have a row in ATTRIBUTION.md", files.len())?;
    } else {
        writeln!(out, "assets: {missing} of {} asset files have no row in ATTRIBUTION.md", files.len())?;
    }
    let manifest = read_manifest(root)?;
    let problems = pinned(root, &manifest)?;
    for line in &problems {
        writeln!(out, "{line}")?;
    }
    if problems.is_empty() {
        writeln!(out, "assets: {} pinned files, all match {MANIFEST}", manifest.pins.len())?;
    } else {
        writeln!(out, "assets: {} problems with the pinned files in {MANIFEST}", problems.len())?;
    }
    Ok(missing == 0 && problems.is_empty())
}

/// The manifest of pinned asset files, relative to the workspace root.
pub(crate) const MANIFEST: &str = ".config/assets.toml";

/// How many bytes of a pinned file are hashed at a time.
const CHUNK: usize = 64 * 1024;

/// A `[[file]]` of the manifest: a file and the sha256 it must have.
struct Pin {
    path: String,
    sha256: String,
}

/// The manifest: the folders it governs and the files it pins.
struct Manifest {
    roots: Vec<String>,
    pins: Vec<Pin>,
}

/// Whether `path` is a plain relative path inside the repository: `/`-separated names, none of them empty, `.` or
/// `..`, and no `\`.
fn inside(path: &str) -> bool {
    !path.is_empty() && !path.contains('\\') && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

fn string<'a>(table: &'a toml::Table, key: &str) -> Option<&'a str> {
    table.get(key).and_then(toml::Value::as_str)
}

/// The tables of the array of tables `key`, or `None` if it is something else.
fn tables<'a>(table: &'a toml::Table, key: &str) -> Option<Vec<&'a toml::Table>> {
    match table.get(key) {
        None => Some(Vec::new()),
        Some(value) => value.as_array()?.iter().map(toml::Value::as_table).collect(),
    }
}

/// Reads [`MANIFEST`] under `root`; a manifest that is missing or invalid is an error.
fn read_manifest(root: &Path) -> Result<Manifest, Error> {
    let path = root.join(MANIFEST);
    let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
    let error = |message: String| Error::Config { path: path.clone(), message };
    let table: toml::Table = text.parse().map_err(|e| error(format!("not valid TOML: {e}")))?;
    if let Some(key) = table.keys().find(|key| !["roots", "source", "file"].contains(&key.as_str())) {
        return Err(error(format!("unknown key `{key}`")));
    }
    let roots = table
        .get("roots")
        .and_then(toml::Value::as_array)
        .and_then(|list| list.iter().map(|value| value.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
        .ok_or_else(|| error("`roots` must be a list of folders".to_owned()))?;
    if let Some(bad) = roots.iter().find(|root| !inside(root)) {
        return Err(error(format!("root `{bad}` is not a relative path inside the repository")));
    }
    let mut sources = Vec::new();
    for source in tables(&table, "source").ok_or_else(|| error("`source` must be an array of tables".to_owned()))? {
        sources.push(string(source, "id").ok_or_else(|| error("a `[[source]]` has no `id`".to_owned()))?);
    }
    let mut pins: Vec<Pin> = Vec::new();
    for file in tables(&table, "file").ok_or_else(|| error("`file` must be an array of tables".to_owned()))? {
        if let Some(key) = file.keys().find(|key| !["path", "source", "from", "sha256"].contains(&key.as_str())) {
            return Err(error(format!("unknown key `{key}` in a `[[file]]`")));
        }
        let path = string(file, "path").ok_or_else(|| error("a `[[file]]` has no `path`".to_owned()))?;
        if !inside(path) {
            return Err(error(format!("`{path}` is not a relative path inside the repository")));
        }
        let sha256 = string(file, "sha256").ok_or_else(|| error(format!("`{path}` has no `sha256`")))?;
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(error(format!("the sha256 of `{path}` is not 64 hexadecimal digits")));
        }
        if let Some(source) = string(file, "source").filter(|source| !sources.contains(source)) {
            return Err(error(format!("`{path}` names the source `{source}`, which has no `[[source]]`")));
        }
        if pins.iter().any(|pin| pin.path == path) {
            return Err(error(format!("`{path}` is pinned twice")));
        }
        pins.push(Pin { path: path.to_owned(), sha256: sha256.to_ascii_lowercase() });
    }
    Ok(Manifest { roots, pins })
}

/// The sha256 of the file at `path` in lower-case hex, read [`CHUNK`] bytes at a time.
fn sha256(path: &Path) -> Result<String, Error> {
    let io_error = |source| Error::Io { path: path.to_path_buf(), source };
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; CHUNK];
    loop {
        let read = file.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(buffer.get(..read).unwrap_or_default());
    }
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Checks the files under the roots of `manifest` against its pins and returns one report line per problem: a file
/// with no pin, a file whose sha256 differs, a pin with no file.
fn pinned(root: &Path, manifest: &Manifest) -> Result<Vec<String>, Error> {
    let mut problems = Vec::new();
    for folder in &manifest.roots {
        let dir = root.join(folder);
        if !dir.is_dir() {
            continue;
        }
        for rel in walk::files(&dir, &|_| true)? {
            let name = std::iter::once(folder.clone())
                .chain(rel.components().map(|part| part.as_os_str().to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
                .join("/");
            if !manifest.pins.iter().any(|pin| pin.path == name) {
                problems.push(format!("assets: {name} is not pinned in {MANIFEST}"));
            }
        }
    }
    for pin in &manifest.pins {
        let path = root.join(&pin.path);
        if !path.is_file() {
            problems.push(format!("assets: {} is pinned but missing", pin.path));
        } else if sha256(&path)? != pin.sha256 {
            problems.push(format!("assets: {} differs from its pinned sha256", pin.path));
        }
    }
    Ok(problems)
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
