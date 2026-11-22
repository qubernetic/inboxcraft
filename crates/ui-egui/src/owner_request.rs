// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The owner requests: `owner` commands an agent asked the owner to allow once, over the control channel, waiting for
//! the owner's answer in the window.
//!
//! One question at a time: only the request in front is asked, and only its two minutes ([`PATIENCE`]) run, from the
//! moment it came to the front. At most [`MOST_WAITING`] wait; one more is turned away at once. A request is settled
//! only by the owner's answer or by its time running out, which is a refusal: nothing here runs a command, and
//! nothing but [`OwnerRequests::settle_front`] takes a request out of the queue.
//!
//! Only a command whose question the app builds is asked ([`asks_with_own_question`]): today `calendar.import`. The
//! question's words are the app's ([`preview`]): built once, when the request comes to the front, from the engine's
//! reading of the request (`calendar.import.preview`) and `tr!`. What came from outside (the name the client gave
//! itself, an event's title, the calendar's name) is data: plain, cut, and only inside typographic quotes, which it
//! cannot close ([`data`]).

use std::collections::VecDeque;
use std::time::Duration;

use inboxcraft_engine::EngineError;
use inboxcraft_time::Timestamp;
use serde_json::Value;

use crate::component::dialog::Tone;
use crate::mailbox::Mailbox;
use crate::tr;
use crate::unicode_class::{Class, class};

/// The most requests that wait for the owner at a time.
pub const MOST_WAITING: usize = 8;

/// How long the owner has to answer the request in front; no answer by then is a refusal.
pub const PATIENCE: Duration = Duration::from_secs(120);

/// An Allow sooner than this many seconds after the question showed is no answer: a key or a press on its way when it
/// opened lands on no question.
pub(crate) const ARM_SECONDS: f64 = 1.0;

/// The most events of an import named in its question; the rest are counted.
pub(crate) const FIRST: usize = 5;

/// The most characters of an event's title or a calendar's name in a question.
pub(crate) const MOST_NAME_CHARS: usize = 80;

/// Whether the owner can be asked to allow `command` once: only an `owner` command whose question the app builds
/// itself, from its own reading of the request, is asked ([`preview`]). Today that is `calendar.import` alone; a
/// later command is asked once it has a preview of its own. Any other `owner` command asked for is refused at once,
/// unqueued and unasked: a question of its bare parameters could leave out the one that matters.
#[must_use]
pub fn asks_with_own_question(command: &str) -> bool {
    command == "calendar.import"
}

/// Which request a settlement answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ticket(u64);

/// How a request ended.
#[derive(Debug)]
pub enum Settled {
    /// The owner allowed it: the command ran, with this answer or refusal.
    Ran(Result<Value, EngineError>),
    /// The owner denied it; nothing ran.
    Denied,
    /// The owner did not answer within [`PATIENCE`]; nothing ran.
    TimedOut,
}

/// One request waiting for the owner.
#[derive(Debug)]
pub(crate) struct OwnerRequest {
    /// Which request it is.
    pub(crate) ticket: Ticket,
    /// The name the asking client gave itself, as the channel knows it: the agent's words, never the app's.
    pub(crate) client: Option<String>,
    /// The `owner` command asked for.
    pub(crate) command: &'static str,
    /// Its parameters, as the agent sent them.
    pub(crate) params: Value,
    /// When it came to the front, so its two minutes run; `None` until [`OwnerRequests::expire`] first sees it there.
    pub(crate) since: Option<Timestamp>,
    /// Asked right before an Allow runs it: false when the asker withdrew it meanwhile, so it runs not.
    pub(crate) may_run: MayRun,
    /// The question's words, built once when it is first asked ([`preview`]): the file is not read again each frame.
    pub(crate) preview: Option<Preview>,
    /// When the window first showed its question, on egui's clock: an Allow counts from [`ARM_SECONDS`] after.
    pub(crate) shown_at: Option<f64>,
}

