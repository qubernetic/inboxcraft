// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the appearance choices are kept between runs: through a [`Keep`] the app hands in.
//!
//! The desktop app keeps them in `inboxcraft/appearance.json` in the owner's config folder ([`KeepFile`]), the sample
//! window too: the file holds no mail and nothing about an account. The file is read as outside data
//! ([`Appearance::from_value`]): a choice that cannot be read is the default, and a file larger than any appearance is
//! not read at all. It is written to a temporary file first and moved over the old one, so a crash mid-write leaves
//! the old choices. A window without a config folder gets [`NoKeep`].

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use inboxcraft_engine::appearance::Appearance;

/// The file's name.
pub const FILE: &str = "appearance.json";

/// The largest file read, in bytes; an appearance takes a few hundred.
pub const MAX_FILE: usize = 16 * 1024;

/// Keeps the appearance choices between runs.
pub trait Keep: Send {
    /// The choices kept, if any could be read.
    fn load(&self) -> Option<Appearance>;

    /// Keeps `appearance` in place of what was kept.
    ///
    /// # Errors
    ///
    /// Why nothing was kept, as the owner reads it.
    fn store(&mut self, appearance: &Appearance) -> Result<(), String>;
}

/// The keeping of a window that may not write files: nothing.
#[derive(Debug, Default)]
pub struct NoKeep;

impl Keep for NoKeep {
    fn load(&self) -> Option<Appearance> {
        None
    }

    fn store(&mut self, _appearance: &Appearance) -> Result<(), String> {
        Ok(())
    }
}

/// Keeps the choices in one file.
#[derive(Debug, Clone)]
pub struct KeepFile {
    path: PathBuf,
}

impl KeepFile {
    /// Keeps the choices in `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// [`FILE`] in `inboxcraft` in the owner's config folder: `XDG_CONFIG_HOME` when it is an absolute path (the
    /// Flatpak sets it to the app's own folder), `%APPDATA%` on Windows, `Library/Application Support` in the home
    /// folder on macOS, else `.config` in the home folder; `None` without one.
    #[must_use]
    pub fn of_owner() -> Option<Self> {
        let absolute = |value: std::ffi::OsString| Some(PathBuf::from(value)).filter(|path| path.is_absolute());
        let home = || std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).and_then(absolute);
        let folder = if let Some(config) = std::env::var_os("XDG_CONFIG_HOME").and_then(absolute) {
            config
        } else if cfg!(windows) {
            std::env::var_os("APPDATA").and_then(absolute)?
        } else if cfg!(target_os = "macos") {
            home()?.join("Library").join("Application Support")
        } else {
            home()?.join(".config")
        };
        Some(Self::new(folder.join("inboxcraft").join(FILE)))
    }

    /// The file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Keep for KeepFile {
    fn load(&self) -> Option<Appearance> {
        let file = std::fs::File::open(&self.path).ok()?;
        let mut bytes = Vec::new();
        // One byte past the cap tells a file that is too large from one that is just large enough.
        file.take(u64::try_from(MAX_FILE).unwrap_or(u64::MAX).saturating_add(1)).read_to_end(&mut bytes).ok()?;
        if bytes.len() > MAX_FILE {
            return None;
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        Some(Appearance::from_value(&value))
    }

    fn store(&mut self, appearance: &Appearance) -> Result<(), String> {
        let failed = |error: std::io::Error| format!("{}: {error}", self.path.display());
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder).map_err(failed)?;
        }
        let text = serde_json::to_string_pretty(&appearance.to_value()).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.new");
        let written = std::fs::File::create(&temporary).and_then(|mut file| file.write_all(text.as_bytes()).and_then(|()| file.sync_all()));
        if let Err(error) = written.and_then(|()| std::fs::rename(&temporary, &self.path)) {
            // Nothing is left half written; a temporary file that could not be removed is overwritten next time.
            let _ = std::fs::remove_file(&temporary);
            return Err(failed(error));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "keep_tests.rs"]
mod tests;
