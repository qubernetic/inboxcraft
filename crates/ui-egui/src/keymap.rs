// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The keymap: which key runs which action. Actions are named by id, the engine's command ids once they have a key
//! and the window's own (`ui.search.focus`) until then. Everything that shows a key (the key chip of a tooltip, a
//! menu row, a toast's Undo) reads it from here when it is drawn, so a changed key shows at once; one key runs one
//! action.

use egui::{Context, Key, KeyboardShortcut, Modifiers};

/// Focus the search field.
pub const FIND: &str = "ui.search.focus";

/// Show the mail: leave the settings or the add-account page (the rail's Mail button).
pub const MAIL: &str = "ui.mail.show";

/// Open the appearance settings (the rail's settings button).
pub const SETTINGS: &str = "ui.settings.open";

/// Done: mark the selected message read and move it out of the inbox (the sample only, for now).
pub const DONE: &str = "ui.message.done";

/// Mark the selected message read, or unread when it is read (the sample only, for now).
pub const READ: &str = "ui.message.read";

/// Delete the selected message: move it to Trash (the sample only, for now).
pub const DELETE: &str = "ui.message.delete";

/// Undo the last act while its toast shows.
pub const UNDO: &str = "ui.undo";

/// The keys of the actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<(String, KeyboardShortcut)>,
}

impl Default for Keymap {
    /// The keys InboxCraft starts with.
    fn default() -> Self {
        let mut keymap = Self::empty();
        keymap.bind(FIND, KeyboardShortcut::new(Modifiers::COMMAND, Key::F));
        keymap.bind(MAIL, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1));
        keymap.bind(SETTINGS, KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma));
        keymap.bind(DONE, KeyboardShortcut::new(Modifiers::NONE, Key::D));
        keymap.bind(READ, KeyboardShortcut::new(Modifiers::NONE, Key::R));
        keymap.bind(DELETE, KeyboardShortcut::new(Modifiers::NONE, Key::Delete));
        keymap.bind(UNDO, KeyboardShortcut::new(Modifiers::COMMAND, Key::Z));
        keymap
    }
}

impl Keymap {
    /// A keymap without a key.
    #[must_use]
    pub fn empty() -> Self {
        Self { bindings: Vec::new() }
    }

    /// The key of `action`, if it has one.
    #[must_use]
    pub fn shortcut(&self, action: &str) -> Option<KeyboardShortcut> {
        self.bindings.iter().find(|(id, _)| id == action).map(|(_, shortcut)| *shortcut)
    }

    /// Gives `action` the key `shortcut`, in place of its old one; an action that had the key loses it.
    pub fn bind(&mut self, action: &str, shortcut: KeyboardShortcut) {
        self.bindings.retain(|(id, held)| id != action && *held != shortcut);
        self.bindings.push((action.to_owned(), shortcut));
    }

    /// Takes the key from `action`.
    pub fn unbind(&mut self, action: &str) {
        self.bindings.retain(|(id, _)| id != action);
    }

    /// The key of `action` as the platform writes it (`Ctrl+F`, `⌘F`), for a key chip.
    #[must_use]
    pub fn chip(&self, ctx: &Context, action: &str) -> Option<String> {
        self.shortcut(action).map(|shortcut| ctx.format_shortcut(&shortcut))
    }

    /// Whether the key of `action` was pressed in this frame; the press is taken, so nothing else sees it.
    #[must_use]
    pub fn pressed(&self, ctx: &Context, action: &str) -> bool {
        self.shortcut(action).is_some_and(|shortcut| ctx.input_mut(|input| input.consume_shortcut(&shortcut)))
    }
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
