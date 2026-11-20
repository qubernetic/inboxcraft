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
//! - The templates of the account the draft is from are offered (`template.list`, none when it is refused); one taken
//!   goes in front of the text (`compose.template`). Each blank it leaves is a `[[prompt]]` mark, asked for one at a
//!   time: what is typed for it takes the mark's place in the subject and the text. While a mark is left, Send is
//!   blocked and says which.
//! - Send goes after the hold, in which it can be taken back (`mail.send`); with a time picked for it, Send becomes
//!   Schedule and the draft goes then (`mail.send_later`).

use std::collections::BTreeSet;

use inboxcraft_time::Timestamp;
use serde_json::{Value, json};

use crate::mailbox::{Mailbox, engine_date, parse_date, plainly_named, shown};
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
    /// The name it may show, fit for one line: none where there is none, or where it could be taken for another
    /// address or shows nothing ([`plainly_named`]); the chip then shows the address, so nothing goes to an address
    /// the owner was not shown.
    fn name(&self) -> Option<String> {
        split(&self.text).0.map(shown).filter(|name| plainly_named(name))
    }

    /// What the chip shows: the name, or the address where it may show none, fit for one line.
    #[must_use]
    pub fn shown(&self) -> String {
        self.name().unwrap_or_else(|| self.address())
    }

    /// The chip's tooltip: its address, where the chip shows a name.
    #[must_use]
    pub fn tip(&self) -> Option<String> {
        self.name().map(|_| self.address())
    }

    /// The address part, fit for one line.
    #[must_use]
    pub fn address(&self) -> String {
        shown(split(&self.text).1)
    }

    /// The chip as a screen reader says it.
    #[must_use]
    pub fn spoken(&self) -> String {
        let named = match self.name() {
            Some(name) => tr!("{name}, {address}", name = name, address = self.address()),
            None => self.address(),
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
    /// The time picked to send it at; `None` sends it now, after the hold.
    later: Option<Timestamp>,
    /// The first frame that painted the whole composer, its lines in sight, since it was opened, moved to another
    /// place or changed from outside its fields: Send waits for a frame after it.
    painted_in: Option<u64>,
    /// A script set the scene without a frame and counts it as seen.
    scripted: bool,
    /// The place it was last drawn in: in the reader's place (`true`) or floating.
    drawn_docked: Option<bool>,
    /// Whether the floating panel is folded to its head.
    folded: bool,
    /// When the engine last kept the draft.
    saved: Option<Timestamp>,
    /// Why the last fill of a blank was refused (it would make the subject or the text longer than the engine keeps),
    /// and the text it refused: said while that text stands.
    fill_refused: Option<(String, String)>,
    /// Every chip shows its address, not the name: the draft was opened from a send refused and changed since (or one
    /// that keeps no record of how it went), so what goes is shown as where it goes. Until the composer closes.
    addresses: bool,
}

/// What a line refused of what was typed in it, which stays in its typed text as it was written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Refused {
    /// Addresses not added because the line held [`MAX_RECIPIENTS`] already.
    over: usize,
    /// An address longer than [`MAX_RECIPIENT_BYTES`] was not added.
    too_long: bool,
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
        self.load_templates(mailbox);
        Ok(self)
    }

    /// Reads the names of the templates of the draft's account. A refusal leaves none, shown in the error banner: the
    /// draft is made by then, and the composer stays open over it without the menu.
    pub(crate) fn load_templates(&mut self, mailbox: &mut Mailbox) {
        self.templates = match mailbox.run("template.list", &json!({ "account": self.account })) {
            Ok(answer) => answer
                .get("templates")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(|template| template.get("name")?.as_str()).map(str::to_owned).collect())
                .unwrap_or_default(),
            Err(error) => {
                mailbox.fail(error);
                Vec::new()
            }
        };
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
            later: None,
            fill_refused: None,
            painted_in: None,
            scripted: false,
            drawn_docked: None,
            folded: false,
            saved: None,
            addresses: false,
        };
        composer.take_answer(answer);
        Ok(composer)
    }

    /// Reads the draft as the engine kept it: the account, the chips checked, the subject, the text and what blocks
    /// Send. What is being typed in a recipient line stays. Cc and Bcc show from then on when either holds a chip or
    /// typed text, so nothing the draft holds is in a line that is not drawn.
    fn take_answer(&mut self, answer: &Value) {
        self.account = answer.get("account").and_then(Value::as_u64).unwrap_or(self.account);
        self.saved = answer.get("saved").and_then(Value::as_str).and_then(parse_date).or(self.saved);
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
        if let Some(typed) = answer.get("typed").and_then(Value::as_array) {
            for (line, text) in Line::ALL.iter().zip(typed) {
                if let (Some(field), Some(text)) = (self.typing.get_mut(line.index()), text.as_str()) {
                    text.clone_into(field);
                }
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
        self.copies |= [Line::Cc, Line::Bcc].into_iter().any(|line| !self.chips(line).is_empty() || !self.typing(line).trim().is_empty());
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

    /// The subject or the text changed in their field: they are kept at the next [`Composer::save`]. Their fields hold
    /// them to what the engine keeps (the subject [`MAX_SUBJECT_BYTES`], the text 1 MiB, under [`MAX_TEXT_BYTES`]),
    /// refusing whole what would pass it, so nothing is cut here.
    pub fn edited(&mut self) {
        self.changed = true;
    }

    /// Whether something changed since the draft was last kept.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Whether every chip shows its address, not the name ([`Composer::show_addresses`]).
    #[must_use]
    pub fn addresses(&self) -> bool {
        self.addresses
    }

    /// Makes every chip show its address, not the name, until the composer closes: the draft of a refused send that
    /// changed since, opened to be sent from here.
    pub fn show_addresses(&mut self) {
        self.addresses = true;
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

    /// The recipient lines the composer draws: To, and Cc and Bcc once they show. The drawing and Send both read it, so
    /// nothing becomes a recipient from a line that is not drawn.
    #[must_use]
    pub fn drawn(&self) -> &'static [Line] {
        if self.copies { &Line::ALL } else { &[Line::To] }
    }

    /// Whether it is the floating panel.
    #[must_use]
    pub fn floating(&self) -> bool {
        self.floating
    }

    /// Makes it the floating panel, or puts it back in the reader's place.
    pub fn set_floating(&mut self, floating: bool) {
        if self.floating != floating {
            self.unsee();
        }
        self.floating = floating;
        if !floating {
            self.folded = false;
        }
    }

    /// Whether the floating panel is folded to its head.
    #[must_use]
    pub fn folded(&self) -> bool {
        self.folded
    }

    /// Folds the floating panel to its head, or opens it again.
    pub fn toggle_fold(&mut self) {
        self.folded = !self.folded;
        self.unsee();
    }

    /// When the engine last kept the draft, if it said.
    #[must_use]
    pub fn saved(&self) -> Option<Timestamp> {
        self.saved
    }

    /// Whether the whole composer, its lines in sight, was painted in a frame before `frame` since it was opened,
    /// moved to another place or changed from outside its fields, so that the person has seen what Send would send.
    #[must_use]
    pub fn seen(&self, frame: u64) -> bool {
        self.scripted || self.painted_in.is_some_and(|painted| painted < frame)
    }

    /// The whole composer, its lines in sight, was painted in `frame`.
    pub(crate) fn mark_painted(&mut self, frame: u64) {
        if self.painted_in.is_none() {
            self.painted_in = Some(frame);
        }
    }

    /// What a script set up counts as seen: the shots set a scene without a frame.
    pub(crate) fn mark_seen(&mut self) {
        self.scripted = true;
    }

    /// It is drawn in the reader's place (`docked`) or floating: a place other than the last one has not been seen.
    pub(crate) fn drawn_in(&mut self, docked: bool) {
        if self.drawn_docked != Some(docked) {
            self.unsee();
        }
        self.drawn_docked = Some(docked);
    }

    /// What it shows changed in a way the person has not seen yet (it moved, a template or a fill went in, the
    /// sender changed): Send waits for a frame that paints it.
    pub(crate) fn unsee(&mut self) {
        self.painted_in = None;
        self.scripted = false;
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
        self.blocks.is_empty() && Line::ALL.iter().all(|line| self.refusal(*line).is_none())
    }

    /// What `line` refused of what was typed in it, as its error line says it: addresses past a line's
    /// [`MAX_RECIPIENTS`], an address past [`MAX_RECIPIENT_BYTES`]. They stay typed; none once the typed text is gone.
    #[must_use]
    pub fn refusal(&self, line: Line) -> Option<String> {
        let refused = self.assess(line, self.typing(line));
        if refused.too_long {
            Some(tr!("An address is too long: it holds at most {size} bytes, and it was not added.", size = MAX_RECIPIENT_BYTES))
        } else if refused.over == 1 {
            Some(tr!("1 address not added: a line holds at most {most}.", most = MAX_RECIPIENTS))
        } else if refused.over > 1 {
            Some(tr!("{count} addresses not added: a line holds at most {most}.", count = refused.over, most = MAX_RECIPIENTS))
        } else {
            None
        }
    }

    /// Why Send is blocked, as the composer says it; none when it is not.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        if let Some(refused) = Line::ALL.iter().find_map(|line| self.refusal(*line)) {
            return Some(refused);
        }
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

    /// Whether [`Composer::problem`] only waits for something still to do (a recipient, a blank to fill in) rather than
    /// saying something is wrong: the composer says the first in `muted`, the second in `minus` with a warning.
    #[must_use]
    pub fn only_waits(&self) -> bool {
        let wrong =
            Line::ALL.iter().any(|line| self.refusal(*line).is_some()) || Line::ALL.iter().flat_map(|line| self.chips(*line)).any(|chip| !chip.ok);
        let waiting = self.blocks.iter().all(|block| block == "no_recipient" || block == "blank");
        !wrong && waiting
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

    /// Why the last fill was refused, as the blank's field says it under itself.
    #[must_use]
    pub fn fill_refused(&self) -> Option<&str> {
        self.fill_refused.as_ref().filter(|(_, filling)| *filling == self.filling).map(|(said, _)| said.as_str())
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
        let (subject, text) = (self.subject.replace(&mark, typed), self.text.replace(&mark, typed));
        // Refused whole, said under the blank's field as a field says it: the blank and what was typed stay.
        let refused = if subject.len() > MAX_SUBJECT_BYTES && subject.len() > self.subject.len() {
            Some(crate::component::field::Cap::Bytes(MAX_SUBJECT_BYTES))
        } else if text.len() > MAX_TEXT_BYTES && text.len() > self.text.len() {
            Some(crate::component::field::Cap::Bytes(MAX_TEXT_BYTES))
        } else {
            None
        };
        self.fill_refused = refused.map(|cap| (cap.sentence(), self.filling.clone()));
        if self.fill_refused.is_some() {
            return Ok(());
        }
        self.subject = subject;
        self.text = text;
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
        // Kept with the draft at the end of the frame, as a change of the subject or the text is.
        self.changed = true;
        let typed = self.typing(line).to_owned();
        if let Some(at) = typed.rfind(SEPARATORS) {
            // Through the last separator, which stays with what the line refuses; what follows is typed on.
            let cut = at.saturating_add(1).min(typed.len());
            let (done, rest) = (typed.get(..cut).unwrap_or(typed.as_str()), typed.get(cut..).unwrap_or_default());
            let before = self.chips(line).len();
            let kept = self.add(line, done);
            if let Some(field) = self.typing_mut(line) {
                *field = if kept.is_empty() { rest.trim_start().to_owned() } else { kept + rest };
            }
            // A chip made is checked by the engine at once; refused text alone is kept at the frame's end.
            if self.chips(line).len() != before {
                self.save(mailbox)?;
            }
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
                let kept = self.add(line, &typed);
                if let Some(field) = self.typing_mut(line) {
                    *field = kept;
                }
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
        let kept = self.add(line, &suggestion.recipient());
        if let Some(field) = self.typing_mut(line) {
            *field = kept;
        }
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

    /// Adds the recipients in `typed` to `line`, split at the separators: each once across the lines. What the line
    /// cannot take (past its [`MAX_RECIPIENTS`], or an address past [`MAX_RECIPIENT_BYTES`]) is never cut or dropped:
    /// it is given back exactly as it was typed (its separators, the spaces, a quoted name's comma), what was taken
    /// left out, for the line's typed text, and the line says why ([`Composer::refusal`]). What comes back is part of
    /// `typed`, so it is never longer.
    fn add(&mut self, line: Line, typed: &str) -> String {
        let mut held: BTreeSet<String> = Line::ALL.iter().flat_map(|line| self.chips(*line)).map(|chip| split(&chip.text).1.to_lowercase()).collect();
        let Some(chips) = self.lines.get_mut(line.index()) else { return typed.to_owned() };
        // Each part with the separator after it: `(start, end of the part, end of its separator)`.
        let mut spans = Vec::new();
        let mut start = 0;
        for (at, c) in typed.char_indices() {
            if SEPARATORS.contains(&c) {
                let through = at.saturating_add(c.len_utf8());
                spans.push((start, at, through));
                start = through;
            }
        }
        spans.push((start, typed.len(), typed.len()));
        let mut kept = String::new();
        let mut keeping = false;
        for (from, to, through) in spans {
            let whole = typed.get(from..through).unwrap_or_default();
            let part = typed.get(from..to).unwrap_or_default().trim();
            if part.is_empty() {
                // Separators after a part kept stay with it.
                if keeping {
                    kept.push_str(whole);
                }
                continue;
            }
            let text = part.chars().filter(|c| !c.is_control()).collect::<String>();
            let address = split(&text).1.to_lowercase();
            keeping = text.len() > MAX_RECIPIENT_BYTES || (!held.contains(&address) && chips.len() >= MAX_RECIPIENTS);
            if keeping {
                kept.push_str(whole);
            } else if held.insert(address) {
                // Checked by the engine once kept; until then it counts as not an address.
                chips.push(Chip { text, ok: false });
                self.changed = true;
            }
        }
        kept.trim_start().to_owned()
    }

    /// What a line would refuse of `typed` (it stays typed): addresses past [`MAX_RECIPIENT_BYTES`], and those no line
    /// holds yet while this one holds [`MAX_RECIPIENTS`]. Worked out from the text each time, so it holds after the
    /// draft is opened again, and it changes as soon as a chip goes or the text is edited.
    fn assess(&self, line: Line, typed: &str) -> Refused {
        // Room for so many more: the addresses typed past it are the ones not added. Asked every frame the line is
        // drawn, so the chips are looked at only when more parts are typed than there is room for.
        let room = MAX_RECIPIENTS.saturating_sub(self.chips(line).len());
        let parts = typed.split(SEPARATORS).filter(|part| !part.trim().is_empty()).count();
        let full = parts > room;
        let held: BTreeSet<String> = if full {
            Line::ALL.iter().flat_map(|line| self.chips(*line)).map(|chip| split(&chip.text).1.to_lowercase()).collect()
        } else {
            BTreeSet::new()
        };
        let mut counted = BTreeSet::new();
        let mut refused = Refused::default();
        let mut new = 0_usize;
        for part in typed.split(SEPARATORS).map(str::trim).filter(|part| !part.is_empty()) {
            let text = part.chars().filter(|c| !c.is_control()).collect::<String>();
            if text.len() > MAX_RECIPIENT_BYTES {
                refused.too_long = true;
            } else if full {
                let address = split(&text).1.to_lowercase();
                if !held.contains(&address) && counted.insert(address) {
                    new = new.saturating_add(1);
                }
            }
        }
        refused.over = new.saturating_sub(room);
        refused
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
        let found = people(mailbox, &query, skip)?;
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
            // What is typed and not a recipient: kept with the draft, so closing or a kill loses none of it.
            "typed": self.typing,
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
        self.load_templates(mailbox);
        Ok(())
    }

    /// Before the composer closes: what is typed in a drawn recipient line becomes a chip and the draft is kept. Text in
    /// a line not drawn stays typed text: it is never made a recipient unseen.
    ///
    /// # Errors
    ///
    /// The command's refusal; then the composer stays open with everything in it.
    pub fn close(&mut self, mailbox: &mut Mailbox) -> Result<(), UiError> {
        for &line in self.drawn() {
            let typed = self.typing.get_mut(line.index()).map(std::mem::take).unwrap_or_default();
            let kept = self.add(line, &typed);
            if let Some(field) = self.typing_mut(line) {
                *field = kept;
            }
        }
        self.dismiss_suggestions();
        self.save(mailbox)
    }

    /// The time picked to send the draft at, if one is: Send is Schedule then.
    #[must_use]
    pub fn later(&self) -> Option<Timestamp> {
        self.later
    }

    /// Sends the draft at `at` instead of now, or now again with `None`.
    pub fn set_later(&mut self, at: Option<Timestamp>) {
        self.later = at;
    }

    /// Sends the draft after keeping it as [`Composer::close`] does: after the hold (`mail.send`), or at the time
    /// picked (`mail.send_later`). The time it goes, as the engine put it in the outbox; `None` for a draft that is
    /// blocked, which is kept and not sent. The lists are read again, so the outbox shows it.
    ///
    /// # Errors
    ///
    /// A command's refusal (an account without a submission server, a time picked that has passed); text typed in a
    /// line that is not drawn, which is then shown and not sent ([`UiError::Refused`]).
    pub fn send(&mut self, mailbox: &mut Mailbox) -> Result<Option<Timestamp>, UiError> {
        // Nothing the owner was not shown becomes a recipient: a line not drawn that holds typed text is shown, and
        // Send waits for the next press.
        if let Some(hidden) = Line::ALL.into_iter().find(|line| !self.drawn().contains(line) && !self.typing(*line).trim().is_empty()) {
            self.show_copies();
            let said = tr!("{line} holds text that was not shown: look at it, then send.", line = hidden.name());
            return Err(UiError::Refused(said));
        }
        self.close(mailbox)?;
        if !self.sendable() {
            return Ok(None);
        }
        let answer = match self.later {
            Some(at) => mailbox.run("mail.send_later", &json!({ "id": self.draft, "at": engine_date(at) }))?,
            None => mailbox.run("mail.send", &json!({ "id": self.draft }))?,
        };
        mailbox.refresh();
        Ok(Some(answer.get("at").and_then(Value::as_str).and_then(parse_date).unwrap_or_else(|| mailbox.now())))
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

/// The people in the mail whose name or address holds `query` (case left aside), at most [`MAX_SUGGESTIONS`], none
/// whose address (lower case) is in `skip`: the composer's suggestions, and the command line's `@`.
///
/// # Errors
///
/// The engine's refusal of `mail.search`.
pub(crate) fn people(mailbox: &mut Mailbox, query: &str, mut skip: BTreeSet<String>) -> Result<Vec<Suggestion>, UiError> {
    let query = query.trim().to_owned();
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
    Ok(found)
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
