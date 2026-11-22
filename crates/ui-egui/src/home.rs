// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What Home shows (`docs/design.md`, "Home"), as plain data: the headline's parts, four counts, the rest of today's
//! agenda, the drafts to pick up, which two buttons, and whether something is failing. [`build`] reads it from the
//! mailbox through `safe` commands only; [`crate::draw::home`] draws it and runs nothing.
//!
//! Everything here comes from outside (a calendar's titles, a draft's subject and recipients): every text is made
//! plain and capped at [`MOST`] characters while the model is built, and only the lines Home shows are kept ([`AGENDA`]
//! agenda lines and [`DRAFTS`] drafts), with how many there are in all, so 10 000 drafts cost Home two rows.

use inboxcraft_model::Role;
use inboxcraft_time::Timestamp;
use serde_json::{Value, json};

use crate::calendar::{Day, EventRow, Moment};
use crate::mailbox::{Mailbox, SendState, month_text};
use crate::tr;

/// The most characters of an outside text the model keeps.
pub const MOST: usize = crate::component::MOST;

/// The most agenda lines Home shows; the rest is "N more today".
pub const AGENDA: usize = 3;

/// The most drafts Home shows; the rest is behind "all drafts (N)".
pub const DRAFTS: usize = 2;

/// Which form of Home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// There is an account: the brief of today.
    Brief,
    /// Before the first account: what InboxCraft is and what adding one does.
    FirstRun,
}

/// The time of day the headline greets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Greeting {
    /// Before 12:00.
    Morning,
    /// From 12:00 to 18:00.
    Afternoon,
    /// From 18:00.
    Evening,
}

impl Greeting {
    /// The greeting of the hour `hour` (0 to 23).
    #[must_use]
    pub const fn at(hour: u8) -> Self {
        match hour {
            0..12 => Self::Morning,
            12..18 => Self::Afternoon,
            _ => Self::Evening,
        }
    }

    /// Its words, with the comma the name follows: "Good afternoon,".
    #[must_use]
    pub fn text(self, named: bool) -> &'static str {
        match (self, named) {
            (Self::Morning, true) => tr!("Good morning,"),
            (Self::Afternoon, true) => tr!("Good afternoon,"),
            (Self::Evening, true) => tr!("Good evening,"),
            (Self::Morning, false) => tr!("Good morning."),
            (Self::Afternoon, false) => tr!("Good afternoon."),
            (Self::Evening, false) => tr!("Good evening."),
        }
    }
}

/// What a count counts; a press on its tile asks for its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counted {
    /// The unread mail of the inbox.
    ToRead,
    /// The senders the screener holds back.
    ToScreen,
    /// The snoozed mail that comes back today.
    BackToday,
    /// The sends in the outbox.
    Outbox,
    /// The sends the server refused.
    NotSent,
}

/// One of the four counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    /// What it counts.
    pub what: Counted,
    /// How many; `None` when it could not be read (the tile then shows its empty form, `0` in `muted`, under its
    /// plain label).
    pub count: Option<u64>,
}

/// A line of the rest of today's agenda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaLine {
    /// Its start: `15:00`.
    pub time: String,
    /// Its title.
    pub title: String,
    /// Where and how long: `Room 3 · 1 h`.
    pub detail: String,
    /// Its account's place, which picks the bar's colour.
    pub account: u64,
}

/// A draft to pick up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftLine {
    /// The draft's id.
    pub id: u64,
    /// Its subject; `None` when it has none.
    pub subject: Option<String>,
    /// Whom it goes to: "to Sunniva Aalto", "no recipient yet".
    pub to: String,
    /// When it was last edited: "edited 12:10".
    pub when: String,
}

/// What is failing, said in the banner that stands in the lede's place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failing {
    /// What the banner says.
    pub text: String,
    /// Whether it offers "open account".
    pub account: bool,
    /// Whether it offers "open outbox".
    pub outbox: bool,
}

/// The two buttons under the brief.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buttons {
    /// Read mail (primary), Write.
    ReadAndWrite,
    /// Write (primary), Open the calendar: nothing to read.
    WriteAndCalendar,
    /// Add an account (primary), Look around with sample mail.
    FirstRun,
}

