// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The owner's choices about InboxCraft without its window, as the Notifications page changes them ([`OwnerDesk`],
//! the app's [`Desk`]).
//!
//! Running in the background is one flag shared with the [`crate::desktop::Desktop`]: off, closing the window ends
//! InboxCraft. The choice is kept in [`FILE`] beside the appearance's file in the owner's config folder, read at the
//! next start ([`kept`]); a file that cannot be read keeps InboxCraft running, as it did before the choice existed.
//!
//! Starting with the computer is the autostart entry of [`crate::autostart`]. Outside the Flatpak it is written at
//! once; in the Flatpak the Background portal is asked first, on a thread of its own, since the desktop may ask the
//! person and take its time. One question waits at a time; the window is woken when its answer comes.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use inboxcraft_ui_egui::desk::Desk;
use inboxcraft_ui_egui::fetch::Wake;
use serde_json::{Value, json};

use crate::autostart::{self, Granted};

/// The name of the file that keeps the choice, beside the appearance's.
pub const FILE: &str = "background.json";

/// The largest choice file read, in bytes; the choice takes about twenty.
pub const MAX_FILE: usize = 1024;

/// Asks the Background portal, from any thread: whether InboxCraft starts at login.
pub type Portal = Arc<dyn Fn(bool) -> Result<Granted, String> + Send + Sync>;

/// Whether the choice kept in `file` leaves InboxCraft running when its window is closed: yes, unless the file says
/// `"background": false`.
#[must_use]
pub fn kept(file: &Path) -> bool {
    let Ok(handle) = std::fs::File::open(file) else { return true };
    let mut bytes = Vec::new();
    // One byte past the cap tells a file that is too large from one that is just large enough.
    if handle.take(u64::try_from(MAX_FILE).unwrap_or(u64::MAX).saturating_add(1)).read_to_end(&mut bytes).is_err() || bytes.len() > MAX_FILE {
        return true;
    }
    serde_json::from_slice::<Value>(&bytes).ok().and_then(|value| value.get("background").and_then(Value::as_bool)).unwrap_or(true)
}

/// The owner's [`Desk`].
pub struct OwnerDesk {
    stays: Arc<AtomicBool>,
    offered: bool,
    file: Option<PathBuf>,
    autostart: Option<PathBuf>,
    program: PathBuf,
    portal: Option<(Portal, Wake)>,
    login: Option<bool>,
    /// Where the answer to the start-at-login question on its way comes.
    waiting: Option<Receiver<Result<String, String>>>,
    /// An answer that came at once, until it is taken.
    ready: Option<Result<(), String>>,
}

impl std::fmt::Debug for OwnerDesk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnerDesk")
            .field("background", &self.background())
            .field("login", &self.login)
            .field("asking", &self.waiting.is_some())
            .finish_non_exhaustive()
    }
}

impl OwnerDesk {
    /// The choices of a run whose window closing leaves it running while `stays` is set, kept in `file`, with the
    /// autostart entry for `program` in `autostart` (the `autostart` folder of the config folder).
    #[must_use]
    pub fn new(stays: Arc<AtomicBool>, file: Option<PathBuf>, autostart: Option<PathBuf>, program: PathBuf) -> Self {
        let login = autostart.as_deref().map(autostart::is_on);
        Self { stays, offered: true, file, autostart, program, portal: None, login, waiting: None, ready: None }
    }

    /// The same, asking `portal` before the entry is changed (the Flatpak), and calling `wake` when it answered.
    #[must_use]
    pub fn with_portal(mut self, portal: Portal, wake: Wake) -> Self {
        self.portal = Some((portal, wake));
        self
    }

    /// The same, for a run nothing could bring a window back to: closing its window always ends it.
    #[must_use]
    pub fn without_background(mut self) -> Self {
        self.offered = false;
        self
    }

    /// The entry is there now, or not.
    fn look(&mut self) {
        self.login = self.autostart.as_deref().map(autostart::is_on);
    }
}

impl Desk for OwnerDesk {
    fn background(&self) -> Option<bool> {
        self.offered.then(|| self.stays.load(Ordering::SeqCst))
    }

    fn set_background(&mut self, on: bool) -> Result<(), String> {
        if !self.offered {
            return Err("InboxCraft cannot run without its window here: nothing could bring the window back".to_owned());
        }
        if let Some(file) = &self.file {
            let written = file
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(file, format!("{:#}\n", json!({ "background": on }))));
            written.map_err(|error| format!("the choice to run in the background could not be kept in {}: {error}", file.display()))?;
        }
        self.stays.store(on, Ordering::SeqCst);
        Ok(())
    }

    fn login(&self) -> Option<bool> {
        self.login
    }

    fn asking(&self) -> bool {
        self.waiting.is_some()
    }

    fn set_login(&mut self, on: bool) {
        if self.waiting.is_some() {
            return;
        }
        let Some(dir) = self.autostart.clone() else {
            self.ready = Some(Err("there is no folder for the autostart entry: HOME (or XDG_CONFIG_HOME) is not set".to_owned()));
            return;
        };
        let Some((portal, wake)) = self.portal.clone() else {
            let chosen = autostart::choose(on, &dir, &self.program, None);
            self.look();
            self.ready = Some(chosen.map(|_| ()));
            return;
        };
        let program = self.program.clone();
        let (send, answer) = mpsc::channel();
        // A thread that ends without an answer (it panicked) drops `send`, which is an answer as well.
        let asked = std::thread::Builder::new().name("inboxcraft-autostart".to_owned()).spawn(move || {
            let _ = send.send(autostart::choose(on, &dir, &program, Some(&*portal)));
            wake();
        });
        match asked {
            Ok(_) => self.waiting = Some(answer),
            Err(error) => self.ready = Some(Err(format!("the Background portal could not be asked: {error}"))),
        }
    }

    fn answer(&mut self) -> Option<Result<(), String>> {
        if let Some(ready) = self.ready.take() {
            return Some(ready);
        }
        let answer = match self.waiting.as_ref()?.try_recv() {
            Ok(chosen) => chosen.map(|_| ()),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("the Background portal did not answer".to_owned()),
        };
        self.waiting = None;
        self.look();
        Some(answer)
    }
}

#[cfg(test)]
#[path = "desk_tests.rs"]
mod tests;
