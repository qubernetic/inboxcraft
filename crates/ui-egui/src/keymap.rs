// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The keymap: which key runs which action. Actions are named by id, the engine's command ids once they have a key
//! and the window's own (`ui.search.focus`) until then. Everything that shows a key (the key chip of a tooltip, a
//! menu row, a toast's Undo) reads it from here when it is drawn, so a changed key shows at once; one key runs one
//! action.
//!
//! Every action of the window is in [`actions`], with its name, so the command palette lists them all and the
//! Shortcuts page gives each one a key. The keymap is kept between runs as [`Keymap::to_value`]: every action's key
//! written as text ([`written`]), or `null` for none; [`Keymap::from_value`] reads that as outside data.

use std::collections::BTreeMap;

use egui::{Context, Event, Key, KeyboardShortcut, Modifiers};
use serde_json::{Map, Value};

use crate::iconset::IconId;
use crate::tr;

/// Focus the search field.
pub const FIND: &str = "ui.search.focus";

/// Show the mail: leave the settings or the add-account page (the rail's Mail button).
pub const MAIL: &str = "ui.mail.show";

/// Open the appearance settings (the rail's settings button).
pub const SETTINGS: &str = "ui.settings.open";

/// Done: mark the selected message read and move it to its account's Archive.
pub const DONE: &str = "ui.message.done";

/// Open the time picker over the open message: snooze it, or set the reminder of a message the owner sent.
pub const SNOOZE: &str = "ui.message.snooze";

/// Move the selected message to its account's Archive as it is.
pub const ARCHIVE: &str = "ui.message.archive";

/// Mark the selected message read, or unread when it is read.
pub const READ: &str = "ui.message.read";

/// Flag the selected message, or take its flag off when it has one.
pub const FLAG: &str = "ui.message.flag";

/// Delete the selected message: move it to Trash, or delete it for good when it is in the Trash.
pub const DELETE: &str = "ui.message.delete";

/// Undo the act of the toast while it shows, else the newest act of the session.
pub const UNDO: &str = "ui.undo";

/// Open the command palette.
pub const PALETTE: &str = "ui.palette.open";

/// Show the calendar (the rail's Calendar button).
pub const CALENDAR: &str = "ui.calendar.show";

/// Show the sent mail of every account (the rail's Sent mail button).
pub const SENT: &str = "ui.sent.show";

/// Show "Later": the snoozed mail, the pins and the park pile (the rail's Later button).
pub const LATER: &str = "ui.later.show";

/// Open the settings on the Shortcuts page.
pub const SHORTCUTS: &str = "ui.shortcuts.open";

/// Let the open message fill the window, or give the list its place back.
pub const FULL_WIDTH: &str = "ui.reader.full_width";

/// Always show light (`ui.theme.set`).
pub const LIGHT: &str = "ui.mode.light";

/// Always show dark (`ui.theme.set`).
pub const DARK: &str = "ui.mode.dark";

/// Show what the system shows (`ui.theme.set`).
pub const SYSTEM_MODE: &str = "ui.mode.system";

/// Back to the list: in one pane the list takes the open message's place again, its keyed row focused.
pub const BACK: &str = "ui.reader.back";

/// Show the inbox grouped: every group open.
pub const VIEW_GROUPED: &str = "ui.view.grouped";

/// Show the inbox smart: the first three rows of each group.
pub const VIEW_SMART: &str = "ui.view.smart";

/// Show the inbox as one list by time.
pub const VIEW_LIST: &str = "ui.view.list";

/// Turn reduced motion on or off (`ui.motion.set`).
pub const MOTION: &str = "ui.motion.toggle";

/// Write a new message (`compose.new`), or bring back the one being written.
pub const COMPOSE: &str = "ui.compose.new";

/// Send the message being written (`mail.send`).
pub const SEND: &str = "ui.compose.send";

/// After the open message left the list, open the message the next-message rule names (the next or the previous
/// one). No key of its own: the palette runs it.
pub const OPEN_HELD: &str = "ui.reader.open_held";

/// Move the list's focused row a page down: by the rows in sight.
pub const PAGE_DOWN: &str = "ui.list.page_down";

/// Move the list's focused row a page up.
pub const PAGE_UP: &str = "ui.list.page_up";

/// Move the list's focused row to the first row.
pub const FIRST: &str = "ui.list.first";