/// What Home shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    /// Which form.
    pub kind: Kind,
    /// The section label's word: "Today · Mon 16 Nov".
    pub date: String,
    /// The headline's greeting.
    pub greeting: Greeting,
    /// The word of the owner's sending name the headline greets by (the first word by default, `greeting.set`), its
    /// second line; `None` when no account's name gives one or the owner chose no name.
    pub name: Option<String>,
    /// The sentence under the headline.
    pub lede: String,
    /// What is failing; `None` when nothing is.
    pub failing: Option<Failing>,
    /// The four counts, in their order.
    pub counts: [Count; 4],
    /// The first lines of the rest of today's agenda (at most [`AGENDA`]).
    pub agenda: Vec<AgendaLine>,
    /// How many events are left today in all.
    pub agenda_total: u64,
    /// The first event of tomorrow, when the agenda is empty: its time and title.
    pub tomorrow: Option<(String, String)>,
    /// The newest drafts (at most [`DRAFTS`]).
    pub drafts: Vec<DraftLine>,
    /// How many drafts there are in all.
    pub drafts_total: u64,
    /// Which buttons.
    pub buttons: Buttons,
    /// When the first of the rest of today's events ends: the agenda changes then, so Home is built again.
    pub until: Option<Moment>,
}

/// `text` as the model keeps it: plain (no control characters, no bidi overrides) and at most [`MOST`] characters,
/// with one `…` where it was cut.
#[must_use]
pub fn kept(text: &str) -> String {
    let (kept, cut) = crate::component::capped(text, MOST);
    let plain = crate::component::plain(kept);
    if cut { format!("{}…", plain.trim_end()) } else { plain.into_owned() }
}

impl Home {
    /// Home before the first account.
    #[must_use]
    pub fn first_run() -> Self {
        Self {
            kind: Kind::FirstRun,
            date: tr!("Welcome").to_owned(),
            greeting: Greeting::Morning,
            name: None,
            lede: tr!(
                "InboxCraft reads and sends mail through your own IMAP and SMTP servers and keeps a copy on this computer. Nothing goes through anyone else, and nothing in a message runs."
            )
            .to_owned(),
            failing: None,
            counts: counts(Some(0), Some(0), Some(0), Some(0), 0),
            agenda: Vec::new(),
            agenda_total: 0,
            tomorrow: None,
            drafts: Vec::new(),
            drafts_total: 0,
            buttons: Buttons::FirstRun,
            until: None,
        }
    }

    /// The headline's two lines: the greeting and the name (`None` for one line).
    #[must_use]
    pub fn headline(&self) -> (&str, Option<&str>) {
        match self.kind {
            Kind::FirstRun => (tr!("Your mail,"), Some(tr!("on your machine."))),
            Kind::Brief => (self.greeting.text(self.name.is_some()), self.name.as_deref()),
        }
    }

    /// Whether nothing waits: every count read and 0 (a count that could not be read is not 0), no event left today
    /// and no draft.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.counts.iter().all(|count| count.count == Some(0)) && self.agenda_total == 0 && self.drafts_total == 0
    }
}

/// The four counts in their order; the fourth is "not sent" while the server refused a send.
fn counts(read: Option<u64>, screen: Option<u64>, back: Option<u64>, outbox: Option<u64>, refused: u64) -> [Count; 4] {
    let last = if refused > 0 { Count { what: Counted::NotSent, count: Some(refused) } } else { Count { what: Counted::Outbox, count: outbox } };
    [
        Count { what: Counted::ToRead, count: read },
        Count { what: Counted::ToScreen, count: screen },
        Count { what: Counted::BackToday, count: back },
        last,
    ]
}

/// The sentence under the headline, from the counts and the next event. A count that could not be read (`None`) is
/// not 0: the sentence leaves it out and claims nothing of it (the tile shows its unread form).
fn lede(read: Option<u64>, screen: Option<u64>, back: Option<u64>, next: Option<&AgendaLine>, quiet: bool) -> String {
    if quiet {
        return tr!("Nothing waits for you. The inbox is read, nobody is at the door, and the day's events are over.").to_owned();
    }
    let n = crate::component::label::count;
    let some = |count: Option<u64>| count.filter(|count| *count > 0);
    let mut parts = Vec::new();
    if let Some(read) = some(read) {
        parts.push(tr!("{n} to read", n = n(read)));
    }
    if let Some(screen) = some(screen) {
        parts.push(tr!("{n} waiting at the door", n = n(screen)));
    }
    if let Some(back) = some(back) {
        parts.push(tr!("{n} back today", n = n(back)));
    }
    if let Some(next) = next {
        parts.push(tr!("{title} at {time}", title = next.title, time = next.time));
    }
    match parts.split_last() {
        None => match (read, screen) {
            (Some(_), Some(_)) => tr!("Nothing waits for you in the inbox.").to_owned(),
            (Some(_), None) => tr!("The inbox is read, and the day's events are over.").to_owned(),
            (None, Some(_)) => tr!("Nobody is at the door, and the day's events are over.").to_owned(),
            (None, None) => tr!("The day's events are over.").to_owned(),
        },
        Some((last, [])) => tr!("{last}.", last = last),
        Some((last, rest)) => tr!("{rest}, and {last}.", rest = rest.join(", "), last = last),
    }
}