/// The question's words, built by the app once, when the request comes to the front.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Preview {
    /// The question, the dialog's heading.
    pub(crate) question: Said,
    /// The lines under it.
    pub(crate) lines: Vec<Said>,
    /// The calendar's name as the question quotes it, for the status line's sentence once it ran.
    calendar: String,
    /// The words as last fitted to the card ([`Fitted`]).
    pub(crate) fitted: Option<Fitted>,
}

/// One sentence of the question, as the app wrote it: its words, its colour, and how the data quoted in it stands.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Said {
    /// The words, the data in them between the one `“` and the one `”`.
    pub(crate) text: String,
    /// Its colour, for a line.
    pub(crate) tone: Tone,
    /// Where it holds data in quotes, how the data must stand when drawn; `None` for words of the app's alone.
    pub(crate) data: Option<Quoted>,
}

impl Said {
    /// The app's own words, no data in them.
    fn plain(text: String, tone: Tone) -> Self {
        Self { text, tone, data: None }
    }

    /// Words with data quoted in them that stands as `quoted` says.
    fn quoting(text: String, tone: Tone, quoted: Quoted) -> Self {
        Self { text, tone, data: Some(quoted) }
    }
}

/// How the data quoted in a sentence stands when drawn: never with a row of it apart from its opening quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Quoted {
    /// The whole line on one row (an event, a value): the data cut with `…` to make room for the rest.
    Row,
    /// The quoted run on one row, with both its quotes (a name in a sentence that wraps): cut with `…` where a row
    /// cannot hold it.
    Run,
}

/// The question's words as fitted to the card of one width, in one type ([`crate::draw`] fits them).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fitted {
    /// What they were fitted for: the words' width, the window's pixels per point, the lines' and the heading's type.
    pub(crate) key: (f32, f32, crate::theme::Font, crate::theme::Font),
    /// The heading.
    pub(crate) question: String,
    /// The lines.
    pub(crate) lines: Vec<(String, Tone)>,
}

impl Preview {
    /// The status line's sentence for the import that ran and gave `answer`: "An agent added {count} events to
    /// “{calendar}”", the count the import answers.
    #[must_use]
    pub(crate) fn done(&self, answer: &Value) -> String {
        let count = answer.get("added").and_then(Value::as_u64).unwrap_or(0);
        tr!("An agent added {count} events to “{calendar}”", count = count, calendar = self.calendar)
    }
}

/// Whether `c` is a mark data could close its own quotes with, or open new ones, whatever kind of character it is:
/// the question's own `“` and `”`, the other quotes, and the marks drawn like a double quote (double primes, the
/// double acute, the diaeresis, the modifier letters, the Hebrew gershayim, the ditto mark, the quote ornaments) or
/// like an apostrophe (two of which read as one double quote: the turned comma, the tonos, primes, accents, the
/// saltillo, the Armenian apostrophe), and the combining marks drawn like either wherever they stand (the double
/// acute, the double grave, the double vertical line and the commas above). [`data`] counts every initial and final
/// quote, modifier letter and modifier symbol among them too ([`Class::QuoteLike`]).
pub(crate) fn quote(c: char) -> bool {
    matches!(
        c,
        '"' | '\''
            | '“'
            | '”'
            | '„'
            | '‟'
            | '«'
            | '»'
            | '‹'
            | '›'
            | '‘'
            | '’'
            | '‚'
            | '‛'
            | '＂'
            | '＇'
            | '〝'
            | '〞'
            | '〟'
            | '″'
            | '‴'
            | '‶'
            | '‷'
            | '˝'
            | 'ʺ'
            | 'ˮ'
            | 'ʼ'
            | 'ʻ'
            | 'ʹ'
            | '′'
            | '‵'
            | '´'
            | '`'
            | '¨'
            | '΄'
            | '\u{030B}'
            | '\u{030E}'
            | '\u{030F}'
            | '\u{0312}'..='\u{0315}'
            | '\u{0343}'
            | '\u{055A}'
            | '״'
            | '׳'
            | '〃'
            | '❛'
            | '❜'
            | '❝'
            | '❞'
            | '\u{275F}'
            | '\u{2760}'
            | '\u{276E}'
            | '\u{276F}'
            | '\u{2E42}'
            | '\u{A78B}'
            | '\u{A78C}'
            | '\u{1F676}'..='\u{1F678}'
    )
}