/// Move the list's focused row to the last row.
pub const LAST: &str = "ui.list.last";

/// Move the list's keyed row one down: the next message.
pub const NEXT: &str = "ui.list.next";

/// Move the list's keyed row one up: the message before.
pub const PREVIOUS: &str = "ui.list.previous";

/// The next message by the arrow: [`NEXT`]'s second key.
pub const DOWN: &str = "ui.list.down";

/// The message before by the arrow: [`PREVIOUS`]'s second key.
pub const UP: &str = "ui.list.up";

/// Open the list's keyed message and hand the keyboard to the reader; `Esc` there gives it back to the list.
pub const OPEN: &str = "ui.list.open";

/// Open the menu of what has the keyboard: the group menu of the list's keyed row, or the menu of the reader's
/// focused line or subject.
pub const MENU: &str = "ui.list.menu";

/// The same menu by the keyboard's Menu key: [`MENU`]'s second key.
pub const MENU_KEY: &str = "ui.list.menu_key";

/// The actions of the list's keys: they run only while the list has the keyboard (a row was pressed or keyed last
/// and nothing else has the focus, or a row has it), and rest otherwise, in a text field as in the reader.
pub const LIST_KEYS: [&str; 11] = [NEXT, PREVIOUS, DOWN, UP, PAGE_DOWN, PAGE_UP, FIRST, LAST, OPEN, MENU, MENU_KEY];

/// The longest key text read, in bytes; `Command+Shift+Alt+ArrowRight` takes 28.
pub const MAX_TEXT: usize = 64;

/// An action of the window: its id, its name as the palette and the Shortcuts page show it, and its icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// The id the keymap knows it by.
    pub id: &'static str,
    /// Its name.
    pub name: &'static str,
    /// Its icon in the palette.
    pub icon: IconId,
}

/// Every action of the window, in the order the palette and the Shortcuts page list them.
#[must_use]
pub fn actions() -> Vec<Action> {
    let action = |id, name, icon| Action { id, name, icon };
    vec![
        action(PALETTE, tr!("Command palette"), IconId::Keyboard),
        action(FIND, tr!("Search mail"), IconId::Search),
        action(MAIL, tr!("Show mail"), IconId::Inbox),
        action(SENT, tr!("Show sent mail"), IconId::Send),
        action(LATER, tr!("Show later: snoozed, pins and parked"), IconId::Pin),
        action(COMPOSE, tr!("New message"), IconId::Compose),
        action(SEND, tr!("Send the message"), IconId::Send),
        action(CALENDAR, tr!("Show the calendar"), IconId::Calendar),
        action(OPEN_HELD, tr!("Open the message the rule names"), IconId::Mail),
        action(DONE, tr!("Done"), IconId::Done),
        action(ARCHIVE, tr!("Archive"), IconId::Archive),
        action(SNOOZE, tr!("Snooze or remind me"), IconId::Snooze),
        action(READ, tr!("Mark read or unread"), IconId::MarkRead),
        action(FLAG, tr!("Flag or take the flag off"), IconId::Flag),
        action(DELETE, tr!("Delete"), IconId::Delete),
        action(NEXT, tr!("The next message of the list"), IconId::Caret),
        action(PREVIOUS, tr!("The message before in the list"), IconId::Caret),
        action(DOWN, tr!("The next message of the list (the arrow)"), IconId::Caret),
        action(UP, tr!("The message before in the list (the arrow)"), IconId::Caret),
        action(OPEN, tr!("Open the list's message and read it"), IconId::Mail),
        action(MENU, tr!("The menu of the list's row, the reader's line or its subject"), IconId::More),
        action(MENU_KEY, tr!("The menu of the list's row, the reader's line or its subject (the Menu key)"), IconId::More),
        action(PAGE_DOWN, tr!("A page down the list"), IconId::Caret),
        action(PAGE_UP, tr!("A page up the list"), IconId::Caret),
        action(FIRST, tr!("The first message of the list"), IconId::Caret),
        action(LAST, tr!("The last message of the list"), IconId::Caret),
        action(UNDO, tr!("Undo"), IconId::Back),
        action(FULL_WIDTH, tr!("Reader full width"), IconId::Mail),
        action(BACK, tr!("Back to the list"), IconId::Back),
        action(SETTINGS, tr!("Settings"), IconId::Settings),
        action(SHORTCUTS, tr!("Shortcuts"), IconId::Keyboard),
        action(LIGHT, tr!("Light mode"), IconId::Palette),
        action(DARK, tr!("Dark mode"), IconId::Palette),
        action(SYSTEM_MODE, tr!("Mode of the system"), IconId::Palette),
        action(MOTION, tr!("Reduce motion on or off"), IconId::Palette),
        action(VIEW_GROUPED, tr!("Inbox view: Grouped"), IconId::Inbox),
        action(VIEW_SMART, tr!("Inbox view: Smart"), IconId::Inbox),
        action(VIEW_LIST, tr!("Inbox view: List"), IconId::Inbox),
    ]
}

