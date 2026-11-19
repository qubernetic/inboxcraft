// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command palette: whether it is open, the text typed into it and the row picked, and which actions match the
//! text. It lists every action of the window ([`keymap::actions`]) by name; typing narrows the list, the arrow keys
//! move the pick and Enter runs it; an action of another screen (the calendar's arrows on the mail) is not listed. The
//! text comes from the keyboard and is cut at [`MAX_CHARS`] characters.
//!
//! After the actions it lists the settings' nine pages, "Settings: Accounts" and the like: each [`Row`] runs
//! `ui.settings.open` with its page and has no key of its own.

use std::borrow::Cow;

use inboxcraft_engine::shown::SETTINGS_PAGES;

use crate::iconset::IconId;
use crate::keymap::{self, Action};
use crate::settings::Section;

/// The most characters the palette's field takes.
pub const MAX_CHARS: usize = 64;

/// A row of the palette: an action of the window, or a page of the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The action it runs: for a page, `ui.settings.open`.
    pub id: &'static str,
    /// Its name.
    pub name: Cow<'static, str>,
    /// Its icon.
    pub icon: IconId,
    /// The page `ui.settings.open` shows ([`Section::id`]); `None` for an action, which runs as its key does.
    pub page: Option<&'static str>,
}

impl Row {
    fn action(action: Action) -> Self {
        Self { id: action.id, name: Cow::Borrowed(action.name), icon: action.icon, page: None }
    }
}

/// The settings' pages as rows, in the settings' order: "Settings: Accounts" runs `ui.settings.open` on `accounts`.
fn pages() -> impl Iterator<Item = Row> {
    SETTINGS_PAGES.into_iter().filter_map(|page| {
        let section = Section::named(page)?;
        let name = crate::tr!("Settings: {page}", page = section.name());
        Some(Row { id: keymap::SETTINGS, name: Cow::Owned(name), icon: IconId::Settings, page: Some(page) })
    })
}

/// The command palette.
#[derive(Debug)]
pub struct Palette {
    open: bool,
    query: String,
    picked: usize,
    /// The screen it opens over: an action of another screen ([`keymap::Scope`]) is not listed.
    screen: keymap::Scope,
}

impl Default for Palette {
    fn default() -> Self {
        Self { open: false, query: String::new(), picked: 0, screen: keymap::Scope::Window }
    }
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

    /// Lists the actions of `screen` and the window's (the window's own scope lists every action).
    pub fn set_screen(&mut self, screen: keymap::Scope) {
        self.screen = screen;
    }

    /// The rows that match the text typed, of the window and of the screen it opens over.
    #[must_use]
    pub fn matches(&self) -> Vec<Row> {
        rows(&self.query).into_iter().filter(|row| keymap::scope(row.id).meets(self.screen)).collect()
    }

    /// The row picked, if one is.
    #[must_use]
    pub fn chosen(&self) -> Option<Row> {
        self.matches().into_iter().nth(self.picked)
    }
}

/// The rows whose name, or the action's id, or the page's, holds every word of `query` (its first [`MAX_CHARS`]
/// characters), case left aside: the actions, then the settings' pages, those whose name starts with the text first.
#[must_use]
pub fn rows(query: &str) -> Vec<Row> {
    let query: String = query.chars().take(MAX_CHARS).collect::<String>().trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    // The command line's bare key opens what the palette's key opens: one row for the two.
    let all = keymap::actions().into_iter().filter(|action| action.id != keymap::COMMAND_LINE).map(Row::action).chain(pages());
    let (mut first, rest): (Vec<Row>, Vec<Row>) = all
        .filter(|row| {
            let name = row.name.to_lowercase();
            let id = row.page.unwrap_or(row.id);
            words.iter().all(|word| name.contains(word) || id.contains(word))
        })
        .partition(|row| row.name.to_lowercase().starts_with(&query));
    first.extend(rest);
    first
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
