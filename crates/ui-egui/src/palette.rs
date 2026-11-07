// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command palette: whether it is open, the text typed into it and the row picked, and which actions match the
//! text. It lists every action of the window ([`keymap::actions`]) by name; typing narrows the list, the arrow keys
//! move the pick and Enter runs it. The text comes from the keyboard and is cut at [`MAX_CHARS`] characters.

use crate::keymap::{self, Action};

/// The most characters the palette's field takes.
pub const MAX_CHARS: usize = 64;

/// The command palette.
#[derive(Debug, Default)]
pub struct Palette {
    open: bool,
    query: String,
    picked: usize,
}

impl Palette {
    /// Whether the palette shows.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the palette empty, the first row picked.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.picked = 0;
    }

    /// Closes the palette; what was typed goes with it.
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.picked = 0;
    }

    /// The text typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The text typed, to type in; call [`Palette::typed`] after a change.
    pub fn query_mut(&mut self) -> &mut String {
        &mut self.query
    }

    /// Puts `text` in the field, cut at [`MAX_CHARS`] characters, and picks the first row.
    pub fn set_query(&mut self, text: &str) {
        self.query = text.chars().take(MAX_CHARS).collect();
        self.typed();
    }

    /// The text changed: the first row is picked.
    pub fn typed(&mut self) {
        self.picked = 0;
    }

    /// The row picked, counted from the top of [`Palette::matches`].
    #[must_use]
    pub fn picked(&self) -> usize {
        self.picked
    }

    /// Moves the pick a row down or up among `count` rows, round from the last to the first and back.
    pub fn step(&mut self, down: bool, count: usize) {
        self.picked = match (count, down) {
            (0, _) => 0,
            (_, true) => self.picked.saturating_add(1) % count,
            (_, false) => self.picked.checked_sub(1).unwrap_or(count - 1).min(count - 1),
        };
    }

    /// The actions that match the text typed.
    #[must_use]
    pub fn matches(&self) -> Vec<Action> {
        matching(&self.query)
    }

    /// The action picked, if a row is.
    #[must_use]
    pub fn chosen(&self) -> Option<Action> {
        self.matches().get(self.picked).copied()
    }
}

/// The actions whose name or id holds every word of `query`, case left aside; those whose name starts with the text
/// first, each group in the order of [`keymap::actions`]. Every action for an empty text.
#[must_use]
pub fn matching(query: &str) -> Vec<Action> {
    let query = query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let (mut first, rest): (Vec<Action>, Vec<Action>) = keymap::actions()
        .into_iter()
        .filter(|action| {
            let name = action.name.to_lowercase();
            words.iter().all(|word| name.contains(word) || action.id.contains(word))
        })
        .partition(|action| action.name.to_lowercase().starts_with(&query));
    first.extend(rest);
    first
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