/// The action `id`, if the window has it.
#[must_use]
pub fn action(id: &str) -> Option<Action> {
    actions().into_iter().find(|action| action.id == id)
}

/// The modifiers as they are written, in the order they are written.
const MODIFIER_NAMES: [&str; 5] = ["Command", "Ctrl", "MacCmd", "Alt", "Shift"];

/// `shortcut` as the keymap's file writes it: the modifiers, then the key, joined by `+` (`Command+Shift+K`). Command
/// is the platform's command key (Ctrl, or ⌘ on a Mac) and stands for Ctrl or ⌘ when it is there.
#[must_use]
pub fn written(shortcut: KeyboardShortcut) -> String {
    let m = shortcut.modifiers;
    let held = [m.command, m.ctrl && !m.command, m.mac_cmd && !m.command, m.alt, m.shift];
    let mut parts: Vec<&str> = MODIFIER_NAMES.iter().zip(held).filter(|(_, on)| *on).map(|(name, _)| *name).collect();
    parts.push(shortcut.logical_key.name());
    parts.join("+")
}

/// The shortcut `text` writes as [`written`] does; modifier names in any case and space around the parts are taken.
/// `None` for anything else, and for a text longer than [`MAX_TEXT`].
#[must_use]
pub fn read(text: &str) -> Option<KeyboardShortcut> {
    if text.len() > MAX_TEXT {
        return None;
    }
    let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let key = Key::from_name(parts.pop()?)?;
    let mut modifiers = Modifiers::NONE;
    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "command" => modifiers.command = true,
            "ctrl" => modifiers.ctrl = true,
            "maccmd" => modifiers.mac_cmd = true,
            "alt" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            _ => return None,
        }
    }
    if modifiers.command {
        // Command is Ctrl or ⌘ already.
        modifiers.ctrl = false;
        modifiers.mac_cmd = false;
    }
    Some(KeyboardShortcut::new(modifiers, key))
}

/// What the next key press was, while the Shortcuts page waits for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captured {
    /// A key, with the modifiers held: the platform's command key as Command.
    Key(KeyboardShortcut),
    /// Escape: no key is given.
    Cancel,
}

/// The first key pressed in this frame (not a repeat), taken from the input so nothing else sees it.
#[must_use]
pub fn captured(ctx: &Context) -> Option<Captured> {
    ctx.input_mut(|input| {
        let at = input.events.iter().position(|event| matches!(event, Event::Key { pressed: true, repeat: false, .. }))?;
        let Event::Key { key, modifiers, .. } = input.events.remove(at) else { return None };
        if key == Key::Escape {
            return Some(Captured::Cancel);
        }
        let modifiers = if modifiers.command { Modifiers { alt: modifiers.alt, shift: modifiers.shift, ..Modifiers::COMMAND } } else { modifiers };
        Some(Captured::Key(KeyboardShortcut::new(modifiers, key)))
    })
}

/// The keys of the actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    /// Each action with a key, by id.
    bindings: BTreeMap<String, KeyboardShortcut>,
}