/// The letter that `letter` with a double acute on it is, where Unicode has it as one character (Hungarian ő and ű):
/// the one place the combining double acute, a quote mark anywhere else, is kept.
fn double_acute(letter: char) -> Option<char> {
    match letter {
        'o' => Some('ő'),
        'O' => Some('Ő'),
        'u' => Some('ű'),
        'U' => Some('Ű'),
        _ => None,
    }
}

/// `text` from outside as a question shows it. The characters that must not be drawn are left out
/// ([`crate::component::hidden`]). Of the rest only these stand, an allow list by kind ([`class`], Unicode 15.1):
/// letters, numbers, punctuation and symbols (emoji among them) as they are; a combining mark only directly on a
/// letter or a number, or on a mark kept there; every quote mark ([`quote`]) and every initial or final quote, modifier
/// letter and modifier symbol as one apostrophe `'`, a run of them as one, so data can never close the quotes the
/// question puts around it nor draw one of its own; anything else (private use, unassigned) not at all. Each run of
/// white space or control characters (no-break and wide spaces among it) is one space, trimmed, and at most `most`
/// characters stand, cut between grapheme clusters with a closing `…`. Only the first 16 `most` characters that are
/// drawn are read: longer data is cut anyway.
#[must_use]
pub(crate) fn data(text: &str, most: usize) -> String {
    let mut plain = String::new();
    let mut gap = false;
    // Whether a combining mark may stand on the character kept last: a letter, a number, or a mark kept on one.
    let mut base = false;
    for c in text.chars().filter(|c| !crate::component::hidden(*c)).take(most.saturating_mul(16)) {
        if c.is_whitespace() || c.is_control() {
            gap = true;
            continue;
        }
        if c == '\u{030B}'
            && !gap
            && let Some(composed) = plain.chars().next_back().and_then(double_acute)
        {
            plain.pop();
            plain.push(composed);
            continue;
        }
        let kind = if quote(c) { Class::QuoteLike } else { class(c) };
        match kind {
            Class::Letter | Class::Number | Class::Punctuation | Class::Symbol | Class::QuoteLike => {}
            // A mark with nothing to stand on (at the start, after a space, punctuation, a symbol or an apostrophe)
            // would draw as a mark of its own; the space before it is still to come.
            Class::Mark if base && !gap => {}
            Class::Mark | Class::Other => continue,
        }
        if gap && !plain.is_empty() {
            plain.push(' ');
        }
        gap = false;
        base = matches!(kind, Class::Letter | Class::Number | Class::Mark);
        if kind == Class::QuoteLike {
            // A run of marks, or two with only what is not drawn or a dropped mark between them, is one apostrophe.
            if !plain.ends_with('\'') {
                plain.push('\'');
            }
        } else {
            plain.push(c);
        }
    }
    match crate::component::capped(&plain, most) {
        (kept, true) => format!("{}…", kept.trim_end()),
        (kept, false) => kept.to_owned(),
    }
}

