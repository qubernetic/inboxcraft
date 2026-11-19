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

use crate::calendar::{Calendar, Day};
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
    /// The owner's first name, the headline's second line; `None` when no account says one.
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

    /// Whether nothing waits: every count 0 or unread, no event left today and no draft.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.counts.iter().all(|count| count.count.unwrap_or(0) == 0) && self.agenda_total == 0 && self.drafts_total == 0
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

/// The sentence under the headline, from the counts and the next event.
fn lede(read: u64, screen: u64, back: u64, next: Option<&AgendaLine>, quiet: bool) -> String {
    if quiet {
        return tr!("Nothing waits for you. The inbox is read, nobody is at the door, and the day's events are over.").to_owned();
    }
    let n = crate::component::label::count;
    let mut parts = Vec::new();
    if read > 0 {
        parts.push(tr!("{n} to read", n = n(read)));
    }
    if screen > 0 {
        parts.push(tr!("{n} waiting at the door", n = n(screen)));
    }
    if back > 0 {
        parts.push(tr!("{n} back today", n = n(back)));
    }
    if let Some(next) = next {
        parts.push(tr!("{title} at {time}", title = next.title, time = next.time));
    }
    match parts.split_last() {
        None => tr!("Nothing waits for you in the inbox.").to_owned(),
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
/// today through `calendar.list` and `event.list`. A refused read leaves its part empty (a count `None`), never made
/// up. Runs commands: call it when the mailbox changed, not every frame.
pub fn build(mailbox: &mut Mailbox) -> Home {
    if mailbox.accounts().is_empty() {
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
    let (drafts, drafts_total) = drafts(mailbox);
    let mut calendar = Calendar::default();
    calendar.refresh(mailbox);
    let (agenda, agenda_total, tomorrow) = agenda(&calendar);
    let name = mailbox.accounts().iter().find_map(|account| account.name.split_whitespace().next().map(kept));
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
        failing: (refused > 0).then(|| Failing {
            text: tr!("The server refused {n} of your sends; they are kept in the outbox.", n = crate::component::label::count(refused)),
            account: false,
            outbox: true,
        }),
        counts,
        agenda,
        agenda_total,
        tomorrow,
        drafts,
        drafts_total,
        buttons: Buttons::ReadAndWrite,
    };
    let quiet = home.is_quiet();
    home.lede = kept(&lede(read.unwrap_or(0), screen.unwrap_or(0), back.unwrap_or(0), home.agenda.first(), quiet));
    home.buttons = if read.unwrap_or(0) > 0 { Buttons::ReadAndWrite } else { Buttons::WriteAndCalendar };
    home
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

/// The newest [`DRAFTS`] drafts and how many there are, from `compose.list`; none when the read was refused.
fn drafts(mailbox: &mut Mailbox) -> (Vec<DraftLine>, u64) {
    let Ok(answer) = mailbox.run("compose.list", &json!({})) else { return (Vec::new(), 0) };
    let rows = answer.get("drafts").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let total = u64::try_from(rows.len()).unwrap_or(u64::MAX);
    let lines = rows.iter().take(DRAFTS).filter_map(|row| draft_line(mailbox, row)).collect();
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

/// The rest of today in `calendar` (timed events of today that have not ended, by their start): the first [`AGENDA`]
/// lines, how many in all, and tomorrow's first timed event when nothing is left today.
fn agenda(calendar: &Calendar) -> (Vec<AgendaLine>, u64, Option<(String, String)>) {
    let today = calendar.today();
    let now = calendar.now();
    let mut left: Vec<_> =
        calendar.events().filter(|event| !event.is_block() && event.first == today && now.is_none_or(|now| event.end > now)).collect();
    left.sort_by_key(|event| (event.start, event.id));
    let total = u64::try_from(left.len()).unwrap_or(u64::MAX);
    let lines = left
        .iter()
        .take(AGENDA)
        .map(|event| {
            let minutes = i64::from(event.end.minute) - i64::from(event.start.minute) + event.end.day.since(event.start.day).saturating_mul(1_440);
            let detail = [kept(&event.location), length(minutes.max(0))].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ");
            AgendaLine { time: event.start.clock(), title: kept(&event.summary), detail, account: calendar.account_place(event.calendar) }
        })
        .collect();
    let tomorrow = (total == 0)
        .then(|| {
            let next = today.plus(1);
            calendar
                .events()
                .filter(|event| !event.is_block() && event.first == next)
                .min_by_key(|event| (event.start, event.id))
                .map(|event| (event.start.clock(), kept(&event.summary)))
        })
        .flatten();
    (lines, total, tomorrow)
}

/// Home as the design file draws it in `state` (`today`, `nothing-to-do`, `overflow-counts`, `first-run`,
/// `everything-failing`), with the file's invented people: the gallery's page.
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
                text: "Offline since 14:10: what you see is what this computer holds. Studio refused the sign-in, and one message was not sent."
                    .into(),
                account: true,
                outbox: true,
            }),
            counts: counts(Some(7), Some(3), Some(2), Some(1), 1),
            ..today
        },
        _ => return None,
    })
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
