// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the owner's mail is kept: the SQLite store in the owner's data folder, kept between runs.
//!
//! The store keeps the mail, the job queue and the local records; a job the owner's act queued and no worker ran
//! before the app ended runs at the next start. Passwords are never in it (they live in the OS credential store).

use std::path::{Path, PathBuf};

use inboxcraft_store::{Sqlite, Storage, StoreConfig};

use crate::Source;

/// The store's folder inside the owner's data folder.
pub const STORE_DIR: &str = "inboxcraft/store";

/// The mail a window shows.
#[derive(Debug)]
pub enum Mail {
    /// The invented sample mailbox, built in memory each time.
    Sample,
    /// The owner's store.
    Store(Owned),
}

/// The owner's store, with its id.
pub struct Owned {
    /// The store.
    pub store: Box<dyn Storage>,
    /// Its own id ([`Sqlite::id`]), which names its accounts' entries in the credential store.
    pub id: String,
}

impl std::fmt::Debug for Owned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Owned").field("id", &self.id).finish_non_exhaustive()
    }
}

/// [`STORE_DIR`] in the owner's data folder: `%LOCALAPPDATA%` on Windows, else `XDG_DATA_HOME` when it is an absolute
/// path (the Flatpak sets it to the app's own `~/.var/app/io.github.qubernetic.inboxcraft/data`), `Library/Application
/// Support` in the home folder on macOS, else `.local/share` in the home folder; `None` without one. Windows takes no
/// `XDG_DATA_HOME` (one a Unix-like shell set): the store's files are narrowed to the owner on Unix only, and on
/// Windows they inherit the folder's access list, which only `%LOCALAPPDATA%` keeps to the owner.
#[must_use]
pub fn store_dir() -> Option<PathBuf> {
    let absolute = |value: std::ffi::OsString| Some(PathBuf::from(value)).filter(|path| path.is_absolute());
    let home = || std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).and_then(absolute);
    let folder = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").and_then(absolute)?
    } else if let Some(data) = std::env::var_os("XDG_DATA_HOME").and_then(absolute) {
        data
    } else if cfg!(target_os = "macos") {
        home()?.join("Library").join("Application Support")
    } else {
        home()?.join(".local").join("share")
    };
    Some(folder.join(STORE_DIR))
}

/// Opens the store in `dir`, creating it if needed.
///
/// # Errors
///
/// What went wrong, naming the folder: a store from a newer build, a damaged one (also one without its id), a folder
/// that cannot be made.
pub fn open_store(dir: &Path) -> Result<Owned, String> {
    let opened = Sqlite::open(dir, StoreConfig::default()).and_then(|store| Ok(Owned { id: store.id()?, store: Box::new(store) }));
    opened.map_err(|error| format!("the mail store in {} could not be opened: {error}", dir.display()))
}

/// The mail the window on `source` shows: the sample, or the owner's store in [`store_dir`].
///
/// # Errors
///
/// No folder to keep the mail in, or a store that does not open; the window does not open on a store that is not
/// kept, so nothing the owner does is lost at the end of the run.
pub fn mail(source: Source) -> Result<Mail, String> {
    match source {
        Source::Sample => Ok(Mail::Sample),
        Source::Owner => {
            let dir = store_dir().ok_or("there is no folder to keep the mail in: HOME (or XDG_DATA_HOME) is not set")?;
            open_store(&dir).map(Mail::Store)
        }
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
