// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The font folders fontconfig's files name, read by the app itself and bounded, for [`crate::fonts`].
//!
//! Only what the search needs is taken from a file (fonts-conf(5)): `<dir>` and `<remap-dir>` (whose text is the
//! folder to read; `as-path` is only the name fontconfig's cache gives it), `<include>`, and the `<alias>` that names
//! the sans-serif family. Everything else is ignored. The files are hostile: only regular files are read (no device,
//! FIFO, socket or folder in their place), at most [`MAX_CONFIG`] bytes each and [`MAX_CONFIGS`] files in all, an
//! include goes at most [`MAX_INCLUDE_DEPTH`] deep, each file is read once (so include loops end), the XML parser
//! keeps at most [`MAX_NODES`] nodes, a file nested more than [`MAX_NESTING`] elements deep is passed over, and only
//! the top elements and their children are looked at.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// The largest configuration file read: 1 MiB.
pub(crate) const MAX_CONFIG: u64 = 1024 * 1024;

/// The most configuration files read in all.
pub(crate) const MAX_CONFIGS: usize = 256;

/// How deep includes go.
pub(crate) const MAX_INCLUDE_DEPTH: usize = 8;

/// The most entries of a folder of configuration files looked at.
const MAX_LISTED: usize = 10_000;

/// How deep elements may nest in a file; a file nested deeper is passed over.
pub(crate) const MAX_NESTING: usize = 64;

/// The most XML nodes one file may hold.
pub(crate) const MAX_NODES: u32 = 100_000;

/// Where relative and prefixed names in the files point.
#[derive(Debug, Clone, Default)]
pub(crate) struct Places {
    /// `HOME`, for names starting with `~`.
    pub(crate) home: Option<PathBuf>,
    /// `XDG_CONFIG_HOME`, for an include with `prefix="xdg"` (else `~/.config`).
    pub(crate) config_home: Option<PathBuf>,
    /// `XDG_DATA_HOME`, for a folder with `prefix="xdg"` (else `~/.local/share`).
    pub(crate) data_home: Option<PathBuf>,
    /// The working folder, for a relative folder name (`prefix="default"` or `"cwd"`).
    pub(crate) cwd: Option<PathBuf>,
}

impl Places {
    /// The places of this process.
    pub(crate) fn of_process() -> Self {
        let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
        let home = var("HOME");
        Self {
            config_home: var("XDG_CONFIG_HOME").or_else(|| home.as_ref().map(|home| home.join(".config"))),
            data_home: var("XDG_DATA_HOME").or_else(|| home.as_ref().map(|home| home.join(".local/share"))),
            cwd: std::env::current_dir().ok(),
            home,
        }
    }
}

/// What the files say.
#[derive(Debug, Default)]
pub(crate) struct Fontconfig {
    /// The font folders, in the order named.
    pub(crate) folders: Vec<PathBuf>,
    /// The family named first for sans-serif by the last alias that names one.
    pub(crate) sans: Option<String>,
    /// The files passed over: not regular files, too large, too many, too deep, not XML.
    pub(crate) skipped: Vec<PathBuf>,
}

/// Reads `files` (each a file or a folder of `*.conf` files) and what they include.
pub(crate) fn read(files: &[PathBuf], places: &Places) -> Fontconfig {
    let mut reading = Reading { places, seen: HashSet::new(), read: 0, found: Fontconfig::default() };
    for file in files {
        reading.path(file, 0);
    }
    reading.found
}

/// A reading of the files: what it has read, by real path, and how many.
struct Reading<'a> {
    places: &'a Places,
    seen: HashSet<PathBuf>,
    read: usize,
    found: Fontconfig,
}

