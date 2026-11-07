// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composer's state over one of the engine's drafts: who it is from, its recipients as chips, its subject and its
//! text. Everything goes through [`Mailbox::run`] by command id (`compose.new`, `compose.update`, `compose.discard`,
//! `mail.send`), as the CLI and MCP do.
//!
//! - The draft is in the store from the moment the composer opens, and every change is kept at once
//!   ([`Composer::save`]): nothing asks, nothing shows, and a crash or a close loses nothing typed.
//! - What is typed into a recipient line becomes a chip on Enter, on a comma or a semicolon, and when the line is
//!   left; a highlighted suggestion is taken instead of the typed text. Each chip is checked by the engine as it is
//!   kept: one that cannot be an address is marked at once and blocks Send, as a draft without a recipient does.
//! - Suggestions are the people in the mail kept here (`mail.search` on senders and recipients) whose name or address
//!   holds what is typed, the owner's own addresses and the recipients already there left out. Their names are the
//!   senders' text: data, shown and never run.
//! - Closing takes what is typed in a recipient line as a chip and keeps the draft; Discard removes it.
//! - Reply, reply all and forward start from a message (`compose.reply`, `compose.reply_all`, `compose.forward`): the
//!   engine fills in the recipients, the subject and the quote.
//! - The templates of the account the draft is from are offered (`template.list`); one taken goes in front of the text
//!   (`compose.template`). Each blank it leaves is a `[[prompt]]` mark, asked for one at a time: what is typed for it
//!   takes the mark's place in the subject and the text. While a mark is left, Send is blocked and says which.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::mailbox::{Mailbox, shown};
use crate::{UiError, tr};

/// The most suggestions shown under a recipient line.
pub const MAX_SUGGESTIONS: usize = 6;

/// The most recipients in one line, as the engine takes them.
pub const MAX_RECIPIENTS: usize = 500;

/// The longest recipient, in bytes, as the engine takes it.
pub const MAX_RECIPIENT_BYTES: usize = 998;

/// The longest subject, in bytes, as the engine takes it.
pub const MAX_SUBJECT_BYTES: usize = 998;

/// The longest text, in bytes, as the engine takes it.
pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// How many messages the search for suggestions reads, per field.
const SEARCHED: u64 = 200;

/// The characters that end a recipient as it is typed.
const SEPARATORS: [char; 4] = [',', ';', '\n', '\r'];

/// How a draft is made from a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// To its sender.
    Reply,
    /// To its sender and its other recipients.
    ReplyAll,
    /// Inline, to whom the owner chooses.
    Forward,
}

impl Answer {
    /// The engine's command that makes the draft.
    #[must_use]
    pub fn command(self) -> &'static str {
        match self {
            Self::Reply => "compose.reply",
            Self::ReplyAll => "compose.reply_all",
            Self::Forward => "compose.forward",
        }
    }
}

/// A recipient line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// To.
    To,
    /// Cc.
    Cc,
    /// Bcc.
    Bcc,
}

impl Line {
    /// The lines, in the order the composer shows them.
    pub const ALL: [Self; 3] = [Self::To, Self::Cc, Self::Bcc];

    fn index(self) -> usize {
        match self {
            Self::To => 0,
            Self::Cc => 1,
            Self::Bcc => 2,
        }
    }

    /// The line's field in the engine's draft.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::To => "to",
            Self::Cc => "cc",
            Self::Bcc => "bcc",
        }
    }

    /// The line's name as the composer shows it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::To => tr!("To"),
            Self::Cc => tr!("Cc"),
            Self::Bcc => tr!("Bcc"),
        }
    }
}

/// A recipient as typed or taken, and whether the engine found an address in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    /// The recipient as kept: `Name <address>` or a bare address.
    pub text: String,
    /// Whether it is an address.
    pub ok: bool,
}

/// `text` split as `Name <address>`: the name, if any, and the address part.
fn split(text: &str) -> (Option<&str>, &str) {
    let trimmed = text.trim();
    let named = trimmed
        .strip_suffix('>')
        .and_then(|rest| rest.rfind('<').map(|at| (rest.get(..at).unwrap_or_default(), rest.get(at.saturating_add(1)..).unwrap_or_default())));
    match named {
        Some((name, address)) => (Some(name.trim().trim_matches('"').trim()).filter(|name| !name.is_empty()), address.trim()),
        None => (None, trimmed),
    }
}

