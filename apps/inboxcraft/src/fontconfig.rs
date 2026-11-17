// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The font folders fontconfig's files name, read by the app itself and bounded, for [`crate::fonts`].
//!
//! Only what the search needs is taken from a file (fonts-conf(5)): `<dir>` and `<remap-dir>` (whose text is the
//! folder to read; `as-path` is only the name fontconfig's cache gives it), `<include>`, and the `<alias>` that names
//! the sans-serif family. Everything else is ignored. The files are hostile: only regular files are read (no device,
//! FIFO, socket or folder in their place), at most [`MAX_CONFIG`] bytes each and [`MAX_CONFIGS`] files in all, an
//! include goes at most [`MAX_INCLUDE_DEPTH`] deep, each file and each folder is read once (so include loops end), a
//! folder is listed for at most [`MAX_LISTED`] entries, and only the top elements and their children are looked at.
//!
//! The XML parser recurses once per nesting level, so the nesting is counted before it parses. Both see the same
//! text: [`plain`] first takes out comments, processing instructions and the document type (refusing an internal
//! DTD and any CDATA, which fontconfig's files never have), then the count runs on what is left and the parser
//! parses exactly that, with at most [`MAX_NODES`] nodes. A file nested more than [`MAX_NESTING`] deep is passed
//! over. The bytes-level part, [`parse`], touches no file and is what the fuzz target drives.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// The largest configuration file read: 1 MiB.
pub const MAX_CONFIG: u64 = 1024 * 1024;

/// The most configuration files read in all.
pub const MAX_CONFIGS: usize = 256;

/// How deep includes go.
pub const MAX_INCLUDE_DEPTH: usize = 8;

/// The most entries of one folder of configuration files looked at.
pub const MAX_LISTED: usize = 4_096;

/// How deep elements may nest in a file; a file nested deeper is passed over.
pub const MAX_NESTING: usize = 64;

/// The most XML nodes one file may hold.
pub const MAX_NODES: u32 = 100_000;

/// Where relative and prefixed names in the files point.
#[derive(Debug, Clone, Default)]
pub struct Places {
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
    /// The folder entries looked at, in all folders of configuration files together.
    pub(crate) listed: usize,
}

/// One thing a file names, in its order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Named {
    /// A font folder (`<dir>`, `<remap-dir>`).
    Folder(PathBuf),
    /// A file or folder of configuration files to read too (`<include>`).
    Include(PathBuf),
    /// The family first named for sans-serif (`<alias>`).
    Sans(String),
}

/// What the configuration file `bytes`, in folder `here`, names, in its order; `None` for a file that is not
/// fontconfig's XML within the limits (not UTF-8, larger than [`MAX_CONFIG`], with CDATA or an internal DTD, nested
/// more than [`MAX_NESTING`] deep, more than [`MAX_NODES`] nodes, not well-formed). Touches no file.
#[must_use]
pub fn parse(bytes: &[u8], here: Option<&Path>, places: &Places) -> Option<Vec<Named>> {
    if u64::try_from(bytes.len()).ok()? > MAX_CONFIG {
        return None;
    }
    let text = plain(std::str::from_utf8(bytes).ok()?)?;
    if !nesting_within(&text, MAX_NESTING) {
        return None;
    }
    let options = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: MAX_NODES };
    let document = roxmltree::Document::parse_with_options(&text, options).ok()?;
    let root = document.root_element();
    if root.tag_name().name() != "fontconfig" {
        return Some(Vec::new());
    }
    let mut named = Vec::new();
    for element in root.children().filter(roxmltree::Node::is_element) {
        let found = match element.tag_name().name() {
            "dir" | "remap-dir" => folder(element, here, places).map(Named::Folder),
            "include" => include(element, here, places).map(Named::Include),
            "alias" => sans_serif(element).map(Named::Sans),
            _ => None,
        };
        named.extend(found);
    }
    Some(named)
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
            // A folder included again (or through another name) is not listed again.
            let Ok(real) = std::fs::canonicalize(path) else { return };
            if !self.seen.insert(real.clone()) {
                return;
            }
            let Ok(entries) = std::fs::read_dir(&real) else { return };
            let mut names: Vec<PathBuf> = entries
                .take(MAX_LISTED)
                .inspect(|_| self.found.listed = self.found.listed.saturating_add(1))
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
        let here = real.parent().map(Path::to_owned);
        let Some(named) = read_bytes(&real).and_then(|bytes| parse(&bytes, here.as_deref(), self.places)) else {
            self.found.skipped.push(real);
            return;
        };
        for item in named {
            match item {
                Named::Folder(folder) => self.found.folders.push(folder),
                Named::Include(include) => self.path(&include, depth.saturating_add(1)),
                Named::Sans(sans) => self.found.sans = Some(sans),
            }
        }
    }
}