/// An event's length: `30 min`, `1 h`, `1 h 30 min`.
fn length(minutes: i64) -> String {
    let (hours, rest) = (minutes / 60, minutes % 60);
    match (hours, rest) {
        (0, rest) => tr!("{m} min", m = rest),
        (hours, 0) => tr!("{h} h", h = hours),
        (hours, rest) => tr!("{h} h {m} min", h = hours, m = rest),
    }
}

/// Home as the mailbox has it now, read through `safe` commands only: the counts from the inbox's unread mail, the
/// screener's senders, `mail.list` of the snoozed mail and the outbox; the drafts through `compose.list`; the rest of
/// today through `event.list` of today and tomorrow and `calendar.list`; what is failing through `sync.status` and
/// `account.list`. A refused read leaves its part empty (a count `None`), never made
/// up. Runs commands: call it when the mailbox changed, not every frame.
pub fn build(mailbox: &mut Mailbox) -> Home {
    build_hiding(mailbox, None)
}

/// Home as [`build`] makes it, with the draft `hidden` left out of the drafts' block and its count: a discard waits
/// behind its toast ([`Kept::current`]).
pub fn build_hiding(mailbox: &mut Mailbox, hidden: Option<u64>) -> Home {
    build_seeing(mailbox, hidden, &mut Vec::new())
}

/// Home as [`build_hiding`] makes it, with `away` the accounts an earlier build saw offline or backing off: one of them
/// connecting again is still away ([`failing`]); `away` is left with the accounts this build saw away.
fn build_seeing(mailbox: &mut Mailbox, hidden: Option<u64>, away: &mut Vec<u64>) -> Home {
    if mailbox.accounts().is_empty() {
        away.clear();
        return Home::first_run();
    }
    let now = mailbox.now();
    let offset = mailbox.offset();
    let civil = now.to_civil(offset);
    let today = Day::of(now, offset);
    let read = mailbox.views().iter().find(|view| view.role == Role::Inbox).map(|view| view.unread);
    let screen = u64::try_from(mailbox.waiting().len()).ok();
    let back = back_today(mailbox, today, offset);
    let outbox = u64::try_from(mailbox.outbox().len()).ok();
    let refused = u64::try_from(mailbox.outbox().iter().filter(|row| matches!(row.state, SendState::Refused { .. })).count()).unwrap_or(u64::MAX);
    let (drafts, drafts_total) = drafts(mailbox, hidden);
    let now_moment = civil.and_then(|civil| {
        let day = Day::new(civil.year(), civil.month(), civil.day())?;
        Some(Moment { day, minute: u16::from(civil.hour()).saturating_mul(60).saturating_add(u16::from(civil.minute())) })
    });
    let Agenda { lines: agenda, total: agenda_total, tomorrow, until } = now_moment.map(|now| agenda(mailbox, now)).unwrap_or_default();
    let by = mailbox.greet_by();
    let name = mailbox.accounts().iter().find_map(|account| by.word(&account.name).map(|word| kept(&word)));
    let date = civil.map_or_else(
        || tr!("Today").to_owned(),
        |civil| {
            tr!(
                "Today · {weekday} {day} {month}",
                weekday = crate::calendar::day::weekday_short(civil.weekday()),
                day = civil.day(),
                month = month_text(civil.month())
            )
        },
    );
    let counts = counts(read, screen, back, outbox, refused);
    let mut home = Home {
        kind: Kind::Brief,
        date,
        greeting: Greeting::at(civil.map_or(12, |civil| civil.hour())),
        name,
        lede: String::new(),
        failing: failing(mailbox, refused, away),
        counts,
        agenda,
        agenda_total,
        tomorrow,
        drafts,
        drafts_total,
        buttons: Buttons::ReadAndWrite,
        until,
    };
    let quiet = home.is_quiet();
    home.lede = kept(&lede(read, screen, back, home.agenda.first(), quiet));
    home.buttons = if read.unwrap_or(0) > 0 { Buttons::ReadAndWrite } else { Buttons::WriteAndCalendar };
    home
}