impl Reading<'_> {
    /// Reads `path`, a file or a folder whose files start with a digit and end in `.conf` (in name order), at `depth`
    /// includes deep.
    fn path(&mut self, path: &Path, depth: usize) {
        if depth > MAX_INCLUDE_DEPTH {
            self.found.skipped.push(path.to_owned());
            return;
        }
        // The link itself, then what it points at: only folders and regular files go on.
        let (Ok(_), Ok(target)) = (std::fs::symlink_metadata(path), std::fs::metadata(path)) else { return };
        if target.is_dir() {
            let Ok(entries) = std::fs::read_dir(path) else { return };
            let mut names: Vec<PathBuf> = entries
                .take(MAX_LISTED)
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with(|c: char| c.is_ascii_digit()) && name.ends_with(".conf"))
                })
                .take(MAX_CONFIGS.saturating_add(1))
                .collect();
            names.sort();
            for name in names {
                if std::fs::metadata(&name).is_ok_and(|meta| meta.is_file()) {
                    self.file(&name, depth);
                } else {
                    self.found.skipped.push(name);
                }
            }
        } else if target.is_file() {
            self.file(path, depth);
        } else {
            // A device, a FIFO or a socket: opening one could block or never end.
            self.found.skipped.push(path.to_owned());
        }
    }

    /// Reads the regular file at `path`, once.
    fn file(&mut self, path: &Path, depth: usize) {
        let Ok(real) = std::fs::canonicalize(path) else { return };
        if !self.seen.insert(real.clone()) {
            return;
        }
        if self.read >= MAX_CONFIGS {
            self.found.skipped.push(real);
            return;
        }
        self.read = self.read.saturating_add(1);
        let Some(text) = read_text(&real) else {
            self.found.skipped.push(real);
            return;
        };
        if !nesting_within(&text, MAX_NESTING) {
            self.found.skipped.push(real);
            return;
        }
        let options = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: MAX_NODES };
        let Ok(document) = roxmltree::Document::parse_with_options(&text, options) else {
            self.found.skipped.push(real);
            return;
        };
        let root = document.root_element();
        if root.tag_name().name() != "fontconfig" {
            return;
        }
        let here = real.parent().map(Path::to_owned);
        for element in root.children().filter(roxmltree::Node::is_element) {
            match element.tag_name().name() {
                "dir" | "remap-dir" => {
                    if let Some(folder) = self.folder(element, here.as_deref()) {
                        self.found.folders.push(folder);
                    }
                }
                "include" => {
                    if let Some(include) = self.include(element, here.as_deref()) {
                        self.path(&include, depth.saturating_add(1));
                    }
                }
                "alias" => {
                    if let Some(sans) = sans_serif(element) {
                        self.found.sans = Some(sans);
                    }
                }
                _ => {}
            }
        }
    }

    /// The folder a `<dir>` or `<remap-dir>` names, in the file in folder `here`.
    fn folder(&self, element: roxmltree::Node<'_, '_>, here: Option<&Path>) -> Option<PathBuf> {
        let text = text_of(element)?;
        if let Some(rest) = text.strip_prefix('~') {
            return self.in_home(rest);
        }
        let base = match element.attribute("prefix") {
            Some("xdg") => self.places.data_home.clone()?,
            Some("relative") => here?.to_owned(),
            _ => self.places.cwd.clone().unwrap_or_default(),
        };
        Some(base.join(text))
    }

    /// The file or folder an `<include>` names, in the file in folder `here`; `ignore_missing` only quiets
    /// fontconfig's warning, and this reading warns about nothing.
    fn include(&self, element: roxmltree::Node<'_, '_>, here: Option<&Path>) -> Option<PathBuf> {
        let text = text_of(element)?;
        if let Some(rest) = text.strip_prefix('~') {
            return self.in_home(rest);
        }
        let base = match element.attribute("prefix") {
            Some("xdg") => self.places.config_home.clone()?,
            _ => here?.to_owned(),
        };
        Some(base.join(text))
    }

    /// `rest` (what follows a `~`) in the home folder.
    fn in_home(&self, rest: &str) -> Option<PathBuf> {
        self.places.home.as_ref().map(|home| home.join(rest.trim_start_matches('/')))
    }
}

