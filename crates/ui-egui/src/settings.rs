// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings sheet: whether it is open and on which page, the four colours of the custom accent as the owner types them,
//! and what the engine said about the last ones it took; on the Shortcuts page, the text that narrows the list, the
//! action waiting for a key and what giving the last key changed; on the Sending page, the signatures as typed until
//! they are saved (`compose.signature`); on the Notifications page, why the last change of starting with the computer
//! was not done.
//!
//! The sheet has one open state: every way out (its Close button, Escape, a press beside it, a rail button or a key
//! that shows another place) goes through [`Settings::close`], which forgets everything the open sheet held, so it
//! opens clean the next time.
//!
//! Every other control of the page runs its `ui.*.set` command at once; the custom accent is a draft the page's
//! preview shows until "Use these colours" runs `ui.accent.set` with it. The colours come from the keyboard or the
//! colour picker, so they are read as hostile: only `#rrggbb` (the `#` may be left out, space around it is dropped)
//! is a colour, and a field holds at most [`MAX_CHARS`] characters.

use std::collections::BTreeMap;

use egui::Color32;
use inboxcraft_engine::appearance::{Accent, Appearance, Rgb};
use serde_json::Value;

use crate::theme::Preset;
use crate::tr;

/// The most characters a colour field takes.
pub const MAX_CHARS: usize = 16;

/// A page of the settings sheet that is built.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    /// Mode, accent, typeface, icons, window frame, text size and motion.
    #[default]
    Appearance,
    /// The accounts, and adding one.
    Accounts,
    /// Every action of the window with its key, to give it another.
    Shortcuts,
    /// The signature of each account's new messages.
    Sending,
    /// The screener's mode, how the inbox shows its groups, and each account's grouping.
    Triage,
    /// Which new mail of each account notifies, returned snoozes and reminders, running in the background and
    /// starting with the computer.
    Notifications,
}

/// The settings sheet.
#[derive(Debug, Default)]
pub struct Settings {
    open: bool,
    section: Section,
    colours: [String; 4],
    note: Option<String>,
    /// The action waiting for a key on the Shortcuts page.
    capturing: Option<&'static str>,
    /// The text that narrows the Shortcuts page's list.
    find: String,
    /// What giving the last key changed, on the Shortcuts page.
    keys_note: Option<String>,
    /// The signatures as typed on the Sending page, by account; an account not in it shows the one it has.
    signatures: BTreeMap<u64, String>,
    /// Why the last change of starting with the computer was not done, on the Notifications page.
    login_note: Option<String>,
}

/// `colour` as `#rrggbb`.
pub(crate) fn hex(colour: Color32) -> String {
    Rgb(colour.r(), colour.g(), colour.b()).hex()
}

/// The colour `text` names: `#rrggbb` or `rrggbb`, space around it dropped.
#[must_use]
pub fn colour(text: &str) -> Option<Color32> {
    crate::theme::typed(text)
}

impl Settings {
    /// Whether the page is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The page shown.
    #[must_use]
    pub fn section(&self) -> Section {
        self.section
    }

    /// Opens the sheet on Appearance with the colours of `appearance` in the custom accent's fields.
    pub fn open(&mut self, appearance: &Appearance) {
        self.open_at(Section::Appearance, appearance);
    }

    /// Opens the sheet on `section`, with the colours of `appearance` in the custom accent's fields.
    pub fn open_at(&mut self, section: Section, appearance: &Appearance) {
        self.forget();
        self.open = true;
        self.section = section;
        self.fill(appearance);
    }

    /// Forgets what the open sheet held: the notes, the text on the Shortcuts page, the action waiting for a key and
    /// the signatures typed and not saved.
    fn forget(&mut self) {
        self.note = None;
        self.capturing = None;
        self.find.clear();
        self.keys_note = None;
        self.signatures.clear();
        self.login_note = None;
    }

    /// Why the last change of starting with the computer was not done, while the sheet is open.
    #[must_use]
    pub fn login_note(&self) -> Option<&str> {
        self.login_note.as_deref()
    }

    /// Says why the last change of starting with the computer was not done; `None` when it was.
    pub fn set_login_note(&mut self, note: Option<String>) {
        self.login_note = note;
    }

