// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command line's state: the text typed (at most [`MOST`] characters), what it looks through ([`Mode`]: mail,
//! commands, people or mailboxes, from the first character typed), the row picked in the results, whether the results
//! show, and where the keyboard was when it opened, which gets it back when it closes.
//!
//! `>`, `@` or `#` typed as the first character of an empty search turns it into commands, people or mailboxes: the
//! character becomes the field's mark and leaves the text. Backspace in an empty field drops the mark again
//! ([`Command::drop_scope`]). Nothing here runs anything: the window reads the state and runs what was chosen.

use egui::{Context, Id};

use crate::compose::Suggestion;

/// The most characters the command line holds.
pub const MOST: usize = crate::component::command_field::MOST;

/// What the command line looks through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Plain text: the mail.
    Search,
    /// `>`: the window's commands.
    Command,
    /// `@`: the people the mail knows.
    Person,
    /// `#`: the mailboxes.
    Mailbox,
}

impl Mode {
    /// The mode a first character asks for, if it is a prefix.
    fn of(first: char) -> Option<Self> {
        match first {
            '>' => Some(Self::Command),
            '@' => Some(Self::Person),
            '#' => Some(Self::Mailbox),
            _ => None,
        }
    }
}

/// The command line's field id: the keys and the tests find it by it.
#[must_use]
pub fn field_id() -> Id {
    Id::unique("command-line-field")
}

/// The command line.
#[derive(Debug)]
pub struct Command {
    text: String,
    mode: Mode,
    /// Narrowed to the mailbox shown (`Ctrl F`).
    narrowed: bool,
    picked: usize,
    results: bool,
    /// What had the keyboard when it opened.
    came_from: Option<Id>,
    /// The people found for `people_for`.
    people: Vec<Suggestion>,
    people_for: Option<String>,
    /// The field asks for the keyboard at the next frame ([`Command::open_with`]).
    focus_asked: bool,
    /// The arrows moved the pick since the text changed.
    stepped: bool,
    /// The field had the keyboard when it was last drawn.
    had_focus: bool,
    /// Frames in which a `/` typed is the one that opened the line by its key, not text.
    slash: u8,
}

impl Default for Command {
    fn default() -> Self {
        Self {
            text: String::new(),
            mode: Mode::Search,
            narrowed: false,
            picked: 0,
            results: false,
            came_from: None,
            people: Vec::new(),
            people_for: None,
            focus_asked: false,
            stepped: false,
            had_focus: false,
            slash: 0,
        }
    }
}

impl Command {
    /// Opens the command line empty, its results shown and the keyboard in its field; narrowed to the mailbox shown
    /// for `Ctrl F` (`narrowed_to_mailbox`), over everything for `Ctrl K` and `/`. What had the keyboard gets it back
    /// on [`Command::close`].
    pub fn open(&mut self, ctx: &Context, narrowed_to_mailbox: bool) {
        let focused = ctx.memory(|memory| memory.focused()).filter(|focused| *focused != field_id());
        if !self.has_focus(ctx) {
            self.came_from = focused;
        }
        self.text.clear();
        self.mode = Mode::Search;
        self.narrowed = narrowed_to_mailbox;
        self.picked = 0;
        self.results = true;
        ctx.memory_mut(|memory| memory.request_focus(field_id()));
    }

    /// Opens the command line by the bare key `/`: the `/` that key types is not taken into the field.
    pub fn open_by_slash(&mut self, ctx: &Context) {
        self.open(ctx, false);
        self.slash = 2;
    }

    /// Whether a `/` typed now is the one of the key that opened the line; counts the frames down.
    pub fn slash_pending(&mut self) -> bool {
        let pending = self.slash > 0;
        self.slash = self.slash.saturating_sub(1);
        pending
    }

    /// Opens the command line with `text` typed, from outside a frame (the pictures): the field takes the keyboard at
    /// the next frame.
    pub fn open_with(&mut self, text: &str) {
        self.drop_scope();
        self.set_text(text);
        self.focus_asked = true;
    }

    /// Whether the field asked for the keyboard; asking once.
    pub fn take_focus_asked(&mut self) -> bool {
        std::mem::take(&mut self.focus_asked)
    }

