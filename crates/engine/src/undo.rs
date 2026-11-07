// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The undo history: each act the owner or an agent makes stores its inverse, and `history.undo` walks back through
//! them, newest first. It lives for the session only and holds at most [`MAX_HISTORY`] acts.

use inboxcraft_model::{Flag, FolderId, MessageId};

use crate::AgentClass;

/// The most acts the history holds; the oldest goes first.
pub(crate) const MAX_HISTORY: usize = 100;

/// One act in the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Act {
    /// The command that made it.
    pub(crate) command: &'static str,
    /// Its class: an agent undoes only what an agent may do.
    pub(crate) class: AgentClass,
    /// What takes it back.
    pub(crate) inverse: Inverse,
}

/// What takes an act back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inverse {
    /// Set (`set`) or clear `flag` on `message` again.
    Mark { message: MessageId, flag: Flag, set: bool },
    /// Move `message` back to `to`.
    Move { message: MessageId, to: FolderId },
}

impl Inverse {
    /// The message it is about.
    pub(crate) fn message(&self) -> MessageId {
        match self {
            Self::Mark { message, .. } | Self::Move { message, .. } => *message,
        }
    }
}

/// What a command does to the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// Adds its act on top.
    Push(Act),
    /// Takes the act on top off: it was undone, or there is nothing left to undo it on.
    Pop,
}

/// `history` after `step`.
pub(crate) fn take(history: &mut Vec<Act>, step: Step) {
    match step {
        Step::Push(act) => {
            history.push(act);
            if history.len() > MAX_HISTORY {
                history.remove(0);
            }
        }
        Step::Pop => {
            history.pop();
        }
    }
}
