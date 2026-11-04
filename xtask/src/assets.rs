// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask assets`: every asset file has a row in `ATTRIBUTION.md`.
//!
//! An asset file is a picture, icon, font or sound (by extension, see [`EXTENSIONS`]) or anything under the top-level
//! `assets/` directory. A row is a table line whose first cell is a path in backticks; `<name>` in it stands for one
//! or more characters within one directory (`docs/images/<shot>.png`). Besides the directories every walk skips, the
//! gitignored local directories in [`LOCAL`] are not checked.

use std::fs;
use std::io::Write;
use std::path::{Component, Path};

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
    match rel.components().next() {
        Some(Component::Normal(name)) => name.to_str(),
        _ => None,
    }
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
    Ok(missing == 0)
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
