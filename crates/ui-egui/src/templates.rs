// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the Templates page of the settings shows and holds: the templates of every account as `template.list` answered
//! them, the one being written or edited, and the one whose delete waits for the owner's confirmation.
//!
//! A template comes from the store, which is outside data: its name and its line (subject and text) are made plain
//! (control characters as spaces, bidi overrides left out) and capped where a row is built, so the page lays out at
//! most a line's worth of each.

use serde_json::{Value, json};

use crate::component::{capped, plain};

/// The most characters of a template's name a row keeps: the model's own cap on a name.
const NAME: usize = inboxcraft_model::Template::MAX_NAME_CHARS;

/// The most characters of a row's line (the subject, a dash and the text) a row keeps: more than a line shows.
const LINE: usize = 240;

/// The most templates the page keeps: the model's cap on one account's, times a generous number of accounts.
pub const MOST: usize = inboxcraft_model::Account::MAX_TEMPLATES * 20;

/// A template as a row of the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRow {
    /// The account it is kept on.
    pub account: u64,
    /// Its name as kept, to edit and delete it by.
    pub name: String,
    /// Its name as the row shows it: plain and capped.
    pub shown: String,
    /// Its subject, a dash and its text as the row shows them: plain and capped.
    pub line: String,
    /// Its subject as kept, for the form.
    pub subject: String,
    /// Its text as kept, for the form.
    pub text: String,
}

impl TemplateRow {
    /// The row of one template of `template.list`'s answer; `None` for one without an account or a name.
    #[must_use]
    pub fn read(value: &Value) -> Option<Self> {
        let account = value.get("account").and_then(Value::as_u64)?;
        let name = value.get("name").and_then(Value::as_str).filter(|name| !name.trim().is_empty())?;
        let subject = value.get("subject").and_then(Value::as_str).unwrap_or_default();
        let text = value.get("text").and_then(Value::as_str).unwrap_or_default();
        let (name_kept, _) = capped(name, NAME);
        let (subject_kept, _) = capped(subject, LINE);
        let (text_kept, _) = capped(text, LINE);
        let joined = if subject_kept.trim().is_empty() { text_kept.to_owned() } else { format!("{subject_kept} — {text_kept}") };
        let (line, _) = capped(&joined, LINE);
        Some(Self {
            account,
            name: name.to_owned(),
            shown: plain(name_kept).trim().to_owned(),
            line: plain(line).split_whitespace().collect::<Vec<_>>().join(" "),
            subject: subject.to_owned(),
            text: text.to_owned(),
        })
    }
}

/// A template as the form holds it while it is written.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateForm {
    /// The account it is kept on.
    pub account: u64,
    /// The account and name it had when the form opened on it; `None` for a new one.
    pub was: Option<(u64, String)>,
    /// Its name as typed.
    pub name: String,
    /// Its subject as typed.
    pub subject: String,
    /// Its text as typed.
    pub text: String,
}

impl TemplateForm {
    /// What `template.save` is asked for.
    #[must_use]
    pub fn params(&self) -> Value {
        json!({ "account": self.account, "name": self.name.trim(), "subject": self.subject, "text": self.text })
    }

    /// Whether saving it leaves its old self behind: it was renamed or moved to another account.
    #[must_use]
    pub fn leaves(&self) -> Option<(u64, &str)> {
        self.was
            .as_ref()
            .filter(|(account, name)| *account != self.account || name != self.name.trim())
            .map(|(account, name)| (*account, name.as_str()))
    }
}

/// Where the keyboard goes when the page is drawn next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hand {
    /// The confirmation's Cancel.
    Cancel,
    /// The `delete` of a row: its account and name.
    Delete(u64, String),
    /// "New template".
    New,
    /// The form's name.
    Name,
}