/// The words of the question for `request`, or the engine's refusal of a request that cannot be previewed (an import
/// whose file or calendar the engine refuses). An import is read once by `calendar.import.preview`, which is `safe`
/// and changes nothing.
///
/// # Errors
///
/// The engine's refusal of `calendar.import.preview`; [`EngineError::NotAllowed`] for a command whose question the
/// app does not build ([`asks_with_own_question`]), which is never queued.
pub(crate) fn preview(mailbox: &mut Mailbox, request: &OwnerRequest) -> Result<Preview, EngineError> {
    if !asks_with_own_question(request.command) {
        return Err(EngineError::NotAllowed { command: request.command, class: inboxcraft_engine::AgentClass::Owner });
    }
    let who = match request.client.as_deref().map(|client| data(client, MOST_NAME_CHARS)).filter(|client| !client.is_empty()) {
        Some(client) => {
            Said::quoting(tr!("A program that calls itself “{client}” asks, over the control channel.", client = client), Tone::Muted, Quoted::Run)
        }
        None => Said::plain(tr!("A program asks, over the control channel.").to_owned(), Tone::Muted),
    };
    let mut lines = vec![who];
    let read = mailbox.read_as_agent("calendar.import.preview", &request.params)?;
    let count = |field: &str| read.get(field).and_then(Value::as_u64).unwrap_or(0);
    let calendar = data(read.get("name").and_then(Value::as_str).unwrap_or_default(), MOST_NAME_CHARS);
    let add = count("add");
    let question = Said::quoting(tr!("Add {count} events to “{calendar}”?", count = add, calendar = calendar), Tone::Ink, Quoted::Run);
    let (skipped, single, left_out) = (count("skipped"), count("single"), count("left_out"));
    if skipped > 0 {
        lines.push(Said::plain(tr!("{count} are already in the calendar and stay as they are.", count = skipped), Tone::Muted));
    }
    if single > 0 {
        lines.push(Said::plain(tr!("{count} repeat in a way InboxCraft cannot write and are added once.", count = single), Tone::Ink));
    }
    if left_out > 0 {
        lines.push(Said::plain(tr!("{count} cannot be read and are left out.", count = left_out), Tone::Ink));
    }
    let first = read.get("first").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let offset = mailbox.offset();
    for event in first.iter().take(FIRST) {
        lines.push(named(event, offset));
    }
    let named_count = u64::try_from(first.len().min(FIRST)).unwrap_or(u64::MAX);
    if add > named_count {
        lines.push(Said::plain(tr!("and {count} more", count = add.saturating_sub(named_count)), Tone::Muted));
    }
    lines.push(Said::plain(tr!("Nothing already in the calendar is changed or removed. Undo takes the events back.").to_owned(), Tone::Muted));
    Ok(Preview { question, lines, calendar, fitted: None })
}

/// The line of one event of an import's preview: its title, quoted, and when it starts, on the clock of `offset`.
fn named(event: &Value, offset: inboxcraft_time::Offset) -> Said {
    let title = data(event.get("summary").and_then(Value::as_str).unwrap_or_default(), MOST_NAME_CHARS);
    let start = event.get("start").and_then(Value::as_str).unwrap_or_default();
    let when = if event.get("all_day").and_then(Value::as_bool) == Some(true) {
        data(start, 10)
    } else {
        crate::mailbox::parse_date(start).and_then(|at| at.to_civil(offset)).map_or_else(String::new, |civil| {
            format!("{:04}-{:02}-{:02} {:02}:{:02}", civil.year(), civil.month(), civil.day(), civil.hour(), civil.minute())
        })
    };
    match (title.is_empty(), when.is_empty()) {
        (true, true) => Said::plain(tr!("(No title)").to_owned(), Tone::Ink),
        (true, false) => Said::plain(tr!("(No title) · {when}", when = when), Tone::Ink),
        (false, true) => Said::quoting(tr!("“{title}”", title = title), Tone::Ink, Quoted::Row),
        (false, false) => Said::quoting(tr!("“{title}” · {when}", title = title, when = when), Tone::Ink, Quoted::Row),
    }
}

/// Whether a request may still run, asked of the side that put it (see [`crate::InboxApp::ask_owner_if`]).
pub(crate) struct MayRun(Box<dyn Fn() -> bool + Send>);

impl MayRun {
    /// The request may run when `check` says so, at the moment it would.
    #[must_use]
    pub(crate) fn new(check: Box<dyn Fn() -> bool + Send>) -> Self {
        Self(check)
    }

    /// Whether the request may run now.
    #[must_use]
    pub(crate) fn check(&self) -> bool {
        (self.0)()
    }
}

impl Default for MayRun {
    /// Always.
    fn default() -> Self {
        Self(Box::new(|| true))
    }
}

impl std::fmt::Debug for MayRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MayRun")
    }
}

/// The requests waiting for the owner, in the order they came, and those settled but not yet answered.
#[derive(Debug, Default)]
pub(crate) struct OwnerRequests {
    waiting: VecDeque<OwnerRequest>,
    settled: Vec<(Ticket, Settled)>,
    next: u64,
}