    /// The signature of `account` as typed on the Sending page, to type in: `kept`, the one it has, until it is
    /// changed. It is cut to what the engine keeps.
    pub fn signature_mut(&mut self, account: u64, kept: &str) -> &mut String {
        let typed = self.signatures.entry(account).or_insert_with(|| kept.to_owned());
        crate::compose::cut(typed, MAX_SIGNATURE_BYTES);
        typed
    }

    /// The signature of `account` as typed, if it was typed since the page opened.
    #[must_use]
    pub fn signature(&self, account: u64) -> Option<&str> {
        self.signatures.get(&account).map(String::as_str)
    }

    /// The signature of `account` was saved: the page shows the one it has from now on.
    pub fn saved_signature(&mut self, account: u64) {
        self.signatures.remove(&account);
    }

    /// Shows `section` on the open sheet; an action waiting for a key stops waiting.
    pub fn show(&mut self, section: Section) {
        self.section = section;
        self.capturing = None;
    }

    /// Closes the sheet and forgets what it held.
    pub fn close(&mut self) {
        self.open = false;
        self.forget();
    }

    /// The action waiting for a key, if one is.
    #[must_use]
    pub fn capturing(&self) -> Option<&'static str> {
        self.capturing
    }

    /// `action` waits for the next key pressed.
    pub fn capture(&mut self, action: &'static str) {
        self.capturing = Some(action);
        self.keys_note = None;
    }

    /// No action waits for a key any more; `note` says what the key given changed, when it changed something.
    pub fn captured(&mut self, note: Option<String>) {
        self.capturing = None;
        self.keys_note = note;
    }

    /// The text that narrows the Shortcuts page's list.
    #[must_use]
    pub fn find(&self) -> &str {
        &self.find
    }

    /// The text that narrows the Shortcuts page's list, to type in.
    pub fn find_mut(&mut self) -> &mut String {
        &mut self.find
    }

    /// What giving the last key changed.
    #[must_use]
    pub fn keys_note(&self) -> Option<&str> {
        self.keys_note.as_deref()
    }

    /// The fields take the colours of `appearance`, after a preset was chosen: no draft is left to show.
    pub fn follow(&mut self, appearance: &Appearance) {
        self.fill(appearance);
        self.note = None;
    }

    /// Puts the colours of `appearance` in the fields.
    fn fill(&mut self, appearance: &Appearance) {
        let preset = Preset::of(&appearance.accent);
        let [first, middle, last] = preset.gradient;
        self.colours = [hex(first), hex(middle), hex(last), hex(preset.accent)];
    }

    /// The four fields as typed: the gradient's three stops and the accent.
    #[must_use]
    pub fn colours(&self) -> &[String; 4] {
        &self.colours
    }

    /// The four fields, to type in.
    pub fn colours_mut(&mut self) -> &mut [String; 4] {
        &mut self.colours
    }

    /// The colours the fields name, as a preset called `custom`; `None` while one of them is not a colour.
    #[must_use]
    pub fn draft(&self) -> Option<Preset> {
        let [first, middle, last, accent] = &self.colours;
        Some(Preset { id: "custom", gradient: [colour(first)?, colour(middle)?, colour(last)?], accent: colour(accent)? })
    }

    /// The draft as the parameters of `ui.accent.set`: four `#rrggbb` colours, the accent last.
    #[must_use]
    pub fn custom(&self) -> Option<[String; 4]> {
        let draft = self.draft()?;
        let [first, middle, last] = draft.gradient;
        Some([hex(first), hex(middle), hex(last), hex(draft.accent)])
    }

    /// What the engine said about the colours it took last, when it changed one.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Takes `answer`, the engine's answer to `ui.accent.set`, and `appearance` after it: the fields show the colours
    /// in use, and a note says so when the engine made the accent lighter.
    pub fn took(&mut self, answer: &Value, appearance: &Appearance) {
        self.fill(appearance);
        let requested = answer.get("requested").and_then(Value::as_str);
        self.note = match (answer.get("nudged").and_then(Value::as_bool), requested, appearance.accent) {
            (Some(true), Some(requested), Accent::Custom(custom)) => Some(tr!(
                "The accent {requested} is hard to read as text on dark surfaces, so {accent} is used instead.",
                requested = requested,
                accent = custom.accent.hex()
            )),
            _ => None,
        };
    }
}

/// The longest signature, in bytes, as the engine keeps it.
pub const MAX_SIGNATURE_BYTES: usize = 16 * 1024;

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