/// The folder a `<dir>` or `<remap-dir>` names, in the file in folder `here`.
fn folder(element: roxmltree::Node<'_, '_>, here: Option<&Path>, places: &Places) -> Option<PathBuf> {
    let text = text_of(element)?;
    if let Some(rest) = text.strip_prefix('~') {
        return in_home(rest, places);
    }
    let base = match element.attribute("prefix") {
        Some("xdg") => places.data_home.clone()?,
        Some("relative") => here?.to_owned(),
        _ => places.cwd.clone().unwrap_or_default(),
    };
    Some(base.join(text))
}

/// The file or folder an `<include>` names, in the file in folder `here`; `ignore_missing` only quiets fontconfig's
/// warning, and this reading warns about nothing.
fn include(element: roxmltree::Node<'_, '_>, here: Option<&Path>, places: &Places) -> Option<PathBuf> {
    let text = text_of(element)?;
    if let Some(rest) = text.strip_prefix('~') {
        return in_home(rest, places);
    }
    let base = match element.attribute("prefix") {
        Some("xdg") => places.config_home.clone()?,
        _ => here?.to_owned(),
    };
    Some(base.join(text))
}

/// `rest` (what follows a `~`) in the home folder.
fn in_home(rest: &str, places: &Places) -> Option<PathBuf> {
    places.home.as_ref().map(|home| home.join(rest.trim_start_matches('/')))
}

/// `text` without its comments, processing instructions (the XML declaration among them) and document type, each
/// replaced by a space: the one text both the nesting count and the parser see. `None` for a text with CDATA, an
/// internal DTD, another `<!` declaration, or any of them left open. Each terminator is looked for only after its
/// whole opener, as an XML parser does (`<!-->` opens a comment, it does not close one). Without recursion.
pub(crate) fn plain(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(next) = bytes.get(at..).and_then(|rest| rest.iter().position(|byte| *byte == b'<')) {
        let start = at.saturating_add(next);
        out.push_str(text.get(at..start)?);
        let rest = bytes.get(start..)?;
        let end = if rest.starts_with(b"<!--") {
            find(rest.get(4..)?, b"-->")?.saturating_add(4 + 3)
        } else if rest.starts_with(b"<?") {
            find(rest.get(2..)?, b"?>")?.saturating_add(2 + 2)
        } else if rest.starts_with(b"<!DOCTYPE") {
            let end = tag_end(rest, true)?;
            if rest.get(..end)?.contains(&b'[') {
                return None;
            }
            end
        } else if rest.starts_with(b"<!") {
            // CDATA and every other declaration.
            return None;
        } else {
            // A tag, copied whole, so nothing inside its quotes is taken for markup.
            let end = tag_end(rest, false)?;
            out.push_str(text.get(start..start.saturating_add(end))?);
            at = start.saturating_add(end);
            continue;
        };
        out.push(' ');
        at = start.saturating_add(end);
    }
    out.push_str(text.get(at..)?);
    Some(out)
}

/// Whether the elements of `text` (the output of [`plain`]) nest at most `max` deep, quotes in a tag honoured; a
/// `<!` or `<?` left in it, or a tag left open, counts as too deep.
fn nesting_within(text: &str, max: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth: usize = 0;
    let mut at = 0;
    while let Some(next) = bytes.get(at..).and_then(|rest| rest.iter().position(|byte| *byte == b'<')) {
        at = at.saturating_add(next);
        let rest = bytes.get(at..).unwrap_or_default();
        if rest.starts_with(b"<!") || rest.starts_with(b"<?") {
            return false;
        }
        let Some(end) = tag_end(rest, false) else { return false };
        if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
        } else if rest.get(end.saturating_sub(2)) != Some(&b'/') {
            depth = depth.saturating_add(1);
            if depth > max {
                return false;
            }
        }
        at = at.saturating_add(end);
    }
    true
}

/// Where `needle` starts in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

/// Just past the `>` that ends the tag at the start of `rest`, with quotes honoured, and in a declaration
/// (`declaration`) its `[...]` too.
fn tag_end(rest: &[u8], declaration: bool) -> Option<usize> {
    let mut quote: Option<u8> = None;
    let mut brackets: usize = 0;
    for (at, byte) in rest.iter().enumerate().skip(1) {
        match (quote, *byte) {
            (Some(open), byte) if byte == open => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(*byte),
            (None, b'[') if declaration => brackets = brackets.saturating_add(1),
            (None, b']') if declaration => brackets = brackets.saturating_sub(1),
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

/// The file at `path`, if it is a regular file of at most [`MAX_CONFIG`] bytes: the type and size are checked on the
/// open file, and no more than the cap is read.
fn read_bytes(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFIG {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG.saturating_add(1)).read_to_end(&mut bytes).ok()?;
    (u64::try_from(bytes.len()).ok()? <= MAX_CONFIG).then_some(bytes)
}

#[cfg(test)]
#[path = "fontconfig_tests.rs"]
mod tests;