impl OwnerRequests {
    /// Queues the request of `client` for `command` with `params`; `None` when [`MOST_WAITING`] wait already.
    #[cfg(test)]
    pub(crate) fn push(&mut self, client: Option<&str>, command: &'static str, params: Value) -> Option<Ticket> {
        self.push_if(client, command, params, MayRun::default())
    }

    /// Queues the request of `client` for `command` with `params`, which runs on Allow only when `may_run` says so
    /// then; `None` when [`MOST_WAITING`] wait already.
    pub(crate) fn push_if(&mut self, client: Option<&str>, command: &'static str, params: Value, may_run: MayRun) -> Option<Ticket> {
        if self.waiting.len() >= MOST_WAITING {
            return None;
        }
        let ticket = Ticket(self.next);
        self.next = self.next.wrapping_add(1);
        self.waiting.push_back(OwnerRequest {
            ticket,
            client: client.map(str::to_owned),
            command,
            params,
            since: None,
            may_run,
            preview: None,
            shown_at: None,
        });
        Some(ticket)
    }

    /// The request the owner is asked about.
    pub(crate) fn front(&self) -> Option<&OwnerRequest> {
        self.waiting.front()
    }

    /// The request the owner is asked about, to keep its words and when it showed.
    pub(crate) fn front_mut(&mut self) -> Option<&mut OwnerRequest> {
        self.waiting.front_mut()
    }

    /// Settles the request in front as `settled`; nothing when none waits.
    pub(crate) fn settle_front(&mut self, settled: Settled) {
        if let Some(request) = self.waiting.pop_front() {
            self.settled.push((request.ticket, settled));
        }
    }

    /// Takes the request `ticket` out of the queue without settling it: its asker is gone, and nobody is answered. One
    /// that comes to the front for it is dated on the next [`OwnerRequests::expire`]. Whether it was waiting.
    pub(crate) fn withdraw(&mut self, ticket: Ticket) -> bool {
        let Some(at) = self.waiting.iter().position(|request| request.ticket == ticket) else { return false };
        self.waiting.remove(at);
        if at == 0
            && let Some(next) = self.waiting.front_mut()
        {
            next.since = None;
        }
        true
    }

    /// Dates the request in front when it has no date yet (a clock that went back since dates it not again: it has
    /// waited nothing until the clock is past its date once more, so it never times out early), and
    /// settles it as [`Settled::TimedOut`] once [`PATIENCE`] has gone by since; the one behind it is dated `now`, so its
    /// two minutes run from then. One behind a request the owner answered is dated on the next call.
    pub(crate) fn expire(&mut self, now: Timestamp) {
        let Some(front) = self.waiting.front_mut() else { return };
        let Some(since) = front.since else {
            front.since = Some(now);
            return;
        };
        match now.duration_since(since) {
            Some(waited) if waited >= PATIENCE => {
                self.settle_front(Settled::TimedOut);
                // The one behind it comes to the front now: its two minutes start.
                if let Some(next) = self.waiting.front_mut() {
                    next.since = Some(now);
                }
            }
            Some(_) | None => {}
        }
    }

    /// How long until the request in front runs out of time: `Some(ZERO)` when it has no date yet, `None` when none
    /// waits.
    pub(crate) fn due_in(&self, now: Timestamp) -> Option<Duration> {
        let front = self.waiting.front()?;
        let Some(since) = front.since else { return Some(Duration::ZERO) };
        let waited = now.duration_since(since).unwrap_or_default();
        Some(PATIENCE.saturating_sub(waited))
    }

    /// The requests settled since the last call, oldest first.
    pub(crate) fn take_settled(&mut self) -> Vec<(Ticket, Settled)> {
        std::mem::take(&mut self.settled)
    }

    /// Whether a request waits for the owner.
    pub(crate) fn waiting(&self) -> bool {
        !self.waiting.is_empty()
    }
}

#[cfg(test)]
#[path = "owner_request_tests.rs"]
pub(crate) mod tests;
