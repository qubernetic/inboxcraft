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
//!
//! The page is made on the frame (it is the reader's document written out again); writing it and handing it to the
//! desktop run on a thread of their own, so a desktop that is slow to answer, or never answers, does not stop any
//! window. The answer reaches the reader's notice on a later frame. One open runs at a time across the readers: a
//! press while one runs is ignored, and its notice says so.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::thread;

/// Opens a cleaned copy of a message in the desktop's browser. It is called on a thread of its own, never on the frame.
pub trait Original: Send + Sync {
    /// Puts `page`, the cleaned copy of message `id`, where the desktop can read it and hands it to the browser.
    ///
    /// # Errors
    ///
    /// Why it was not opened, as the owner reads it.
    fn open(&self, id: u64, page: &str) -> Result<(), String>;
}

/// How an open goes once asked for.
pub(crate) type Opening = Receiver<Result<(), String>>;

/// What [`Shared::start`] did.
#[derive(Debug)]
pub(crate) enum Started {
    /// The open runs; its answer comes on this.
    Running(Opening),
    /// Another open still runs: this one was not started.
    Busy,
    /// The thread could not be started.
    Failed(String),
}

/// One [`Original`] shared by the readers (the pane's and the windows of their own), one open at a time.
#[derive(Clone)]
pub(crate) struct Shared {
    original: Arc<dyn Original>,
    busy: Arc<AtomicBool>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Shared")
    }
}

/// Clears the busy mark when the open's thread ends, whatever way it ends.
struct Done(Arc<AtomicBool>);

impl Drop for Done {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Shared {
    /// `original`, to be shared.
    pub(crate) fn new(original: Box<dyn Original>) -> Self {
        Self { original: Arc::from(original), busy: Arc::new(AtomicBool::new(false)) }
    }

    /// Starts opening `page` as message `id`'s original ([`Original::open`]) on a thread of its own; `wake` is called
    /// once its answer is there, so the window draws again.
    pub(crate) fn start(&self, id: u64, page: String, wake: impl Fn() + Send + 'static) -> Started {
        if self.busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return Started::Busy;
        }
        let (sender, receiver) = channel();
        let (original, done) = (Arc::clone(&self.original), Done(Arc::clone(&self.busy)));
        let spawned = thread::Builder::new().name("inboxcraft-original".to_owned()).spawn(move || {
            let answer = original.open(id, &page);
            // Free before the answer is told, so a press right after it starts a new open.
            drop(done);
            // A reader that is gone no longer listens; nothing to tell it then.
            let _ = sender.send(answer);
            wake();
        });
        match spawned {
            Ok(_) => Started::Running(receiver),
            Err(error) => {
                self.busy.store(false, Ordering::Release);
                Started::Failed(error.to_string())
            }
        }
    }
}

#[cfg(test)]
#[path = "original_tests.rs"]
pub(crate) mod tests;
