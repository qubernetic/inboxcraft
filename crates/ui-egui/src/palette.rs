// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command list: which actions of the window ([`keymap::actions`]) match a text, by name, for the command line's
//! `>` and the Shortcuts page's search. The command line keeps the text, the pick and whether its results show
//! ([`crate::command`]) and leaves out the actions of another screen. The text is read to its first [`MAX_CHARS`]
//! characters.
//!
//! After the actions it lists the settings' nine pages, "Settings: Accounts" and the like: each [`Row`] runs
//! `ui.settings.open` with its page and has no key of its own.

use std::borrow::Cow;

use inboxcraft_engine::shown::SETTINGS_PAGES;

use crate::iconset::IconId;
use crate::keymap::{self, Action};
use crate::settings::Section;

/// The most characters of a text the list reads.
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
