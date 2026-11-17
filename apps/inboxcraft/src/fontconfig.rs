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
//! text: `xml::plain` first takes out comments, processing instructions and the document type (refusing an internal
//! DTD and any CDATA, which fontconfig's files never have), then the count runs on what is left and the parser
//! parses exactly that, with at most [`MAX_NODES`] nodes. A file nested more than [`MAX_NESTING`] deep is passed
//! over. The bytes-level part, [`parse`], touches no file and is what the fuzz target drives.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

mod xml;

pub use xml::{MAX_CONFIG, MAX_NESTING, MAX_NODES, Named, Places, parse};
#[cfg(test)]
use xml::{depth_of, plain};

/// The most configuration files read in all.
pub const MAX_CONFIGS: usize = 256;

/// How deep includes go.
pub const MAX_INCLUDE_DEPTH: usize = 8;

/// The most entries of one folder of configuration files looked at.
pub const MAX_LISTED: usize = 4_096;

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
