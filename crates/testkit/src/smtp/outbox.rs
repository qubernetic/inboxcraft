// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The messages the fake accepted.

use std::sync::{Arc, Mutex, PoisonError};

/// One message the fake accepted: its envelope, the message, and how it came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub(super) id: u32,
    pub(super) from: String,
    pub(super) params: Vec<String>,
    pub(super) recipients: Vec<String>,
    pub(super) data: Vec<u8>,
    pub(super) user: Option<String>,
    pub(super) tls: bool,
}

impl Submission {
    /// The queue number the fake gave it in `250 Queued as <id>`, from 1 on.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The reverse path of MAIL FROM, without its angle brackets; empty for `<>`.
    #[must_use]
    pub fn from(&self) -> &str {
        &self.from
    }

    /// The parameters of MAIL FROM as sent (`SIZE=120`, `BODY=8BITMIME`, `SMTPUTF8`).
    #[must_use]
    pub fn params(&self) -> &[String] {
        &self.params
    }

    /// The recipients the fake took, in order, without their angle brackets: Bcc ones too, as they are only here.
    #[must_use]
    pub fn recipients(&self) -> &[String] {
        &self.recipients
    }

    /// The message as the client meant it: every line ended in CRLF, the dot-stuffing undone, the final dot gone.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The user the client had signed in as, if it had.
    #[must_use]
    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// Whether the message came over an encrypted connection.
    #[must_use]
    pub fn tls(&self) -> bool {
        self.tls
    }
}

/// The messages a fake accepted, in order, shared by every clone of the handle and every session of the fake.
#[derive(Debug, Clone, Default)]
pub struct Outbox(Arc<Mutex<Vec<Submission>>>);

impl Outbox {
    /// The messages accepted so far.
    #[must_use]
    pub fn messages(&self) -> Vec<Submission> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Captures `submission`, giving it the next queue number, which it returns.
    pub(super) fn capture(&self, mut submission: Submission) -> u32 {
        let mut messages = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let id = u32::try_from(messages.len()).unwrap_or(u32::MAX).saturating_add(1);
        submission.id = id;
        messages.push(submission);
        id
    }
}
