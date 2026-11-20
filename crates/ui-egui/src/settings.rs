// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings, a place of the window: whether they are the place shown and on which page; on the Shortcuts page, the
//! text that narrows the list, the action waiting for a key and what giving the last key changed; on the Sending page,
//! the signatures as typed until they are saved (`compose.signature`); on the Notifications page, why the last change
//! of starting with the computer was not done.
//!
//! A place is not closed: every way to another place (its key or the index's SETTINGS pressed again, a key or a row of
//! the index that shows another place) goes through [`Settings::leave`], which ends a key being waited for and drops
//! the notes on what the last change did, and keeps the rest: the page, the text that narrows the Shortcuts page, a
//! signature typed and not saved. Coming back shows the settings as they were left.
//!
//! Every other control of the page runs its `ui.*.set` command at once.

use std::collections::BTreeMap;

/// A page of the settings.
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
    /// Every section, in the order the settings list them: the nine of the design, then the two planned ones.
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

/// The settings place.
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
    /// The signatures as typed on the Sending page and not saved yet, by account, kept while another place is shown;
    /// an account not in it shows the one it has.
    signatures: BTreeMap<u64, String>,
    /// Why the last change of starting with the computer was not done, on the Notifications page.
    login_note: Option<String>,
    /// The page under the section shown, if one is.
    under: Option<Under>,
    /// The Templates page's state.
    templates: crate::templates::Templates,
    /// The Scheduled page's state.
    scheduled: crate::scheduled::Scheduled,
    /// The Privacy page's state.
    privacy: crate::privacy::Privacy,
    /// The keyboard goes to the shown section's row of the settings' own column when it is drawn next.
    nav_hand: bool,
}

/// A page under a section, reached from a row of the section's page, with a link back to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Under {
    /// The templates of every account, under Sending.
    Templates,
    /// Everything that waits for its time, under Sending.
    Scheduled,
}

impl Under {
    /// The section it is under.
    #[must_use]
    pub fn section(self) -> Section {
        match self {
            Self::Templates | Self::Scheduled => Section::Sending,
        }
    }

    /// Its id, as a page of the settings is named from outside.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Templates => "templates",
            Self::Scheduled => "scheduled",
        }
    }

    /// The page whose [`Self::id`] is `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [Self::Templates, Self::Scheduled].into_iter().find(|under| under.id() == name)
    }
}

impl Settings {
    /// Whether the settings are the place shown.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The page shown.
    #[must_use]
    pub fn section(&self) -> Section {
        self.section
    }

    /// Shows the settings on the page last shown (Appearance the first time).
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Shows the settings on `section`; an action waiting for a key stops waiting.
    pub fn open_at(&mut self, section: Section) {
        self.open = true;
        self.show(section);
    }

    /// Shows the settings on the section whose id ([`Section::id`]) is `page`; a page that is no section's id (outside
    /// data: the control channel's, a script's) shows the page last shown.
    pub fn open_page(&mut self, page: Option<&str>) {
        if let Some(under) = page.and_then(Under::named) {
            self.open = true;
            self.show_under(under);
            return;
        }
        match page.and_then(Section::named) {
            Some(section) => self.open_at(section),
            None => self.open(),
        }
    }

    /// Why the last change of starting with the computer was not done, while the settings show.
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

    /// Shows `section`; an action waiting for a key stops waiting and a page under a section closes.
    pub fn show(&mut self, section: Section) {
        self.section = section;
        self.capturing = None;
        self.under = None;
        self.templates.leave();
        self.scheduled.leave();
        self.privacy.leave();
    }

    /// Shows the page `under` its section.
    pub fn show_under(&mut self, under: Under) {
        self.show(under.section());
        self.under = Some(under);
    }

    /// The page under the section shown, if one is.
    #[must_use]
    pub fn under(&self) -> Option<Under> {
        self.under
    }

    /// Whether the page shown asks a confirmation in its dialog over the dimmed window: a template's delete, an entry's
    /// cancel, or, on Accounts, the account's removal (`removing`: the account's form asks it).
    #[must_use]
    pub fn confirms(&self, removing: bool) -> bool {
        self.open
            && match self.under {
                Some(Under::Templates) => self.templates.form().is_none() && self.templates.confirming().is_some(),
                Some(Under::Scheduled) => self.scheduled.confirming().is_some(),
                None => self.section == Section::Accounts && removing,
            }
    }

    /// The Templates page's state.
    #[must_use]
    pub fn templates(&self) -> &crate::templates::Templates {
        &self.templates
    }

    /// The Templates page's state, to change.
    pub fn templates_mut(&mut self) -> &mut crate::templates::Templates {
        &mut self.templates
    }

    /// The Scheduled page's state.
    #[must_use]
    pub fn scheduled(&self) -> &crate::scheduled::Scheduled {
        &self.scheduled
    }

    /// The Scheduled page's state, to change.
    pub fn scheduled_mut(&mut self) -> &mut crate::scheduled::Scheduled {
        &mut self.scheduled
    }

    /// The Privacy page's state.
    #[must_use]
    pub fn privacy(&self) -> &crate::privacy::Privacy {
        &self.privacy
    }

    /// The Privacy page's state, to change.
    pub fn privacy_mut(&mut self) -> &mut crate::privacy::Privacy {
        &mut self.privacy
    }

    /// Asks that the keyboard go to the shown section's row of the settings' own column when it is drawn next: where
    /// a page's last control went.
    pub fn hand_to_nav(&mut self) {
        self.nav_hand = true;
    }

    /// Whether the keyboard was asked to go to the shown section's row, taken.
    pub fn take_nav_hand(&mut self) -> bool {
        std::mem::take(&mut self.nav_hand)
    }

    /// Another place is shown: an action waiting for a key stops waiting and the notes on what the last change did go;
    /// the page, the text that narrows the Shortcuts page and a signature typed stay.
    pub fn leave(&mut self) {
        self.open = false;
        self.capturing = None;
        self.keys_note = None;
        self.login_note = None;
        self.templates.leave();
        self.scheduled.leave();
        self.privacy.leave();
        self.nav_hand = false;
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
