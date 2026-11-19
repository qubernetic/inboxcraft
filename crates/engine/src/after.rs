// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What follows an act that takes the open message out of the list (Done, Archive, Delete, Snooze, Move): the owner's
//! choice, set by `after.set` (`owner`). The session keeps it; the front end keeps it on disk and hands it back at the
//! start ([`crate::Session::set_after`]).

/// What follows an act on the open message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum After {
    /// The message below it, the next older one (the one above when it was the last), opened.
    #[default]
    Next,
    /// The message above it, the next newer one (the one below when it was the first), opened.
    Previous,
    /// Nothing: back to the list.
    List,
    /// The row that took its place has the keyboard, and nothing is opened.
    Stay,
}

impl After {
    /// Every choice, in the order the settings show them.
    pub const ALL: [Self; 4] = [Self::Next, Self::Previous, Self::List, Self::Stay];

    /// The choice's name in `after.set` and on disk.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Next => "next",
            Self::Previous => "previous",
            Self::List => "list",
            Self::Stay => "stay",
        }
    }

    /// The choice named `name`; none for any other text.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|after| after.name() == name)
    }
}
