// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings sheet: whether it is open and on which page; on the Shortcuts page, the text that narrows the list, the
//! action waiting for a key and what giving the last key changed; on the Sending page, the signatures as typed until
//! they are saved (`compose.signature`); on the Notifications page, why the last change of starting with the computer
//! was not done.
//!
//! The sheet has one open state: every way out (its Close button, Escape, a press beside it, a rail button or a key
//! that shows another place) goes through [`Settings::close`], which forgets everything the open sheet held, so it
//! opens clean the next time.
//!
//! Every other control of the page runs its `ui.*.set` command at once.

use std::collections::BTreeMap;

/// A page of the settings sheet that is built.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    /// Mode, window frame, text size and motion.
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

    /// Opens the sheet on Appearance.
    pub fn open(&mut self) {
        self.open_at(Section::Appearance);
    }

    /// Opens the sheet on `section`.
    pub fn open_at(&mut self, section: Section) {
        self.forget();
        self.open = true;
        self.section = section;
    }

    /// Forgets what the open sheet held: the notes, the text on the Shortcuts page, the action waiting for a key and
    /// the signatures typed and not saved.
    fn forget(&mut self) {
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
}

/// The longest signature, in bytes, as the engine keeps it.
pub const MAX_SIGNATURE_BYTES: usize = 16 * 1024;

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