impl Chip {
    /// What the chip shows: the name, or the address when there is none, fit for one line.
    #[must_use]
    pub fn shown(&self) -> String {
        let (name, address) = split(&self.text);
        shown(name.unwrap_or(address))
    }

    /// The address part, fit for one line.
    #[must_use]
    pub fn address(&self) -> String {
        shown(split(&self.text).1)
    }

    /// The chip as a screen reader says it.
    #[must_use]
    pub fn spoken(&self) -> String {
        let (name, address) = split(&self.text);
        let named = match name {
            Some(name) => tr!("{name}, {address}", name = shown(name), address = shown(address)),
            None => shown(address),
        };
        if self.ok { named } else { tr!("{recipient}, not an address", recipient = named) }
    }
}

/// A person the mail knows, offered under a recipient line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Their name, empty when the mail gives none.
    pub name: String,
    /// Their address.
    pub address: String,
}

impl Suggestion {
    /// The recipient it becomes: `Name <address>`, or the address alone when the name has nothing to show or holds a
    /// character that would change where the address starts.
    #[must_use]
    pub fn recipient(&self) -> String {
        let plain = !self.name.trim().is_empty() && !self.name.contains(['<', '>', '"', ',', ';', '@']);
        if plain { format!("{} <{}>", self.name.trim(), self.address) } else { self.address.clone() }
    }

    /// The suggestion as the list shows it.
    #[must_use]
    pub fn shown(&self) -> String {
        if self.name.trim().is_empty() { self.address.clone() } else { tr!("{name} <{address}>", name = self.name.trim(), address = self.address) }
    }
}

/// `text` cut to at most `bytes` bytes, at a character's start.
pub(crate) fn cut(text: &mut String, bytes: usize) {
    if text.len() > bytes {
        let mut end = bytes;
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        text.truncate(end);
    }
}

fn shape(command: &'static str, detail: &str) -> UiError {
    UiError::Answer { command, detail: detail.to_owned() }
}

/// The composer over one draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composer {
    draft: u64,
    account: u64,
    lines: [Vec<Chip>; 3],
    typing: [String; 3],
    subject: String,
    text: String,
    /// Whether the Cc and Bcc lines show.
    copies: bool,
    /// Whether it is the floating panel, the list and the reader having the window.
    floating: bool,
    blocks: Vec<String>,
    suggestions: Vec<Suggestion>,
    /// The line the suggestions are for.
    suggesting: Option<Line>,
    highlighted: Option<usize>,
    /// Whether something changed since the draft was last kept.
    changed: bool,
    /// The marks of the blanks still in the subject or the text, as the engine kept them.
    blanks: Vec<String>,
    /// What is typed for the first blank.
    filling: String,
    /// The names of the templates of the draft's account.
    templates: Vec<String>,
}

impl Composer {
    /// A composer over a new draft (`compose.new`), from the account that last sent, with its signature.
    ///
    /// # Errors
    ///
    /// The command's refusal (no account to write from), or an answer the composer does not read.
    pub fn start(mailbox: &mut Mailbox) -> Result<Self, UiError> {
        let answer = mailbox.run("compose.new", &json!({}))?;
        Self::over("compose.new", &answer)?.with_templates(mailbox)
    }

    /// A composer over a new draft made from the message `id` as `answer` asks, filled in by the engine.
    ///
    /// # Errors
    ///
    /// The command's refusal (no such message), or an answer the composer does not read.
    pub fn answer(mailbox: &mut Mailbox, id: u64, answer: Answer) -> Result<Self, UiError> {
        let command = answer.command();
        let answer = mailbox.run(command, &json!({ "id": id }))?;
        Self::over(command, &answer)?.with_templates(mailbox)
    }

    /// A composer over the kept draft `id` (`compose.show`).
    ///
    /// # Errors
    ///
    /// The command's refusal (no such draft), or an answer the composer does not read.
    pub fn open(mailbox: &mut Mailbox, id: u64) -> Result<Self, UiError> {
        let answer = mailbox.run("compose.show", &json!({ "id": id }))?;
        Self::over("compose.show", &answer)?.with_templates(mailbox)
    }

    fn with_templates(mut self, mailbox: &mut Mailbox) -> Result<Self, UiError> {
        self.load_templates(mailbox)?;
        Ok(self)
    }

