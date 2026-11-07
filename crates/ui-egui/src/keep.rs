// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the appearance choices and the keys are kept between runs: through a [`Keep`] the app hands in.
//!
//! The desktop app keeps them in `inboxcraft/appearance.json` in the owner's config folder ([`KeepFile`]), the sample
//! window too: the file holds no mail and nothing about an account. The file is read as outside data
//! ([`Appearance::from_value`]): a choice that cannot be read is the default, and a file larger than any appearance is
//! not read at all. It is written to a temporary file first and moved over the old one, so a crash mid-write leaves
//! the old choices. The keys go into [`KEYS_FILE`] beside it, read the same way ([`Keymap::from_value`]), and the
//! triage choices (the screener's mode, the inbox view) into [`TRIAGE_FILE`] ([`Triage::from_value`]). A window
//! without a config folder gets [`NoKeep`].

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use inboxcraft_engine::ScreenerMode;
use inboxcraft_engine::appearance::Appearance;
use serde_json::{Value, json};

use crate::keymap::Keymap;
use crate::mailbox::InboxView;

/// The file's name.
pub const FILE: &str = "appearance.json";

/// The name of the keys' file, beside [`FILE`].
pub const KEYS_FILE: &str = "shortcuts.json";

/// The name of the triage choices' file, beside [`FILE`].
pub const TRIAGE_FILE: &str = "triage.json";

/// The triage choices kept between runs: what the screener does with a new sender's first mail, and how the unified
/// inbox shows its groups.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Triage {
    /// The screener's mode.
    pub screener: ScreenerMode,
    /// How the unified inbox shows its groups.
    pub view: InboxView,
}

impl Triage {
    /// The choices `value` holds, read as outside data: a choice that cannot be read is the default.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let named = |key: &str| value.get(key).and_then(Value::as_str);
        Self {
            screener: named("screener").and_then(ScreenerMode::from_name).unwrap_or_default(),
            view: named("view").and_then(InboxView::named).unwrap_or_default(),
        }
    }

    /// The choices as they are written.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({ "screener": self.screener.name(), "view": self.view.id() })
    }
}

/// The largest file read, in bytes; an appearance takes a few hundred, the keys about a thousand.
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

    /// The keys kept, if any could be read; none by default.
    fn load_keys(&self) -> Option<Keymap> {
        None
    }

    /// Keeps `keymap` in place of the keys kept; keeps nothing by default.
    ///
    /// # Errors
    ///
    /// Why nothing was kept, as the owner reads it.
    fn store_keys(&mut self, _keymap: &Keymap) -> Result<(), String> {
        Ok(())
    }

    /// The triage choices kept, if any could be read; none by default.
    fn load_triage(&self) -> Option<Triage> {
        None
    }

    /// Keeps `triage` in place of the choices kept; keeps nothing by default.
    ///
    /// # Errors
    ///
    /// Why nothing was kept, as the owner reads it.
    fn store_triage(&mut self, _triage: &Triage) -> Result<(), String> {
        Ok(())
    }
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

impl KeepFile {
    /// The keys' file, beside the appearance's; none when the appearance's path names no file.
    fn keys_path(&self) -> Option<PathBuf> {
        self.beside(KEYS_FILE)
    }

    /// The file `name` beside the appearance's; none when the appearance's path names no file.
    fn beside(&self, name: &str) -> Option<PathBuf> {
        self.path.file_name().map(|_| self.path.with_file_name(name))
    }
}

/// The JSON in `path`, unless it is missing, larger than [`MAX_FILE`] or not JSON.
fn read_json(path: &Path) -> Option<Value> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    // One byte past the cap tells a file that is too large from one that is just large enough.
    file.take(u64::try_from(MAX_FILE).unwrap_or(u64::MAX).saturating_add(1)).read_to_end(&mut bytes).ok()?;
    if bytes.len() > MAX_FILE {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// Writes `value` into `path` through a temporary file moved over the old one.
fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("{}: {error}", path.display());
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.new");
    let written = std::fs::File::create(&temporary).and_then(|mut file| file.write_all(text.as_bytes()).and_then(|()| file.sync_all()));
    if let Err(error) = written.and_then(|()| std::fs::rename(&temporary, path)) {
        // Nothing is left half written; a temporary file that could not be removed is overwritten next time.
        let _ = std::fs::remove_file(&temporary);
        return Err(failed(error));
    }
    Ok(())
}

impl Keep for KeepFile {
    fn load(&self) -> Option<Appearance> {
        read_json(&self.path).map(|value| Appearance::from_value(&value))
    }

    fn store(&mut self, appearance: &Appearance) -> Result<(), String> {
        write_json(&self.path, &appearance.to_value())
    }

    fn load_keys(&self) -> Option<Keymap> {
        read_json(&self.keys_path()?).map(|value| Keymap::from_value(&value))
    }

    fn store_keys(&mut self, keymap: &Keymap) -> Result<(), String> {
        let path = self.keys_path().ok_or_else(|| format!("{}: no file to keep the keys beside", self.path.display()))?;
        write_json(&path, &keymap.to_value())
    }

    fn load_triage(&self) -> Option<Triage> {
        read_json(&self.beside(TRIAGE_FILE)?).map(|value| Triage::from_value(&value))
    }

    fn store_triage(&mut self, triage: &Triage) -> Result<(), String> {
        let path = self.beside(TRIAGE_FILE).ok_or_else(|| format!("{}: no file to keep the triage choices beside", self.path.display()))?;
        write_json(&path, &triage.to_value())
    }
}

#[cfg(test)]
#[path = "keep_tests.rs"]
mod tests;