/// The Templates page's state.
#[derive(Debug, Default)]
pub struct Templates {
    /// Where the keyboard goes when the page is drawn next.
    hand: Option<Hand>,
    rows: Vec<TemplateRow>,
    /// The template being written, while the form shows.
    form: Option<TemplateForm>,
    /// The template whose delete waits for the owner's confirmation.
    confirm: Option<(u64, String)>,
    /// Whether the account choice of the form is open.
    choosing: bool,
    /// Why the last save or delete was not done.
    note: Option<String>,
}

impl Templates {
    /// The templates, in the order `template.list` gave them.
    #[must_use]
    pub fn rows(&self) -> &[TemplateRow] {
        &self.rows
    }

    /// Takes `template.list`'s answer: at most [`MOST`] rows, the confirmation dropped when its template went.
    pub fn read(&mut self, answer: &Value) {
        let listed = answer.get("templates").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        self.rows = listed.iter().filter_map(TemplateRow::read).take(MOST).collect();
        if let Some((account, name)) = &self.confirm
            && !self.rows.iter().any(|row| row.account == *account && row.name == *name)
        {
            self.confirm = None;
        }
    }

    /// The form, while it shows.
    #[must_use]
    pub fn form(&self) -> Option<&TemplateForm> {
        self.form.as_ref()
    }

    /// The form to type in, while it shows.
    pub fn form_mut(&mut self) -> Option<&mut TemplateForm> {
        self.form.as_mut()
    }

    /// Opens the form on a new template of `account`.
    pub fn write_new(&mut self, account: u64) {
        self.form = Some(TemplateForm { account, ..TemplateForm::default() });
        self.confirm = None;
        self.choosing = false;
        self.note = None;
    }

    /// Opens the form on the template `name` of `account`; nothing when there is none.
    pub fn edit(&mut self, account: u64, name: &str) {
        if let Some(row) = self.rows.iter().find(|row| row.account == account && row.name == name) {
            self.form = Some(TemplateForm {
                account,
                was: Some((account, row.name.clone())),
                name: row.name.clone(),
                subject: row.subject.clone(),
                text: row.text.clone(),
            });
            self.confirm = None;
            self.choosing = false;
            self.note = None;
        }
    }

    /// Closes the form, keeping nothing typed.
    pub fn close_form(&mut self) {
        self.form = None;
        self.choosing = false;
        self.note = None;
    }

    /// Whether the form's account choice is open.
    pub fn choosing_mut(&mut self) -> &mut bool {
        &mut self.choosing
    }

    /// The template whose delete waits for the owner's confirmation.
    #[must_use]
    pub fn confirming(&self) -> Option<(u64, &str)> {
        self.confirm.as_ref().map(|(account, name)| (*account, name.as_str()))
    }

    /// Asks the owner to confirm the delete of `name` of `account`.
    pub fn ask_delete(&mut self, account: u64, name: &str) {
        self.confirm = Some((account, name.to_owned()));
        self.note = None;
    }

    /// The delete was not confirmed.
    pub fn keep(&mut self) {
        self.confirm = None;
    }

    /// Why the last save or delete was not done.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Says why the last save or delete was not done; `None` when it was.
    pub fn set_note(&mut self, note: Option<String>) {
        self.note = note;
    }

    /// Whether the account `account` already has another template named as the form's: Save replaces it.
    #[must_use]
    pub fn replaces(&self) -> bool {
        let Some(form) = &self.form else { return false };
        let name = form.name.trim();
        let same = form.was.as_ref().is_some_and(|(account, was)| *account == form.account && was == name);
        !same && !name.is_empty() && self.rows.iter().any(|row| row.account == form.account && row.name == name)
    }

    /// Asks that the keyboard go to `hand` when the page is drawn next.
    pub fn hand_to(&mut self, hand: Hand) {
        self.hand = Some(hand);
    }

    /// Where the keyboard was asked to go, taken.
    pub fn take_hand(&mut self) -> Option<Hand> {
        self.hand.take()
    }

    /// Another place or page is shown: the confirmation and the account choice close; the form stays as typed.
    pub fn leave(&mut self) {
        self.confirm = None;
        self.choosing = false;
        self.hand = None;
    }
}