/// What is failing, read through `sync.status` (the last sync) and `account.list` (each account's connection and its last
/// failure), with the `refused` sends of the outbox: offline since the last sync when no account is signed in and one
/// that is not stopped for the owner is offline or backing off; the accounts that refused the sign-in; the sends the
/// server refused. `None` when none of it holds, also before a first sync: offline with nothing synced yet is the
/// status line's to say. A refused read leaves its part out. At most one account's name is kept.
///
/// An account connecting again is still away when it has a failure on record that does not stop it, or when it is
/// one of `away`, the accounts an earlier build saw offline or backing off: a reconnect attempt does not take the
/// banner down and put it back. `away` is left with the accounts seen away now. At startup nothing is on record and
/// nothing was synced, so a first connection says nothing.
fn failing(mailbox: &mut Mailbox, refused: u64, away: &mut Vec<u64>) -> Option<Failing> {
    let status = mailbox.run("sync.status", &json!({})).ok();
    let last = status.as_ref().and_then(|status| status.get("last_synced")).and_then(Value::as_str).and_then(crate::mailbox::parse_date);
    let listed = mailbox.run("account.list", &json!({})).ok();
    let rows = listed.as_ref().and_then(|listed| listed.get("accounts")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let (mut signed_in, mut gone, mut refusals, mut names) = (false, false, 0_u64, Vec::new());
    let mut seen = Vec::new();
    for row in rows {
        let state = row.get("connection").and_then(|connection| connection.get("state")).and_then(Value::as_str);
        let kind = row.get("failure").and_then(|failure| failure.get("kind")).and_then(Value::as_str);
        let known = kind.and_then(error_kind);
        let stopped = known.is_some_and(inboxcraft_model::ErrorKind::needs_the_owner);
        let id = row.get("id").and_then(Value::as_u64);
        let was = id.is_some_and(|id| away.contains(&id));
        let trying = state == Some("connecting") && (kind.is_some() || was);
        signed_in |= matches!(state, Some("online" | "watching"));
        if !stopped && (matches!(state, Some("offline" | "backing_off")) || trying) {
            gone = true;
            seen.extend(id);
        }
        if known == Some(inboxcraft_model::ErrorKind::LoginRefused) {
            refusals = refusals.saturating_add(1);
            if names.is_empty() {
                names.extend(name_of(row));
            }
        }
    }
    *away = seen;
    let since = last.filter(|_| !signed_in && gone).map(|at| mailbox.short_date(at));
    let to = mailbox
        .outbox()
        .iter()
        .find(|row| matches!(row.state, SendState::Refused { .. }))
        .and_then(|row| row.to.first())
        .map(|to| kept(to))
        .filter(|to| !to.trim().is_empty());
    let text = failing_text(since.as_deref(), &names, refusals, refused, to.as_deref());
    (!text.is_empty()).then_some(Failing { text, account: refusals > 0, outbox: refused > 0 })
}

/// The failing banner's words: "Offline since 14:10: what you see is what this computer holds." when `since` is given,
/// then the `refusals` accounts that refused the sign-in (named by the first of `names` when there is one) and the
/// `not_sent` sends the server refused (the first going `to` someone) in one sentence, and "Nothing is lost." after a
/// send not sent. Empty when there is nothing to say.
pub(crate) fn failing_text(since: Option<&str>, names: &[String], refusals: u64, not_sent: u64, to: Option<&str>) -> String {
    let n = crate::component::label::count;
    let mut sentences = Vec::new();
    if let Some(since) = since {
        sentences.push(tr!("Offline since {when}: what you see is what this computer holds.", when = since));
    }
    let refused = match (refusals, names.first()) {
        (0, _) => None,
        (1, Some(name)) => Some(tr!("{name} refused the sign-in", name = name)),
        (1, None) => Some(tr!("One account refused the sign-in").to_owned()),
        (many, _) => Some(tr!("{n} accounts refused the sign-in", n = n(many))),
    };
    let sent = |start: bool| match (not_sent, to) {
        (0, _) => None,
        (1, Some(who)) if start => Some(tr!("One message to {who} was not sent", who = who)),
        (1, Some(who)) => Some(tr!("one message to {who} was not sent", who = who)),
        (1, None) if start => Some(tr!("One message was not sent").to_owned()),
        (1, None) => Some(tr!("one message was not sent").to_owned()),
        (many, _) => Some(tr!("{n} messages were not sent", n = n(many))),
    };
    match (refused, sent(false)) {
        (Some(refused), Some(sent)) => sentences.push(tr!("{refused}, and {sent}.", refused = refused, sent = sent)),
        (Some(refused), None) => sentences.push(tr!("{refused}.", refused = refused)),
        (None, Some(_)) => sentences.extend(sent(true).map(|sent| tr!("{sent}.", sent = sent))),
        (None, None) => {}
    }
    if not_sent > 0 {
        sentences.push(tr!("Nothing is lost.").to_owned());
    }
    sentences.join(" ")
}

/// The most characters of an account's name in the failing banner's sentence: with the longest name, the sentence's
/// end ("refused the sign-in") still shows on the banner's two lines, at a 600 point brief and at 130 %.
pub(crate) const NAME_MOST: usize = 40;

/// An account's name as the failing banner's sentence holds it: its label, or its address where the label is blank;
/// plain ([`kept`]) and cut at [`NAME_MOST`] characters with one `…`. `None` when both are blank.
fn name_of(row: &Value) -> Option<String> {
    ["label", "address"].into_iter().filter_map(|key| row.get(key).and_then(Value::as_str)).map(named).find(|name| !name.is_empty())
}

/// `text` plain, trimmed and at most [`NAME_MOST`] characters, cut between grapheme clusters with one `…`.
pub(crate) fn named(text: &str) -> String {
    let plain = kept(text);
    let (name, cut) = crate::component::capped(plain.trim(), NAME_MOST);
    if cut { format!("{}…", name.trim_end()) } else { name.to_owned() }
}

/// The error kind `account.list` names (`login_refused`), by the engine's own names of every kind: a name it does not
/// give is no kind.
fn error_kind(name: &str) -> Option<inboxcraft_model::ErrorKind> {
    inboxcraft_model::ErrorKind::ALL.into_iter().find(|kind| inboxcraft_engine::error_kind_name(*kind) == name)
}

/// How many snoozed messages come back today, from `mail.list` of the snoozed mail (at most the engine's page of
/// it); `None` when the read was refused.
fn back_today(mailbox: &mut Mailbox, today: Option<Day>, offset: inboxcraft_time::Offset) -> Option<u64> {
    let answer = mailbox.run("mail.list", &json!({ "role": "snoozed", "limit": inboxcraft_engine::MAX_LIST })).ok()?;
    let rows = answer.get("messages").and_then(Value::as_array)?;
    let today = today?;
    let back = rows
        .iter()
        .filter_map(|row| row.get("snoozed_until").and_then(Value::as_str).and_then(crate::mailbox::parse_date))
        .filter(|until| Day::of(*until, offset) == Some(today))
        .count();
    u64::try_from(back).ok()
}

/// What Home asks `compose.list` for: the [`DRAFTS`] it shows; the "all drafts (N)" line reads the answer's `total`.
fn drafts_asked() -> Value {
    json!({ "limit": DRAFTS })
}

/// The newest [`DRAFTS`] drafts and how many there are, from `compose.list`; none when the read was refused. The draft
/// `hidden` is left out: one more is asked for to stand in its place, and the count drops by one when it was among
/// those answered (a hidden draft further down, newer ones written since, stays counted).
fn drafts(mailbox: &mut Mailbox, hidden: Option<u64>) -> (Vec<DraftLine>, u64) {
    let asked = if hidden.is_some() { json!({ "limit": DRAFTS.saturating_add(1) }) } else { drafts_asked() };
    let Ok(answer) = mailbox.run("compose.list", &asked) else { return (Vec::new(), 0) };
    let rows = answer.get("drafts").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let total = answer.get("total").and_then(Value::as_u64).unwrap_or(0).max(u64::try_from(rows.len()).unwrap_or(u64::MAX));
    let is_hidden = |row: &&Value| hidden.is_some() && row.get("id").and_then(Value::as_u64) == hidden;
    let total = if rows.iter().any(|row| is_hidden(&row)) { total.saturating_sub(1) } else { total };
    let lines = rows.iter().filter(|row| !is_hidden(row)).take(DRAFTS).filter_map(|row| draft_line(mailbox, row)).collect();
    (lines, total)
}

/// A draft of `compose.list` as Home shows it.
fn draft_line(mailbox: &Mailbox, row: &Value) -> Option<DraftLine> {
    let id = row.get("id").and_then(Value::as_u64)?;
    let subject = row.get("subject").and_then(Value::as_str).map(kept).filter(|subject| !subject.trim().is_empty());
    let first = row.get("to").and_then(Value::as_array).and_then(|to| to.first()).and_then(|to| to.get("text")).and_then(Value::as_str);
    let to = match first.map(kept).filter(|to| !to.trim().is_empty()) {
        Some(to) => tr!("to {who}", who = to),
        None => tr!("no recipient yet").to_owned(),
    };
    let saved: Option<Timestamp> = row.get("saved").and_then(Value::as_str).and_then(crate::mailbox::parse_date);
    let when = saved.map_or_else(String::new, |saved| tr!("edited {when}", when = mailbox.short_date(saved)));
    Some(DraftLine { id, subject, to, when })
}

/// The rest of today, tomorrow's first event and when the first event left ends, read with one `event.list` of the two days Home needs (today and
/// tomorrow on the owner's clock, a day either side because the engine counts from UTC midnights) and one
/// `calendar.list` for the accounts' colours; nothing when a read was refused (Home puts no error in the window). The
/// first [`AGENDA`] lines, how many in all, and tomorrow's first timed event when nothing is left today.
///
/// An event belongs to the day it starts on: a timed event of today that has not ended is the rest of today, also when
/// it runs past midnight; tomorrow's first is the first timed event that starts tomorrow. One that started before
/// today is neither, since its time would read as a time of today. All-day events are not lines.
fn agenda(mailbox: &mut Mailbox, now: Moment) -> Agenda {
    let (today, next) = (now.day, now.day.plus(1));
    let params = json!({ "from": today.plus(-1).iso_midnight(), "to": today.plus(3).iso_midnight() });
    let Ok(answer) = mailbox.run("event.list", &params) else { return Agenda::default() };
    let rows = answer.get("events").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let timed: Vec<EventRow> =
        rows.iter().filter_map(EventRow::read).filter(|event| !event.all_day && event.start.day >= today && event.start.day <= next).collect();
    let mut left: Vec<&EventRow> = timed.iter().filter(|event| event.start.day == today && event.end > now).collect();
    left.sort_by_key(|event| (event.start, event.id));
    let total = u64::try_from(left.len()).unwrap_or(u64::MAX);
    let until = left.iter().map(|event| event.end).min();
    let places = account_places(mailbox);
    let lines = left
        .iter()
        .take(AGENDA)
        .map(|event| {
            let minutes = (i64::from(event.end.minute) - i64::from(event.start.minute))
                .saturating_add(event.end.day.since(event.start.day).saturating_mul(1_440));
            let detail = [kept(&event.location), length(minutes.max(0))].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ");
            let account = places.iter().find(|(calendar, _)| *calendar == event.calendar).map_or(0, |(_, place)| *place);
            AgendaLine { time: event.start.clock(), title: kept(&event.summary), detail, account }
        })
        .collect();
    let tomorrow = (total == 0)
        .then(|| {
            timed
                .iter()
                .filter(|event| event.start.day == next)
                .min_by_key(|event| (event.start, event.id))
                .map(|event| (event.start.clock(), kept(&event.summary)))
        })
        .flatten();
    Agenda { lines, total, tomorrow, until }
}

/// The first of today's events left, as the status line names it: its start (`15:00`), its title, and the minutes
/// until it starts (none or less once it has). Read with [`agenda`]'s commands: call it when the mailbox changed.
pub(crate) fn next_event(mailbox: &mut Mailbox) -> Option<(String, String, i64)> {
    let civil = mailbox.now().to_civil(mailbox.offset())?;
    let day = Day::new(civil.year(), civil.month(), civil.day())?;
    let minute = u16::from(civil.hour()).saturating_mul(60).saturating_add(u16::from(civil.minute()));
    let line = agenda(mailbox, Moment { day, minute }).lines.into_iter().next()?;
    let (hours, minutes) = line.time.split_once(':')?;
    let start = hours.parse::<i64>().ok()?.saturating_mul(60).saturating_add(minutes.parse::<i64>().ok()?);
    Some((line.time, line.title, start.saturating_sub(i64::from(minute))))
}

/// The rest of today as [`agenda`] reads it.
#[derive(Default)]
struct Agenda {
    /// The first [`AGENDA`] lines.
    lines: Vec<AgendaLine>,
    /// How many events are left today.
    total: u64,
    /// Tomorrow's first timed event, when nothing is left today.
    tomorrow: Option<(String, String)>,
    /// When the first of today's events left ends.
    until: Option<Moment>,
}

/// Each calendar's account's place among the accounts (which picks an agenda bar's colour), from `calendar.list`.
fn account_places(mailbox: &mut Mailbox) -> Vec<(u64, u64)> {
    let Ok(answer) = mailbox.run("calendar.list", &json!({})) else { return Vec::new() };
    let rows = answer.get("calendars").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    rows.iter()
        .filter_map(|row| {
            let calendar = row.get("id").and_then(Value::as_u64)?;
            let account = row.get("account").and_then(Value::as_u64)?;
            let place = mailbox.accounts().iter().find(|held| held.id == account).map_or(0, |held| u64::try_from(held.index).unwrap_or(0));
            Some((calendar, place))
        })
        .collect()
}

/// What a built Home was built from: the mailbox's [`Mailbox::revision`] and [`Mailbox::writes`], the day and the
/// hour on the owner's clock (the greeting, the date, the rest of today), and the word of the name it greets by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    revision: u64,
    writes: u64,
    hour: Option<(Day, u8)>,
    hidden: Option<u64>,
    greet_by: crate::mailbox::GreetBy,
}