/// Whether the elements of the XML `text` nest at most `max` deep, read before the XML parser sees it (which recurses
/// once per level and would overflow the stack on a deep file). A lexer of its own, without recursion: comments,
/// CDATA, processing instructions and declarations are passed over, quotes in a tag are honoured, and a file it cannot
/// follow to the end counts as too deep, as does one with an internal DTD (`<!DOCTYPE x [ ... ]>`), whose entities
/// could hide markup from this count.
fn nesting_within(text: &str, max: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth: usize = 0;
    let mut at = 0;
    while let Some(next) = bytes.get(at..).and_then(|rest| rest.iter().position(|byte| *byte == b'<')) {
        at = at.saturating_add(next);
        let rest = bytes.get(at..).unwrap_or_default();
        let end = if rest.starts_with(b"<!--") {
            find(rest, b"-->").map(|end| end.saturating_add(3))
        } else if rest.starts_with(b"<![CDATA[") {
            find(rest, b"]]>").map(|end| end.saturating_add(3))
        } else if rest.starts_with(b"<?") {
            find(rest, b"?>").map(|end| end.saturating_add(2))
        } else {
            let end = tag_end(rest);
            if rest.starts_with(b"<!") {
                // A declaration: no element. An internal DTD could declare entities that hide markup, and fontconfig's
                // files never have one.
                if end.and_then(|end| rest.get(..end)).is_none_or(|declaration| declaration.contains(&b'[')) {
                    return false;
                }
            } else if rest.starts_with(b"</") {
                depth = depth.saturating_sub(1);
            } else if let Some(end) = end
                && bytes.get(at.saturating_add(end).saturating_sub(2)) != Some(&b'/')
            {
                depth = depth.saturating_add(1);
                if depth > max {
                    return false;
                }
            }
            end
        };
        let Some(end) = end else { return false };
        at = at.saturating_add(end.max(1));
    }
    true
}

/// Where `needle` starts in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

/// Just past the `>` that ends the tag at the start of `rest`, with quotes and a declaration's `[...]` honoured.
fn tag_end(rest: &[u8]) -> Option<usize> {
    let mut quote: Option<u8> = None;
    let mut brackets: usize = 0;
    for (at, byte) in rest.iter().enumerate().skip(1) {
        match (quote, *byte) {
            (Some(open), byte) if byte == open => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(*byte),
            (None, b'[') => brackets = brackets.saturating_add(1),
            (None, b']') => brackets = brackets.saturating_sub(1),
            (None, b'>') if brackets == 0 => return Some(at.saturating_add(1)),
            _ => {}
        }
    }
    None
}

/// The trimmed text of `element`, if it has any.
fn text_of(element: roxmltree::Node<'_, '_>) -> Option<String> {
    let text = element.text()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The family an `<alias>` for sans-serif names first (`<prefer>`, else `<accept>`, else `<default>`).
fn sans_serif(alias: roxmltree::Node<'_, '_>) -> Option<String> {
    let children = || alias.children().filter(roxmltree::Node::is_element);
    let family = children().find(|child| child.has_tag_name("family")).and_then(text_of)?;
    if !["sans-serif", "sans serif"].iter().any(|name| family.eq_ignore_ascii_case(name)) {
        return None;
    }
    ["prefer", "accept", "default"].iter().find_map(|kind| {
        children()
            .filter(|child| child.has_tag_name(*kind))
            .flat_map(|list| list.children().filter(|family| family.has_tag_name("family")))
            .find_map(text_of)
    })
}

/// The file at `path` as text, if it is a regular file of at most [`MAX_CONFIG`] bytes in UTF-8: the type and size
/// are checked on the open file, and no more than the cap is read.
fn read_text(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFIG {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG.saturating_add(1)).read_to_end(&mut bytes).ok()?;
    if u64::try_from(bytes.len()).ok()? > MAX_CONFIG {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
#[path = "fontconfig_tests.rs"]
mod tests;
