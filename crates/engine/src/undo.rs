// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The undo history: each act the owner or an agent makes stores its inverse, and `history.undo` walks back through
//! them, newest first. It lives for the session only and holds at most [`MAX_HISTORY`] acts.

use inboxcraft_model::{EmailAddress, Flag, FolderId, MessageId, MessageState, Sender};

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
    /// Take a screener decision back.
    Screen(Box<Screened>),
}

/// What takes a screener decision on `address` back: the sender as it was `before` (`None`: nothing known of it),
/// each message of `moves` back to its folder, the read mark off `opened` and each message of `states` in its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Screened {
    pub(crate) address: EmailAddress,
    pub(crate) before: Option<Sender>,
    pub(crate) moves: Vec<(MessageId, FolderId)>,
    pub(crate) opened: Option<MessageId>,
    pub(crate) states: Vec<(MessageId, MessageState)>,
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