    /// The field took the keyboard by a press: the results show, and nothing is known of where the keyboard was.
    pub fn pressed_open(&mut self) {
        if !self.results {
            self.results = true;
            self.picked = 0;
        }
    }

    /// What the command line looks through: decided by the first character typed.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Whether it is narrowed to the mailbox shown.
    #[must_use]
    pub fn narrowed(&self) -> bool {
        self.narrowed
    }

    /// Whether its field has the keyboard.
    #[must_use]
    pub fn has_focus(&self, ctx: &Context) -> bool {
        ctx.memory(|memory| memory.has_focus(field_id()))
    }

    /// Notes whether the field has the keyboard as it is drawn ([`Command::had_focus`]).
    pub fn keep_focus(&mut self, ctx: &Context) {
        self.had_focus = self.has_focus(ctx);
    }

    /// Whether the field had the keyboard when it was last drawn: egui takes it away before a frame with `Esc`.
    #[must_use]
    pub fn had_focus(&self) -> bool {
        self.had_focus
    }

    /// Whether its results show.
    #[must_use]
    pub fn results(&self) -> bool {
        self.results
    }

    /// Closes the results; the field keeps the keyboard.
    pub fn close_results(&mut self) {
        self.results = false;
        self.picked = 0;
    }

    /// Closes the command line: the results go, a command, a person or a mailbox typed goes (a search stays, as the
    /// list shows it), and the keyboard goes back to where it was when it opened, or leaves the field.
    pub fn close(&mut self, ctx: &Context) {
        self.close_results();
        if self.mode != Mode::Search {
            self.text.clear();
            self.mode = Mode::Search;
        }
        match self.came_from.take() {
            Some(back) => ctx.memory_mut(|memory| memory.request_focus(back)),
            None => ctx.memory_mut(|memory| memory.surrender_focus(field_id())),
        }
    }

    /// The text typed, after the mark.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text, to type in; call [`Command::typed`] after a change.
    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    /// Puts `text` in the field as if typed into an empty one: capped, its first character read for the mode.
    pub fn set_text(&mut self, text: &str) {
        self.mode = Mode::Search;
        text.clone_into(&mut self.text);
        self.typed();
    }

    /// The text changed: cut at [`MOST`] characters; in a search, a first character `>`, `@` or `#` becomes the mark
    /// (the first character only: one typed later is text); the first row is picked and the results show.
    pub fn typed(&mut self) {
        if let Some((cut, _)) = self.text.char_indices().nth(MOST) {
            self.text.truncate(cut);
        }
        if self.mode == Mode::Search
            && let Some(mode) = self.text.chars().next().and_then(Mode::of)
        {
            self.mode = mode;
            self.narrowed = false;
            self.text.remove(0);
        }
        self.picked = 0;
        self.stepped = false;
        self.results = true;
    }

    /// Whether the arrows moved the pick since the text changed: Enter on a bare prefix runs only a row picked so.
    #[must_use]
    pub fn stepped(&self) -> bool {
        self.stepped
    }

    /// Backspace in an empty field: the mark goes, back to a search of everything.
    pub fn drop_scope(&mut self) {
        self.mode = Mode::Search;
        self.narrowed = false;
        self.picked = 0;
    }

    /// The row picked, from the top of the results.
    #[must_use]
    pub fn picked(&self) -> usize {
        self.picked
    }

    /// Moves the pick a row down or up among `count` rows, round from the last to the first and back.
    pub fn step(&mut self, down: bool, count: usize) {
        self.stepped = true;
        self.picked = match (count.checked_sub(1), down) {
            (None, _) => 0,
            (Some(_), true) => self.picked.saturating_add(1) % count,
            (Some(last), false) => self.picked.checked_sub(1).unwrap_or(last).min(last),
        };
    }

    /// The people found for the text typed, when they were looked up for it ([`Command::set_people`]).
    #[must_use]
    pub fn people(&self) -> Option<&[Suggestion]> {
        self.people_for.as_deref().filter(|text| *text == self.text).map(|_| self.people.as_slice())
    }

    /// Keeps the people found for the text typed now.
    pub fn set_people(&mut self, people: Vec<Suggestion>) {
        self.people = people;
        self.people_for = Some(self.text.clone());
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