/// Home as the window keeps it between frames: built while Home shows, and again only when the mailbox changed (a load,
/// a draft kept, an event saved), the hour or the day turned, or the first of today's events left ended. While another
/// place shows nothing is built; the first frame Home shows again builds it once ([`Kept::forget`]).
#[derive(Debug, Default)]
pub struct Kept {
    home: Option<Home>,
    stamp: Option<Stamp>,
    builds: u64,
    /// The accounts the last build saw offline or backing off (see [`failing`]).
    away: Vec<u64>,
}

impl Kept {
    /// Home for this frame, built again only when what it was built from changed; the draft `hidden` (a discard waiting
    /// behind its toast) left out of it, and back in when it is no longer hidden.
    pub fn current(&mut self, mailbox: &mut Mailbox, hidden: Option<u64>) -> &Home {
        let (now, offset) = (mailbox.now(), mailbox.offset());
        let civil = now.to_civil(offset);
        let hour = civil.and_then(|civil| Some((Day::new(civil.year(), civil.month(), civil.day())?, civil.hour())));
        let stamp = Stamp { revision: mailbox.revision(), writes: mailbox.writes(), hour, hidden, greet_by: mailbox.greet_by() };
        let moment = hour
            .zip(civil)
            .map(|((day, _), civil)| Moment { day, minute: u16::from(civil.hour()).saturating_mul(60).saturating_add(u16::from(civil.minute())) });
        let ended = |home: &Home| home.until.zip(moment).is_some_and(|(until, now)| until <= now);
        let stale = self.stamp != Some(stamp) || self.home.as_ref().is_none_or(ended);
        if stale {
            let home = build_seeing(mailbox, hidden, &mut self.away);
            self.builds = self.builds.saturating_add(1);
            // The build read the mailbox: the stamp is taken after it, so its own reads count as seen.
            self.stamp = Some(Stamp { revision: mailbox.revision(), writes: mailbox.writes(), hour, hidden, greet_by: mailbox.greet_by() });
            self.home = Some(home);
        }
        self.home.get_or_insert_with(Home::first_run)
    }

