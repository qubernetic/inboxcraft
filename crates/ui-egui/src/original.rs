// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Opening a message's original in the browser, through an [`Original`] the app hands in
//! ([`crate::InboxApp::with_original`]).
//!
//! What goes to the browser is a cleaned copy: the display document the reader shows, written out again by
//! [`inboxcraft_html::page`], with no script, no event handler and no address of any kind in it. The message's own
//! bytes never leave the store. The act is the owner's press of the reader's More menu row alone: it is no command, so
//! no agent, palette, shortcut or control channel can ask for it. A window without an [`Original`] (the web app, the
//! sample's tests) does not offer the row.

use std::sync::{Arc, Mutex, PoisonError};

/// Opens a cleaned copy of a message in the desktop's browser.
pub trait Original: Send {
    /// Puts `page`, the cleaned copy of message `id`, where the desktop can read it and hands it to the browser.
    ///
    /// # Errors
    ///
    /// Why it was not opened, as the owner reads it.
    fn open(&mut self, id: u64, page: &str) -> Result<(), String>;
}

/// One [`Original`] shared by the readers (the pane's and the windows of their own), one open at a time.
#[derive(Clone)]
pub(crate) struct Shared(Arc<Mutex<Box<dyn Original>>>);

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Shared")
    }
}

impl Shared {
    /// `original`, to be shared.
    pub(crate) fn new(original: Box<dyn Original>) -> Self {
        Self(Arc::new(Mutex::new(original)))
    }

    /// Opens `page` as message `id`'s original, as [`Original::open`].
    pub(crate) fn open(&self, id: u64, page: &str) -> Result<(), String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).open(id, page)
    }
}

#[cfg(test)]
#[path = "original_tests.rs"]
mod tests;
