// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where a saved attachment goes: through a [`Save`] the app hands in.
//!
//! The desktop app saves into the owner's downloads folder ([`Downloads`]). The file name is the sender's and
//! hostile: [`safe_name`] keeps it to one plain name in that folder (no path, no hidden file, no control or bidi
//! characters, no characters Windows refuses), and a file that is already there is never overwritten: the new one
//! gets a number. A window without a folder gets [`NoSave`].

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::tr;

/// The longest file name written, in characters, extension included.
pub const MAX_NAME: usize = 120;

/// How many numbered names are tried before saving gives up.
const MAX_TRIES: u32 = 1000;

/// Saves attachments.
pub trait Save: Send {
    /// Writes `bytes` under a name made from `name`; returns where it went, as the owner reads it.
    ///
    /// # Errors
    ///
    /// Why nothing was written, as the owner reads it.
    fn save(&mut self, name: &str, bytes: &[u8]) -> Result<String, String>;
}

/// The saving of a window that may not write files.
#[derive(Debug, Default)]
pub struct NoSave;

impl Save for NoSave {
    fn save(&mut self, _name: &str, _bytes: &[u8]) -> Result<String, String> {
        Err(tr!("Saving attachments is not available in this window.").to_owned())
    }
}

/// Saves into one folder.
#[derive(Debug, Clone)]
pub struct Downloads {
    folder: PathBuf,
}

impl Downloads {
    /// Saves into `folder`.
    #[must_use]
    pub fn new(folder: PathBuf) -> Self {
        Self { folder }
    }

    /// The owner's downloads folder: `XDG_DOWNLOAD_DIR` when it is an absolute path, else `Downloads` in the home
    /// folder; `None` without a home folder.
    #[must_use]
    pub fn of_owner() -> Option<Self> {
        let absolute = |value: std::ffi::OsString| Some(PathBuf::from(value)).filter(|path| path.is_absolute());
        if let Some(folder) = std::env::var_os("XDG_DOWNLOAD_DIR").and_then(absolute) {
            return Some(Self::new(folder));
        }
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).and_then(absolute).map(|home| Self::new(home.join("Downloads")))
    }

    /// The folder.
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}

impl Save for Downloads {
    fn save(&mut self, name: &str, bytes: &[u8]) -> Result<String, String> {
        let name = safe_name(name);
        let (stem, extension) = match name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => (stem.to_owned(), format!(".{extension}")),
            _ => (name.clone(), String::new()),
        };
        for n in 1..=MAX_TRIES {
            let candidate = if n == 1 { name.clone() } else { format!("{stem} ({n}){extension}") };
            let path = self.folder.join(&candidate);
            // `create_new` never overwrites and never follows a link that is already there.
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    return match file.write_all(bytes).and_then(|()| file.flush()) {
                        Ok(()) => Ok(path.display().to_string()),
                        Err(error) => Err(tr!("{name} could not be written: {error}", name = candidate, error = error)),
                    };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(tr!("{name} could not be written: {error}", name = candidate, error = error)),
            }
        }
        Err(tr!("{name} could not be written: too many files of that name", name = name))
    }
}

/// `name` made fit to be one file in a folder: the last part of a path, without control, bidi or format
/// characters and the characters Windows refuses, without dots and spaces at either end, cut to [`MAX_NAME`]
/// characters with the extension kept; `attachment` when nothing is left.
#[must_use]
pub fn safe_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let kept: String = last
        .chars()
        .filter(|c| !c.is_control())
        .filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        .filter(|c| !matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{061C}' | '\u{FEFF}'))
        .collect();
    let trimmed = kept.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let cut: String = if trimmed.chars().count() <= MAX_NAME {
        trimmed.to_owned()
    } else {
        let extension = trimmed.rsplit_once('.').map(|(_, extension)| extension).filter(|extension| extension.chars().count() <= 16).unwrap_or("");
        let room = MAX_NAME.saturating_sub(extension.chars().count().saturating_add(1));
        let stem: String = trimmed.chars().take(room).collect();
        if extension.is_empty() { trimmed.chars().take(MAX_NAME).collect() } else { format!("{}.{extension}", stem.trim_end_matches('.')) }
    };
    let stem = cut.split('.').next().unwrap_or_default();
    if cut.is_empty() {
        tr!("attachment").to_owned()
    } else if reserved_on_windows(stem.trim_end()) {
        format!("_{cut}")
    } else {
        cut
    }
}

/// Whether Windows keeps `stem` for a device: CON, PRN, AUX, NUL, CONIN$, CONOUT$, and COM or LPT with a digit
/// (0 to 9, or a superscript 1 to 3), in any case.
fn reserved_on_windows(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&upper.as_str()) {
        return true;
    }
    let Some(rest) = upper.strip_prefix("COM").or_else(|| upper.strip_prefix("LPT")) else {
        return false;
    };
    let mut chars = rest.chars();
    matches!((chars.next(), chars.next()), (Some('0'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None))
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
