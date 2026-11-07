// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Mail that arrives from outside a session, such as the fake SMTP server's delivery reports.

use std::sync::{Arc, Mutex, PoisonError};

use crate::imap::Message;

/// Messages on their way into a [`FakeImap`](crate::imap::FakeImap)'s mailboxes from another thread, such as the
/// delivery reports of [`crate::smtp::Bounce`]. Every clone is the same postbox.
///
/// The fake IMAP server takes what has arrived before it answers each command, and the client hears of it as of any
/// new mail: `EXISTS` at once for the open mailbox, `STATUS` for another one NOTIFY covers. A client that idles hears
/// of it only at its next command.
#[derive(Debug, Clone, Default)]
pub struct Postbox(Arc<Mutex<Vec<(String, Message)>>>);

impl Postbox {
    /// An empty postbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts `message` in for the mailbox called `mailbox`; a mailbox the fake does not have drops it.
    pub fn deliver(&self, mailbox: &str, message: Message) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push((mailbox.to_owned(), message));
    }

    /// Takes everything that has arrived, in order.
    pub(crate) fn take(&self) -> Vec<(String, Message)> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}