    /// Home is not shown: the next frame it shows builds it again.
    pub fn forget(&mut self) {
        self.stamp = None;
    }

    /// How many times Home was built (for the tests).
    #[must_use]
    pub fn builds(&self) -> u64 {
        self.builds
    }

    /// How long until Home must be built again by the clock alone (the hour turns, an event left today ends), from
    /// `mailbox`'s now; `None` before the first build or when the clock cannot be read.
    #[must_use]
    pub fn due_in(&self, mailbox: &Mailbox) -> Option<std::time::Duration> {
        let civil = mailbox.now().to_civil(mailbox.offset())?;
        let into_hour = u64::from(civil.minute()) * 60 + u64::from(civil.second());
        let to_hour = 3_600_u64.saturating_sub(into_hour).max(1);
        let day = Day::new(civil.year(), civil.month(), civil.day())?;
        let minute = i64::from(civil.hour()) * 60 + i64::from(civil.minute());
        let to_end = self.home.as_ref().and_then(|home| home.until).map(|until| {
            let minutes = until.day.since(day).saturating_mul(1_440).saturating_add(i64::from(until.minute)).saturating_sub(minute);
            u64::try_from(minutes.saturating_mul(60).saturating_sub(i64::from(civil.second()))).unwrap_or(0).max(1)
        });
        Some(std::time::Duration::from_secs(to_end.map_or(to_hour, |end| end.min(to_hour))))
    }
}

