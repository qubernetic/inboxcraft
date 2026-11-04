// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What happens on the server at scripted points of a session: faults and lines sent as they are.

use crate::Fault;

/// A point in a session at which [`crate::imap::FakeImap::on`] plays its actions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum When {
    /// Before the greeting.
    Greeting,
    /// When the client has sent this command (`UID FETCH`, `IDLE`), before the server answers it.
    Command(String),
    /// After the server's tagged answer to this command.
    Answered(String),
}

impl When {
    /// [`When::Command`] for `name`, in any case.
    #[must_use]
    pub fn command(name: &str) -> Self {
        Self::Command(name.to_ascii_uppercase())
    }

    /// [`When::Answered`] for `name`, in any case.
    #[must_use]
    pub fn answered(name: &str) -> Self {
        Self::Answered(name.to_ascii_uppercase())
    }
}

/// Something the server does at a [`When`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Drop the connection, wait, or send garbage. A dropped connection ends the session.
    Fault(Fault),
    /// Send this line as it is (an `* OK [ALERT] ...`, or plaintext right after the STARTTLS answer); CR, LF and NUL
    /// are stripped from it.
    Send(String),
    /// Send `* BYE` with this text and close the connection.
    Bye(String),
}

/// Actions waiting for their [`When`]; each rule is played once, at the first matching point after the rules before
/// it with the same point.
#[derive(Debug, Clone, Default)]
pub(crate) struct Rules {
    waiting: Vec<(When, Vec<Action>)>,
}

impl Rules {
    pub(crate) fn add(&mut self, when: When, actions: Vec<Action>) {
        self.waiting.push((when, actions));
    }

    /// The actions of the first rule waiting for `when`, which is then played.
    pub(crate) fn take(&mut self, when: &When) -> Vec<Action> {
        match self.waiting.iter().position(|(waiting, _)| waiting == when) {
            Some(index) => self.waiting.remove(index).1,
            None => Vec::new(),
        }
    }
}
