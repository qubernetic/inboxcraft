// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A message as the store holds it.

use inboxcraft_model::Message;

/// A held message, whether the owner opened it and whether its body is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    message: Message,
    opened: bool,
    has_body: bool,
}

impl Entry {
    pub(crate) fn new(message: Message, opened: bool, has_body: bool) -> Self {
        Self { message, opened, has_body }
    }

    /// The message.
    #[must_use]
    pub fn message(&self) -> &Message {
        &self.message
    }

    /// Whether the owner opened it ([`Change::MarkOpened`](crate::Change::MarkOpened)).
    #[must_use]
    pub fn opened(&self) -> bool {
        self.opened
    }

    /// Whether its body is kept.
    #[must_use]
    pub fn has_body(&self) -> bool {
        self.has_body
    }
}
