// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings sheet: whether it is open and on which page; on the Shortcuts page, the text that narrows the list, the
//! action waiting for a key and what giving the last key changed; on the Sending page, the signatures as typed until
//! they are saved (`compose.signature`); on the Notifications page, why the last change of starting with the computer
//! was not done.
//!
//! The sheet has one open state: every way out (its Close button, Escape, a press beside it, a rail button or a key
//! that shows another place) goes through [`Settings::close`], which forgets what the open sheet held (the key waited
//! for, the text that narrows the Shortcuts page, the notes), so it opens clean the next time. What was typed into a
//! setting is never lost that way: a signature typed and not saved stays until it is saved.
//!
//! Every other control of the page runs its `ui.*.set` command at once.

use std::collections::BTreeMap;

/// A page of the settings sheet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    /// Look (mode, motion, text size) and Layout (the inbox view, where a message opens and is written, the window
    /// frame).
    #[default]
    Appearance,
    /// The accounts, and adding one.
    Accounts,
    /// Which new mail of each account notifies, returned snoozes and reminders, running in the background and
    /// starting with the computer.
    Notifications,
    /// The screener's mode, pins on done mail, and each account's grouping.
    Triage,
    /// Undo send, the signature of each account's new messages, the templates and the scheduled list.
    Sending,
    /// The calendar's alerts and each account's calendars; coming soon.
    Calendar,
    /// Every action of the window with its key, to give it another.
    Shortcuts,
    /// The senders whose remote pictures always load; coming soon.
    Privacy,
    /// The program, its licences and its makers; coming soon.
    About,
    /// A model that summarises and drafts; planned, coming soon.
    Ai,
    /// Plugins; planned, coming soon.
    Plugins,
}

impl Section {
    /// Every section, in the order the sheet lists them: the nine of the design, then the two planned ones.
    pub const ALL: [Self; 11] = [
        Self::Appearance,
        Self::Accounts,
        Self::Notifications,
        Self::Triage,
        Self::Sending,
        Self::Calendar,
        Self::Shortcuts,
        Self::Privacy,
        Self::About,
        Self::Ai,
        Self::Plugins,
    ];

    /// Its name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Appearance => crate::tr!("Appearance"),
            Self::Accounts => crate::tr!("Accounts"),
            Self::Notifications => crate::tr!("Notifications"),
            Self::Triage => crate::tr!("Triage"),
            Self::Sending => crate::tr!("Sending"),
            Self::Calendar => crate::tr!("Calendar"),
            Self::Shortcuts => crate::tr!("Shortcuts"),
            Self::Privacy => crate::tr!("Privacy"),
            Self::About => crate::tr!("About"),
            Self::Ai => crate::tr!("AI"),
            Self::Plugins => crate::tr!("Plugins"),
        }
    }

    /// Its name in scripts (the `ui_shot` examples): `appearance`, `accounts` … `ai`, `plugins`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Accounts => "accounts",
            Self::Notifications => "notifications",
            Self::Triage => "triage",
            Self::Sending => "sending",
            Self::Calendar => "calendar",
            Self::Shortcuts => "shortcuts",
            Self::Privacy => "privacy",
            Self::About => "about",
            Self::Ai => "ai",
            Self::Plugins => "plugins",
        }
    }

    /// The section [`Section::id`] calls `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.id() == name)
    }

    /// Whether the whole section is planned and not built: its page carries the coming-soon mark on its head.
    #[must_use]
    pub fn planned(self) -> bool {
        matches!(self, Self::About | Self::Ai | Self::Plugins)
    }
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
    /// The signatures as typed on the Sending page and not saved yet, by account, kept while the sheet is closed; an
    /// account not in it shows the one it has.
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

    /// Forgets what the open sheet held: the notes, the text on the Shortcuts page and the action waiting for a key.
    /// The signatures typed and not saved stay: what a person wrote is never lost on the way out.
    fn forget(&mut self) {
        self.capturing = None;
        self.find.clear();
        self.keys_note = None;
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
    /// changed. It is never cut: one longer than the field holds opens whole, under the field's opening-size limit.
    pub fn signature_mut(&mut self, account: u64, kept: &str) -> &mut String {
        self.signatures.entry(account).or_insert_with(|| kept.to_owned())
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

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
