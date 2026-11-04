// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The script a fake server plays to one client: lines to send, lines to expect and faults.

use std::time::Duration;

/// Something that goes wrong on purpose, at a point of the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// Close the connection at once; the rest of the script is not played.
    Drop,
    /// Wait this long before the next step.
    Delay(Duration),
    /// Send these bytes as they are, terminator included or not.
    Garbage(Vec<u8>),
}

/// One step of a [`Script`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Send this line; the server adds CRLF.
    Send(String),
    /// Read one line and fail the script unless it is exactly this.
    Expect(String),
    /// Play a fault.
    Fault(Fault),
}

/// The steps a fake server plays, in order, to the one client it serves.
///
/// ```
/// use inboxcraft_testkit::{Fault, Script, Step};
///
/// let script = Script::new().send("* OK ready").expect("a1 LOGOUT").drop_connection();
/// assert_eq!(script.steps().last(), Some(&Step::Fault(Fault::Drop)));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Script {
    steps: Vec<Step>,
}

impl Script {
    /// An empty script: the server accepts the client and closes the connection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a line to send. CR, LF and NUL are stripped from it; [`Script::garbage`] sends raw bytes.
    #[must_use]
    pub fn send(self, line: &str) -> Self {
        self.step(Step::Send(line.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect()))
    }

    /// Adds a line the client must send next.
    #[must_use]
    pub fn expect(self, line: &str) -> Self {
        self.step(Step::Expect(line.to_owned()))
    }

    /// Adds [`Fault::Drop`].
    #[must_use]
    pub fn drop_connection(self) -> Self {
        self.step(Step::Fault(Fault::Drop))
    }

    /// Adds [`Fault::Delay`].
    #[must_use]
    pub fn delay(self, wait: Duration) -> Self {
        self.step(Step::Fault(Fault::Delay(wait)))
    }

    /// Adds [`Fault::Garbage`].
    #[must_use]
    pub fn garbage(self, bytes: &[u8]) -> Self {
        self.step(Step::Fault(Fault::Garbage(bytes.to_vec())))
    }

    /// Adds the steps of `other` after these.
    #[must_use]
    pub fn then(mut self, other: Self) -> Self {
        self.steps.extend(other.steps);
        self
    }

    /// Adds any step.
    #[must_use]
    pub fn step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// The steps in the order they are played.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}