impl Default for Keymap {
    /// The keys InboxCraft starts with.
    fn default() -> Self {
        let mut keymap = Self::empty();
        keymap.bind(FIND, KeyboardShortcut::new(Modifiers::COMMAND, Key::F));
        keymap.bind(MAIL, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1));
        keymap.bind(SETTINGS, KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma));
        keymap.bind(DONE, KeyboardShortcut::new(Modifiers::NONE, Key::D));
        keymap.bind(ARCHIVE, KeyboardShortcut::new(Modifiers::NONE, Key::A));
        keymap.bind(SNOOZE, KeyboardShortcut::new(Modifiers::NONE, Key::H));
        keymap.bind(READ, KeyboardShortcut::new(Modifiers::NONE, Key::R));
        keymap.bind(FLAG, KeyboardShortcut::new(Modifiers::NONE, Key::S));
        keymap.bind(DELETE, KeyboardShortcut::new(Modifiers::NONE, Key::Delete));
        keymap.bind(UNDO, KeyboardShortcut::new(Modifiers::COMMAND, Key::Z));
        keymap.bind(PALETTE, KeyboardShortcut::new(Modifiers::COMMAND, Key::K));
        keymap.bind(CALENDAR, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2));
        keymap.bind(COMPOSE, KeyboardShortcut::new(Modifiers::NONE, Key::C));
        keymap.bind(SEND, KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter));
        keymap.bind(NEXT, KeyboardShortcut::new(Modifiers::NONE, Key::J));
        keymap.bind(PREVIOUS, KeyboardShortcut::new(Modifiers::NONE, Key::K));
        keymap.bind(DOWN, KeyboardShortcut::new(Modifiers::NONE, Key::ArrowDown));
        keymap.bind(UP, KeyboardShortcut::new(Modifiers::NONE, Key::ArrowUp));
        keymap.bind(OPEN, KeyboardShortcut::new(Modifiers::NONE, Key::Enter));
        keymap.bind(MENU, KeyboardShortcut::new(Modifiers::SHIFT, Key::F10));
        keymap.bind(MENU_KEY, KeyboardShortcut::new(Modifiers::NONE, Key::Menu));
        keymap.bind(PAGE_DOWN, KeyboardShortcut::new(Modifiers::NONE, Key::PageDown));
        keymap.bind(PAGE_UP, KeyboardShortcut::new(Modifiers::NONE, Key::PageUp));
        keymap.bind(FIRST, KeyboardShortcut::new(Modifiers::NONE, Key::Home));
        keymap.bind(LAST, KeyboardShortcut::new(Modifiers::NONE, Key::End));
        keymap
    }
}

impl Keymap {
    /// A keymap without a key.
    #[must_use]
    pub fn empty() -> Self {
        Self { bindings: BTreeMap::new() }
    }

    /// The key of `action`, if it has one.
    #[must_use]
    pub fn shortcut(&self, action: &str) -> Option<KeyboardShortcut> {
        self.bindings.get(action).copied()
    }

    /// The action that has the key `shortcut`, if one has it.
    #[must_use]
    pub fn action_of(&self, shortcut: KeyboardShortcut) -> Option<&str> {
        self.bindings.iter().find(|(_, held)| **held == shortcut).map(|(id, _)| id.as_str())
    }

    /// Every action's key, to keep: `{"keys": {"ui.search.focus": "Command+F", "ui.sent.show": null, ...}}`.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let keys: Map<String, Value> = actions()
            .into_iter()
            .map(|action| (action.id.to_owned(), self.shortcut(action.id).map_or(Value::Null, |key| Value::from(written(key)))))
            .collect();
        serde_json::json!({ "keys": keys })
    }

    /// The keymap `value` keeps, as outside data: the defaults, then each known action's key from `value` (`null`
    /// takes it away). Unknown actions, keys that cannot be read and values of the wrong type are left out.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let mut keymap = Self::default();
        let Some(keys) = value.get("keys").and_then(Value::as_object) else { return keymap };
        for action in actions() {
            match keys.get(action.id) {
                Some(Value::Null) => keymap.unbind(action.id),
                Some(Value::String(text)) => {
                    if let Some(shortcut) = read(text) {
                        keymap.bind(action.id, shortcut);
                    }
                }
                _ => {}
            }
        }
        keymap
    }

    /// Gives `action` the key `shortcut`, in place of its old one; an action that had the key loses it.
    pub fn bind(&mut self, action: &str, shortcut: KeyboardShortcut) {
        self.bindings.retain(|_, held| *held != shortcut);
        self.bindings.insert(action.to_owned(), shortcut);
    }

    /// Takes the key from `action`.
    pub fn unbind(&mut self, action: &str) {
        self.bindings.remove(action);
    }

    /// The key of `action` written out in words for a key chip (`Ctrl F`, `Del`, `↓`); `None` when it has none.
    #[must_use]
    pub fn chip(&self, _ctx: &Context, action: &str) -> Option<String> {
        self.shortcut(action).map(|shortcut| crate::component::key::key_words(&shortcut))
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
