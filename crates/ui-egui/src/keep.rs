// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the appearance choices and the keys are kept between runs: through a [`Keep`] the app hands in.
//!
//! The desktop app keeps them in `inboxcraft/appearance.json` in the owner's config folder ([`KeepFile`]), the sample
//! window too: the file holds no mail and nothing about an account. The file is read as outside data
//! ([`Appearance::from_value`]): a choice that cannot be read is the default, and a file larger than any appearance is
//! not read at all. It is written to a temporary file first and moved over the old one, so a crash mid-write leaves
//! the old choices. The keys go into [`KEYS_FILE`] beside it, read the same way ([`Keymap::from_value`]), and the
//! triage choices (the screener's mode, the inbox view, whether a pin outlasts done, how long a send is held, what follows an act, and which word of the sending name Home greets by) into [`TRIAGE_FILE`] ([`Triage::from_value`]), and the
//! notification choices (each account's level, returned snoozes and reminders) into [`NOTIFY_FILE`]
//! ([`NotifySettings::from_value`]); that file names accounts by their number only. The week rule (`week.set`) goes into
//! [`WEEK_FILE`], read as outside data by the session ([`inboxcraft_engine::week::from_value`]). A window without a
//! config folder gets [`NoKeep`].

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use inboxcraft_engine::appearance::Appearance;
use inboxcraft_engine::notify::Settings as NotifySettings;
use inboxcraft_engine::{Hold, ScreenerMode};
use serde_json::{Value, json};

use crate::keymap::Keymap;
use crate::mailbox::{After, GreetBy, InboxView};

/// The file's name.
pub const FILE: &str = "appearance.json";

/// The name of the keys' file, beside [`FILE`].
pub const KEYS_FILE: &str = "shortcuts.json";

/// The name of the triage choices' file, beside [`FILE`].
pub const TRIAGE_FILE: &str = "triage.json";

/// The name of the notification choices' file, beside [`FILE`].
pub const NOTIFY_FILE: &str = "notify.json";

/// The name of the week rule's file, beside [`FILE`].
pub const WEEK_FILE: &str = "week.json";

/// The triage choices kept between runs: what the screener does with a new sender's first mail, how the unified
/// inbox shows its groups, whether a pin stays on a message when it is done, how long a send is held before it goes,
/// what follows an act on the open message, and which word of the sending name Home greets by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Triage {
    /// The screener's mode.
    pub screener: ScreenerMode,
    /// How the unified inbox shows its groups.
    pub view: InboxView,
    /// A pin stays on a message when it is done.
    pub keep_pins: bool,
    /// How long a send is held, in seconds ([`Hold::MIN`] to [`Hold::MAX`]); `None` until it was kept.
    pub hold: Option<u8>,
    /// What follows an act on the open message.
    pub after: After,
    /// Which word of the sending name Home greets by.
    pub greet_by: GreetBy,
}

impl Triage {
    /// The choices `value` holds, read as outside data: a choice that cannot be read is the default.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let named = |key: &str| value.get(key).and_then(Value::as_str);
        Self {
            screener: named("screener").and_then(ScreenerMode::from_name).unwrap_or_default(),
            // Today's key first; a file of an earlier InboxCraft names the view under `view`, by the ids of then.
            view: match named("inbox_view") {
                Some(name) => InboxView::named(name),
                None => named("view").and_then(InboxView::named_before),
            }
            .unwrap_or_default(),
            keep_pins: value.get("keep_pins").and_then(Value::as_bool).unwrap_or(false),
            hold: value.get("hold_seconds").and_then(Value::as_u64).and_then(Hold::new).map(Hold::seconds),
            after: named("after").and_then(After::named).unwrap_or_default(),
            greet_by: named("greeting").and_then(GreetBy::named).unwrap_or_default(),
        }
    }

    /// The choices as they are written: the view under `inbox_view`, and under `view` by the ids of earlier
    /// InboxCraft, which reads only that key.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "screener": self.screener.name(),
            "inbox_view": self.view.id(),
            "view": self.view.id_before(),
            "keep_pins": self.keep_pins,
            "hold_seconds": self.hold,
            "after": self.after.name(),
            "greeting": self.greet_by.name(),
        })
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

    /// The notification choices kept, if any could be read; none by default.
    fn load_notify(&self) -> Option<NotifySettings> {
        None
    }

    /// Keeps `settings` in place of the notification choices kept; keeps nothing by default.
    ///
    /// # Errors
    ///
    /// Why nothing was kept, as the owner reads it.
    fn store_notify(&mut self, _settings: &NotifySettings) -> Result<(), String> {
        Ok(())
    }

    /// The week rule kept, as written ([`inboxcraft_engine::week::to_value`]), if any could be read; none by default.
    fn load_week(&self) -> Option<Value> {
        None
    }

    /// Keeps `week` in place of the week rule kept; keeps nothing by default.
    ///
    /// # Errors
    ///
    /// Why nothing was kept, as the owner reads it.
    fn store_week(&mut self, _week: &Value) -> Result<(), String> {
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

/// The JSON in `path`, unless it is missing, not a regular file, larger than [`MAX_FILE`] or not JSON.
fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&read_kept(path, MAX_FILE)?).ok()
}