    /// Reads the names of the templates of the draft's account.
    fn load_templates(&mut self, mailbox: &mut Mailbox) -> Result<(), UiError> {
        let answer = mailbox.run("template.list", &json!({ "account": self.account }))?;
        self.templates = answer
            .get("templates")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|template| template.get("name")?.as_str()).map(str::to_owned).collect())
            .unwrap_or_default();
        Ok(())
    }

    fn over(command: &'static str, answer: &Value) -> Result<Self, UiError> {
        let draft = answer.get("id").and_then(Value::as_u64).ok_or_else(|| shape(command, "no draft id"))?;
        let mut composer = Self {
            draft,
            account: 0,
            lines: [Vec::new(), Vec::new(), Vec::new()],
            typing: [String::new(), String::new(), String::new()],
            subject: String::new(),
            text: String::new(),
            copies: false,
            floating: false,
            blocks: Vec::new(),
            suggestions: Vec::new(),
            suggesting: None,
            highlighted: None,
            changed: false,
            blanks: Vec::new(),
            filling: String::new(),
            templates: Vec::new(),
        };
        composer.take_answer(answer);
        composer.copies = !(composer.chips(Line::Cc).is_empty() && composer.chips(Line::Bcc).is_empty());
        Ok(composer)
    }

    /// Reads the draft as the engine kept it: the account, the chips checked, the subject, the text and what blocks
    /// Send. What is being typed in a recipient line stays.
    fn take_answer(&mut self, answer: &Value) {
        self.account = answer.get("account").and_then(Value::as_u64).unwrap_or(self.account);
        for line in Line::ALL {
            if let Some(list) = answer.get(line.key()).and_then(Value::as_array)
                && let Some(chips) = self.lines.get_mut(line.index())
            {
                *chips = list
                    .iter()
                    .filter_map(|chip| {
                        let text = chip.get("text").and_then(Value::as_str)?;
                        Some(Chip { text: text.to_owned(), ok: chip.get("ok").and_then(Value::as_bool) == Some(true) })
                    })
                    .collect();
            }
        }
        if let Some(subject) = answer.get("subject").and_then(Value::as_str) {
            subject.clone_into(&mut self.subject);
        }
        if let Some(text) = answer.get("text").and_then(Value::as_str) {
            text.clone_into(&mut self.text);
        }
        self.blocks = answer
            .get("blocks")
            .and_then(Value::as_array)
            .map(|blocks| blocks.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        self.blanks = answer
            .get("blanks")
            .and_then(Value::as_array)
            .map(|blanks| blanks.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
    }

    /// The draft's id.
    #[must_use]
    pub fn draft(&self) -> u64 {
        self.draft
    }

    /// The account it is from.
    #[must_use]
    pub fn account(&self) -> u64 {
        self.account
    }

    /// The chips of `line`.
    #[must_use]
    pub fn chips(&self, line: Line) -> &[Chip] {
        self.lines.get(line.index()).map_or(&[], Vec::as_slice)
    }

    /// What is being typed in `line`.
    #[must_use]
    pub fn typing(&self, line: Line) -> &str {
        self.typing.get(line.index()).map_or("", String::as_str)
    }

    /// What is being typed in `line`, for its field.
    pub fn typing_mut(&mut self, line: Line) -> Option<&mut String> {
        self.typing.get_mut(line.index())
    }

    /// The subject.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The subject, for its field; [`Composer::edited`] after a change.
    pub fn subject_mut(&mut self) -> &mut String {
        &mut self.subject
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text, for its field; [`Composer::edited`] after a change.
    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    /// The subject or the text changed in their field: they are cut to what the engine keeps, and kept at the next
    /// [`Composer::save`].
    pub fn edited(&mut self) {
        cut(&mut self.subject, MAX_SUBJECT_BYTES);
        cut(&mut self.text, MAX_TEXT_BYTES);
        self.changed = true;
    }

    /// Whether something changed since the draft was last kept.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Whether the Cc and Bcc lines show.
    #[must_use]
    pub fn copies(&self) -> bool {
        self.copies
    }

    /// Shows the Cc and Bcc lines.
    pub fn show_copies(&mut self) {
        self.copies = true;
    }

    /// Whether it is the floating panel.
    #[must_use]
    pub fn floating(&self) -> bool {
        self.floating
    }

    /// Makes it the floating panel, or puts it back in the reader's place.
    pub fn set_floating(&mut self, floating: bool) {
        self.floating = floating;
    }

    /// What the composer's title says: the subject, or "New message".
    #[must_use]
    pub fn title(&self) -> String {
        let subject = shown(self.subject.trim());
        if subject.is_empty() { tr!("New message").to_owned() } else { subject }
    }

    /// Whether the draft can be sent as it is kept: it has a recipient and every one is an address.
    #[must_use]
    pub fn sendable(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Why Send is blocked, as the composer says it; none when it is not.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        if let Some(bad) = Line::ALL.iter().flat_map(|line| self.chips(*line)).find(|chip| !chip.ok) {
            return Some(tr!("{address} is not an address", address = bad.address()));
        }
        let blocked = |block: &str| self.blocks.iter().any(|held| held == block);
        if blocked("no_recipient") {
            return Some(tr!("Add a recipient to send").to_owned());
        }
        if let Some(blank) = self.blank().filter(|_| blocked("blank")) {
            return Some(tr!("Fill in \"{blank}\" to send", blank = blank));
        }
        if blocked("forward_body") {
            return Some(tr!("The forwarded message is no longer here, so this cannot be sent").to_owned());
        }
        (!self.blocks.is_empty()).then(|| tr!("This draft cannot be sent yet").to_owned())
    }

    /// The prompt of the first blank still to fill in, fit for one line.
    #[must_use]
    pub fn blank(&self) -> Option<String> {
        self.blanks.first().map(|mark| shown(mark.strip_prefix("[[").and_then(|mark| mark.strip_suffix("]]")).unwrap_or(mark)))
    }

    /// The marks of the blanks still to fill in.
    #[must_use]
    pub fn blanks(&self) -> &[String] {
        &self.blanks
    }

    /// What is typed for the first blank.
    #[must_use]
    pub fn filling(&self) -> &str {
        &self.filling
    }

    /// What is typed for the first blank, for its field.
    pub fn filling_mut(&mut self) -> &mut String {
        &mut self.filling
    }

    /// Puts what is typed for the first blank in its mark's place, in the subject and the text, and keeps the draft;
    /// nothing typed fills nothing.
    ///
    /// # Errors
    ///
    /// The command's refusal.
    pub fn fill(&mut self, mailbox: &mut Mailbox) -> Result<(), UiError> {
        let typed: String = self.filling.chars().filter(|c| !c.is_control()).collect();
        let typed = typed.trim();
        let Some(mark) = self.blanks.first().cloned() else { return Ok(()) };
        if typed.is_empty() {
            return Ok(());
        }
        self.subject = self.subject.replace(&mark, typed);
        self.text = self.text.replace(&mark, typed);
        self.filling.clear();
        self.edited();
        self.save(mailbox)
    }

    /// The names of the templates of the account the draft is from.
    #[must_use]
    pub fn templates(&self) -> &[String] {
        &self.templates
    }

    /// Puts the template `name` into the draft (`compose.template`), after keeping what is typed: its text in front,
    /// its subject where there is none, its blanks marked.
    ///
    /// # Errors
    ///
    /// The command's refusal (no such template, or a text that would be too long).
    pub fn use_template(&mut self, mailbox: &mut Mailbox, name: &str) -> Result<(), UiError> {
        self.save(mailbox)?;
        let answer = mailbox.run("compose.template", &json!({ "id": self.draft, "name": name }))?;
        self.take_answer(&answer);
        Ok(())
    }

    /// The suggestions for what is typed, and the line they are for.
    #[must_use]
    pub fn suggestions(&self) -> (Option<Line>, &[Suggestion]) {
        (self.suggesting, &self.suggestions)
    }

    /// The highlighted suggestion's place, if one is.
    #[must_use]
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// Moves the highlight one suggestion down (`down`) or up; from none, down reaches the first. Stays at either
    /// end; up from the first takes the highlight off.
    pub fn step(&mut self, down: bool) {
        let last = self.suggestions.len().checked_sub(1);
        self.highlighted = match (self.highlighted, last) {
            (_, None) => None,
            (None, Some(_)) if down => Some(0),
            (None, Some(_)) => None,
            (Some(at), Some(last)) if down => Some(at.saturating_add(1).min(last)),
            (Some(0), Some(_)) => None,
            (Some(at), Some(_)) => Some(at.saturating_sub(1)),
        };
    }

    /// Forgets the suggestions.
    pub fn dismiss_suggestions(&mut self) {
        self.suggestions.clear();
        self.suggesting = None;
        self.highlighted = None;
    }

    /// What is typed in `line` changed: what stands before a comma, a semicolon or a line break becomes chips, and the
    /// rest is looked up among the people the mail knows.
    ///
    /// # Errors
    ///
    /// A command's refusal; the typed text stays.
    pub fn typed(&mut self, mailbox: &mut Mailbox, line: Line) -> Result<(), UiError> {
        let typed = self.typing(line).to_owned();
        if let Some(at) = typed.rfind(SEPARATORS) {
            let (done, rest) = typed.split_at(at);
            let rest = rest.get(1..).unwrap_or_default().trim_start().to_owned();
            self.add(line, done);
            if let Some(field) = self.typing_mut(line) {
                *field = rest;
            }
            self.save(mailbox)?;
        }
        self.suggest(mailbox, line)
    }

    /// Takes what is typed in `line` as chips, or the highlighted suggestion in its place (Enter, or the line left).
    ///
    /// # Errors
    ///
    /// A command's refusal.
    pub fn take(&mut self, mailbox: &mut Mailbox, line: Line) -> Result<(), UiError> {
        match self.highlighted.filter(|_| self.suggesting == Some(line)) {
            Some(at) => self.pick(mailbox, at),
            None => {
                let typed = self.typing.get_mut(line.index()).map(std::mem::take).unwrap_or_default();
                self.add(line, &typed);
                self.dismiss_suggestions();
                self.save(mailbox)
            }
        }
    }

    /// Takes the suggestion at `at` as a chip of its line; the typed text it stood for goes.
    ///
    /// # Errors
    ///
    /// A command's refusal.
    pub fn pick(&mut self, mailbox: &mut Mailbox, at: usize) -> Result<(), UiError> {
        let (Some(line), Some(suggestion)) = (self.suggesting, self.suggestions.get(at).cloned()) else { return Ok(()) };
        if let Some(field) = self.typing_mut(line) {
            field.clear();
        }
        self.add(line, &suggestion.recipient());
        self.dismiss_suggestions();
        self.save(mailbox)
    }

    /// Removes the chip at `at` of `line`.
    ///
    /// # Errors
    ///
    /// A command's refusal.
    pub fn remove(&mut self, mailbox: &mut Mailbox, line: Line, at: usize) -> Result<(), UiError> {
        if let Some(chips) = self.lines.get_mut(line.index())
            && at < chips.len()
        {
            chips.remove(at);
            self.changed = true;
        }
        self.save(mailbox)
    }

    /// Removes the last chip of `line` (Backspace in its empty field).
    ///
    /// # Errors
    ///
    /// A command's refusal.
    pub fn remove_last(&mut self, mailbox: &mut Mailbox, line: Line) -> Result<(), UiError> {
        let last = self.chips(line).len().checked_sub(1);
        match last {
            Some(at) => self.remove(mailbox, line, at),
            None => Ok(()),
        }
    }

    /// Adds the recipients in `typed` to `line`, split at the separators: each once across the lines, cut to what the
    /// engine keeps.
    fn add(&mut self, line: Line, typed: &str) {
        let mut held: BTreeSet<String> = Line::ALL.iter().flat_map(|line| self.chips(*line)).map(|chip| split(&chip.text).1.to_lowercase()).collect();
        let Some(chips) = self.lines.get_mut(line.index()) else { return };
        for part in typed.split(SEPARATORS).map(str::trim).filter(|part| !part.is_empty()) {
            if chips.len() >= MAX_RECIPIENTS {
                break;
            }
            let mut text = part.chars().filter(|c| !c.is_control()).collect::<String>();
            cut(&mut text, MAX_RECIPIENT_BYTES);
            if held.insert(split(&text).1.to_lowercase()) {
                // Checked by the engine once kept; until then it counts as not an address.
                chips.push(Chip { text, ok: false });
                self.changed = true;
            }
        }
    }

    /// Looks up the people whose name or address holds what is typed in `line`.
    fn suggest(&mut self, mailbox: &mut Mailbox, line: Line) -> Result<(), UiError> {
        let query = self.typing(line).trim().to_owned();
        self.dismiss_suggestions();
        if query.is_empty() {
            return Ok(());
        }
        let mut skip: BTreeSet<String> = Line::ALL.iter().flat_map(|line| self.chips(*line)).map(|chip| split(&chip.text).1.to_lowercase()).collect();
        skip.extend(mailbox.accounts().iter().map(|account| account.address.to_lowercase()));
        let wanted = query.to_lowercase();
        let mut found = Vec::new();
        for field in ["from", "to"] {
            let mut params = json!({ "limit": SEARCHED });
            if let Value::Object(fields) = &mut params {
                fields.insert(field.to_owned(), Value::String(query.clone()));
            }
            let answer = mailbox.run("mail.search", &params)?;
            let messages = answer.get("messages").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
            for person in messages.iter().flat_map(|message| ["from", "to"].into_iter().filter_map(|key| message.get(key)?.as_array()).flatten()) {
                let name = shown(person.get("name").and_then(Value::as_str).unwrap_or_default());
                let address = shown(person.get("address").and_then(Value::as_str).unwrap_or_default());
                let matches = name.to_lowercase().contains(&wanted) || address.to_lowercase().contains(&wanted);
                if found.len() < MAX_SUGGESTIONS && matches && !address.is_empty() && skip.insert(address.to_lowercase()) {
                    found.push(Suggestion { name, address });
                }
            }
        }
        if !found.is_empty() {
            self.suggestions = found;
            self.suggesting = Some(line);
        }
        Ok(())
    }

    /// Keeps the draft as it is (`compose.update`) and reads back the chips as the engine checked them.
    ///
    /// # Errors
    ///
    /// The command's refusal (a draft on its way is not changed); the composer keeps what was typed.
    pub fn save(&mut self, mailbox: &mut Mailbox) -> Result<(), UiError> {
        let list = |line: Line| self.chips(line).iter().map(|chip| chip.text.clone()).collect::<Vec<_>>();
        let params = json!({
            "id": self.draft,
            "to": list(Line::To),
            "cc": list(Line::Cc),
            "bcc": list(Line::Bcc),
            "subject": self.subject,
            "text": self.text,
        });
        let answer = mailbox.run("compose.update", &params)?;
        self.take_answer(&answer);
        self.changed = false;
        Ok(())
    }

    /// Writes the draft from the account `id` instead: its signature takes the place of the old one where that is
    /// still at the end as it was.
    ///
    /// # Errors
    ///
    /// The command's refusal.
    pub fn set_account(&mut self, mailbox: &mut Mailbox, id: u64) -> Result<(), UiError> {
        self.save(mailbox)?;
        let answer = mailbox.run("compose.update", &json!({ "id": self.draft, "account": id }))?;
        self.take_answer(&answer);
        self.load_templates(mailbox)
    }

    /// Before the composer closes: what is typed in a recipient line becomes a chip and the draft is kept.
    ///
    /// # Errors
    ///
    /// The command's refusal; then the composer stays open with everything in it.
    pub fn close(&mut self, mailbox: &mut Mailbox) -> Result<(), UiError> {
        for line in Line::ALL {
            let typed = self.typing.get_mut(line.index()).map(std::mem::take).unwrap_or_default();
            self.add(line, &typed);
        }
        self.dismiss_suggestions();
        self.save(mailbox)
    }

    /// Sends the draft (`mail.send`) after keeping it as [`Composer::close`] does. Whether it went: a draft that is
    /// blocked is kept and not sent.
    ///
    /// # Errors
    ///
    /// A command's refusal (an account without a submission server).
    pub fn send(&mut self, mailbox: &mut Mailbox) -> Result<bool, UiError> {
        self.close(mailbox)?;
        if !self.sendable() {
            return Ok(false);
        }
        mailbox.run("mail.send", &json!({ "id": self.draft }))?;
        Ok(true)
    }

    /// Removes the draft here and its copy on the server (`compose.discard`).
    ///
    /// # Errors
    ///
    /// The command's refusal.
    pub fn discard(&self, mailbox: &mut Mailbox) -> Result<(), UiError> {
        mailbox.run("compose.discard", &json!({ "id": self.draft }))?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