/// Home as the design file draws it in `state` (`today`, `nothing-to-do`, `overflow-counts`, `first-run`,
/// `everything-failing`, and for the drafts' block alone `drafts` and `drafts-empty`), with the file's invented people:
/// the gallery's page.
#[must_use]
pub fn sample(state: &str) -> Option<Home> {
    let line =
        |time: &str, title: &str, detail: &str, account: u64| AgendaLine { time: time.into(), title: title.into(), detail: detail.into(), account };
    let draft =
        |id: u64, subject: Option<&str>, to: &str, when: &str| DraftLine { id, subject: subject.map(Into::into), to: to.into(), when: when.into() };
    let drafts = vec![
        draft(1, Some("Price list for the winter workshops"), "to Sunniva Aalto", "edited 12:10"),
        draft(2, Some("Re: Kiln service: two dates to choose from"), "to Halvard Engen", "edited yesterday"),
    ];
    let today = Home {
        kind: Kind::Brief,
        date: "Today · Mon 16 Nov".into(),
        greeting: Greeting::Afternoon,
        name: Some("Odile".into()),
        lede: "Seven to read, three people waiting at the door, and a design review at 15:00.".into(),
        failing: None,
        counts: counts(Some(7), Some(3), Some(2), Some(1), 0),
        agenda: vec![line("15:00", "Design review", "Room 3 · 1 h · in 58 min", 1), line("18:30", "Dinner with Marit", "Harbour kitchen", 0)],
        agenda_total: 2,
        tomorrow: None,
        drafts,
        drafts_total: 2,
        buttons: Buttons::ReadAndWrite,
        until: None,
    };
    Some(match state {
        "today" => today,
        "nothing-to-do" => Home {
            greeting: Greeting::Evening,
            lede: "Nothing waits for you. The inbox is read, nobody is at the door, and the day's events are over.".into(),
            counts: counts(Some(0), Some(0), Some(0), Some(0), 0),
            agenda: Vec::new(),
            agenda_total: 0,
            tomorrow: Some(("08:30".into(), "Kiln loading".into())),
            drafts: Vec::new(),
            drafts_total: 0,
            buttons: Buttons::WriteAndCalendar,
            ..today
        },
        "overflow-counts" => Home {
            greeting: Greeting::Morning,
            lede: "More than nine thousand to read, 1 204 people at the door, and twelve events today.".into(),
            counts: counts(Some(12_000), Some(1_204), Some(10_000), Some(37), 0),
            agenda: vec![
                line("09:15", "Call Ines", "30 min · in 3 min", 0),
                line("10:00", "Workshop prep", "Room 3 · 1 h", 1),
                line("11:30", "Supplier visit", "Yard", 1),
            ],
            agenda_total: 11,
            drafts_total: 23,
            ..today
        },
        "first-run" => Home::first_run(),
        "everything-failing" => Home {
            failing: Some(Failing {
                text: crate::home::failing_text(Some("14:10"), &["Studio".to_owned()], 1, 1, Some("Odette Marchand")),
                account: true,
                outbox: true,
            }),
            counts: counts(Some(7), Some(3), Some(2), Some(1), 1),
            ..today
        },
        "drafts" => Home {
            drafts: vec![
                draft(1, Some("Price list for the winter workshops"), "to Sunniva Aalto", "edited 12:10"),
                draft(2, Some("Re: Kiln service: two dates to choose from"), "to Halvard Engen", "edited yesterday"),
                draft(3, None, "to Marit Solheim", "edited Fri"),
            ],
            drafts_total: 5,
            ..today
        },
        "drafts-empty" => Home { drafts: Vec::new(), drafts_total: 0, ..today },
        _ => return None,
    })
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
