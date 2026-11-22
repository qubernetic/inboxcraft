// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A fontconfig file's bytes to the folders, includes and sans-serif family it names: no file touched, nothing used
//! beyond `std` and `roxmltree`, so the fuzz target (`fuzz/fuzz_targets/fontconfig_scan.rs`) includes this file as
//! it is. The XML parser recurses once per nesting level, so the nesting is counted before it parses, and both see
//! the same text: [`plain`] takes out comments, processing instructions and the document type (refusing an internal
//! DTD and any CDATA, which fontconfig's files never have), the count ([`depth_of`]) runs on what is left, and the
//! parser parses exactly that.

use std::path::{Path, PathBuf};

/// The largest configuration file read: 1 MiB.
pub const MAX_CONFIG: u64 = 1024 * 1024;

/// How deep elements may nest in a file; a file nested deeper is passed over.
pub const MAX_NESTING: usize = 64;

/// The most XML nodes one file may hold. Also a bound on the parser's depth on its own, should the nesting count ever
/// let a deep file through: 20 000 levels at about 210 bytes of stack each (seen in a debug build) is about 4.2 MB,
/// well inside the font worker's 16 MiB stack (`fonts::WORKER_STACK`). The largest real fontconfig file has about
/// 1 300 elements.
pub const MAX_NODES: u32 = 20_000;

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
/// taken out without a trace (so `/usr/<!--x-->share` reads `/usr/share`): the one text both the nesting count and the parser see. `None` for a text with CDATA, an
/// internal DTD, another `<!` declaration, or any of them left open. Each terminator is looked for only after its
/// whole opener, as an XML parser does (`<!-->` opens a comment, it does not close one). Without recursion.
pub fn plain(text: &str) -> Option<String> {
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
        at = start.saturating_add(end);
    }
    out.push_str(text.get(at..)?);
    Some(out)
}

/// Whether the elements of `text` (the output of [`plain`]) nest at most `max` deep.
fn nesting_within(text: &str, max: usize) -> bool {
    depth_of(text).is_some_and(|depth| depth <= max)
}

/// How deep the elements of `text` (the output of [`plain`]) nest, quotes in a tag honoured; `None` for a `<!` or `<?`
/// left in it, or a tag left open. Without recursion, linear in the text.
pub fn depth_of(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let (mut depth, mut deepest): (usize, usize) = (0, 0);
    let mut at = 0;
    while let Some(next) = bytes.get(at..).and_then(|rest| rest.iter().position(|byte| *byte == b'<')) {
        at = at.saturating_add(next);
        let rest = bytes.get(at..).unwrap_or_default();
        if rest.starts_with(b"<!") || rest.starts_with(b"<?") {
            return None;
        }
        let end = tag_end(rest, false)?;
        if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
        } else if rest.get(end.saturating_sub(2)) == Some(&b'/') {
            // A self-closed element is one level deeper, and closes at once.
            deepest = deepest.max(depth.saturating_add(1));
        } else {
            depth = depth.saturating_add(1);
            deepest = deepest.max(depth);
        }
        at = at.saturating_add(end);
    }
    Some(deepest)
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