/// The bytes of the kept file `path`, unless it is missing, not a regular file or larger than `cap` bytes.
///
/// A kept file is read at the start, so nothing in it may hold the start up: a named pipe, a link to one, a device or
/// a folder is never read. The type is looked at before the file is opened (following a link, so a kept file may be a
/// link to a regular file), the file is opened in a way that does not wait for a pipe's writer where the platform has
/// one, and the type of what was opened is looked at again, in case the file changed in between.
#[must_use]
pub fn read_kept(path: &Path, cap: usize) -> Option<Vec<u8>> {
    if !regular(&std::fs::metadata(path).ok()?) {
        return None;
    }
    read_opened(open_kept(path).ok()?, cap)
}

/// Whether `metadata` is that of a regular file.
fn regular(metadata: &std::fs::Metadata) -> bool {
    metadata.is_file()
}

/// The bytes of the opened kept file, unless it is not a regular file or larger than `cap` bytes.
fn read_opened(file: std::fs::File, cap: usize) -> Option<Vec<u8>> {
    if !regular(&file.metadata().ok()?) {
        return None;
    }
    let mut bytes = Vec::new();
    // One byte past the cap tells a file that is too large from one that is just large enough.
    file.take(u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1)).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= cap).then_some(bytes)
}

/// `O_NONBLOCK`: an open with it never waits for a pipe's writer, and a regular file reads as without it.
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "s390x",
        target_arch = "loongarch64"
    )
))]
const NO_WAIT: i32 = 0o4000;
/// `O_NONBLOCK`: an open with it never waits for a pipe's writer, and a regular file reads as without it.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly"))]
const NO_WAIT: i32 = 0x0004;
/// No flag of known value keeps the open from waiting here; the type looked at before the open is the guard.
#[cfg(all(
    unix,
    not(any(
        all(
            any(target_os = "linux", target_os = "android"),
            any(
                target_arch = "x86",
                target_arch = "x86_64",
                target_arch = "arm",
                target_arch = "aarch64",
                target_arch = "riscv32",
                target_arch = "riscv64",
                target_arch = "powerpc",
                target_arch = "powerpc64",
                target_arch = "s390x",
                target_arch = "loongarch64"
            )
        ),
        any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly")
    ))
))]
const NO_WAIT: i32 = 0;

/// Opens the kept file `path` to read, without waiting for a pipe's writer.
#[cfg(unix)]
fn open_kept(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().read(true).custom_flags(NO_WAIT).open(path)
}

/// Opens the kept file `path` to read; the type looked at before the open is the guard.
#[cfg(not(unix))]
fn open_kept(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
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

    fn load_notify(&self) -> Option<NotifySettings> {
        read_json(&self.beside(NOTIFY_FILE)?).map(|value| NotifySettings::from_value(&value))
    }

    fn store_notify(&mut self, settings: &NotifySettings) -> Result<(), String> {
        let path = self.beside(NOTIFY_FILE).ok_or_else(|| format!("{}: no file to keep the notification choices beside", self.path.display()))?;
        write_json(&path, &settings.to_value())
    }

    fn load_week(&self) -> Option<Value> {
        read_json(&self.beside(WEEK_FILE)?)
    }

    fn store_week(&mut self, week: &Value) -> Result<(), String> {
        let path = self.beside(WEEK_FILE).ok_or_else(|| format!("{}: no file to keep the week rule beside", self.path.display()))?;
        write_json(&path, week)
    }
}

#[cfg(test)]
#[path = "keep_tests.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "keep_unix_tests.rs"]
mod unix_tests;
