// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the Scheduled page of the settings shows and holds: everything that waits for its time as `schedule.list`
//! answered it (a send, a snooze that comes back, a follow-up reminder, an event's alert), the next first, and the
//! entry whose cancel waits for the owner's confirmation.
//!
//! An entry's subject and who it names come from mail and calendars, which are outside data: the engine answers them
//! as one short clean line, and a row still makes them plain and caps them where it is built, so the page lays out at
//! most a line's worth of each whatever answered.

use inboxcraft_time::{Offset, Timestamp};
use serde_json::Value;

use crate::component::{capped, plain};
use crate::tr;

/// The most characters of a row's subject and of its second text a row keeps: more than a line shows.
const LINE: usize = 240;

/// The most entries the page keeps.
pub const MOST: usize = 2_000;

/// What an entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A message sent at its time.
    Send,
    /// A snoozed message that comes back.
    Snooze,
    /// A follow-up reminder if no reply came.
    Reminder,
    /// A calendar event's alert.
    Alert,
}

impl Kind {
    /// The kind of `what` in `schedule.list`'s answer.
    fn named(what: &str) -> Option<Self> {
        match what {
            "send" => Some(Self::Send),
            "unsnooze" => Some(Self::Snooze),
            "follow_up" => Some(Self::Reminder),
            "alert" => Some(Self::Alert),
            _ => None,
        }
    }

    /// The word of its tag.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Send => tr!("send"),
            Self::Snooze => tr!("snooze"),
            Self::Reminder => tr!("reminder"),
            Self::Alert => tr!("alert"),
        }
    }
}

/// An entry as a row of the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledRow {
    /// The schedule entry's id, to cancel it by.
    pub id: u64,
    /// When it is due.
    pub due: Timestamp,
    /// What it does.
    pub kind: Kind,
    /// The account it belongs to, if the store still holds what it names.
    pub account: Option<u64>,
    /// Its subject (an alert's event title): plain and capped; empty when it has none.
    pub subject: String,
    /// Who it names (a send's recipients, a snooze's sender, a reminder's recipient), plain and capped.
    pub who: String,
    /// For an alert, how many minutes before the event it rings.
    pub before: Option<u64>,
    /// For a snooze or a reminder, the message it names: a snooze's cancel brings that message back.
    pub message: Option<u64>,
}

/// `text` plain, on one line, at most [`LINE`] characters.
fn line(text: &str) -> String {
    let (kept, _) = capped(text, LINE);
    plain(kept).split_whitespace().collect::<Vec<_>>().join(" ")
}

impl ScheduledRow {
    /// The row of one entry of `schedule.list`'s answer; `None` for one without an id, a time or a known kind.
    #[must_use]
    pub fn read(value: &Value) -> Option<Self> {
        let id = value.get("id").and_then(Value::as_u64)?;
        let due = value.get("due").and_then(Value::as_str).and_then(crate::mailbox::parse_date)?;
        let kind = value.get("what").and_then(Value::as_str).and_then(Kind::named)?;
        let text = |key: &str| line(value.get(key).and_then(Value::as_str).unwrap_or_default());
        Some(Self {
            id,
            due,
            kind,
            account: value.get("account").and_then(Value::as_u64),
            subject: text("subject"),
            who: text("who"),
            before: value.get("before_minutes").and_then(Value::as_u64),
            message: value.get("message").and_then(Value::as_u64),
        })
    }

    /// The row's subject, or what stands for it when it has none.
    #[must_use]
    pub fn title(&self) -> String {
        if self.subject.is_empty() { tr!("(no subject)").to_owned() } else { self.subject.clone() }
    }

    /// The row's second text: whom a send goes to, whom a snoozed message is from, whom a reminder waits on, how long
    /// before its event an alert rings.
    #[must_use]
    pub fn note(&self) -> String {
        match self.kind {
            Kind::Send if self.who.is_empty() => tr!("no recipient yet").to_owned(),
            Kind::Send => tr!("to {who}", who = self.who),
            Kind::Snooze => self.who.clone(),
            Kind::Reminder if self.who.is_empty() => tr!("if no reply").to_owned(),
            Kind::Reminder => tr!("if no reply · to {who}", who = self.who),
            Kind::Alert => match self.before {
                Some(0) => tr!("at its start").to_owned(),
                Some(minutes) if minutes % (24 * 60) == 0 => tr!("{n} d before", n = minutes / (24 * 60)),
                Some(minutes) if minutes % 60 == 0 => tr!("{n} h before", n = minutes / 60),
                Some(minutes) => tr!("{n} min before", n = minutes),
                None => String::new(),
            },
        }
    }
}

#[cfg(test)]
thread_local! {
    /// How many times [`when`] ran on this thread: a test's count of the rows formatted.
    pub(crate) static WHENS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `due` as a row shows it at `now` in `offset`: "Today 14:50", the weekday within the coming week, the day and month
/// further on; a time gone by as the mail list dates it.
#[must_use]
pub fn when(due: Timestamp, now: Timestamp, offset: Offset) -> String {
    #[cfg(test)]
    WHENS.with(|whens| whens.set(whens.get().saturating_add(1)));
    let (Some(day), Some(today)) = (due.to_civil(offset), now.to_civil(offset)) else { return String::new() };
    let ahead = crate::mailbox::day_number(day).zip(crate::mailbox::day_number(today)).map_or(i64::MIN, |(day, today)| day.saturating_sub(today));
    let time = format!("{:02}:{:02}", day.hour(), day.minute());
    match ahead {
        0 => tr!("Today {time}", time = time),
        1..=6 => format!("{} {time}", crate::calendar::day::weekday_short(day.weekday())),
        7.. => format!("{} {} {time}", day.day(), crate::mailbox::month_text(day.month())),
        _ => crate::mailbox::when(due, now, offset),
    }
}

/// Where the keyboard goes when the page is drawn next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hand {
    /// The confirmation's "Keep it".
    Keep,
    /// The `cancel` of the row of this entry.
    Cancel(u64),
    /// The link back to Sending.
    Back,
}

/// The Scheduled page's state.
#[derive(Debug, Default)]
pub struct Scheduled {
    /// Where the keyboard goes when the page is drawn next.
    hand: Option<Hand>,
    rows: Vec<ScheduledRow>,
    /// The entry whose cancel waits for the owner's confirmation.
    confirm: Option<u64>,
    /// Why the last cancel or read was not done.
    note: Option<String>,
}

impl Scheduled {
    /// The entries, the next first, as `schedule.list` gave them.
    #[must_use]
    pub fn rows(&self) -> &[ScheduledRow] {
        &self.rows
    }

    /// Takes `schedule.list`'s answer: at most [`MOST`] rows, the confirmation dropped when its entry went.
    pub fn read(&mut self, answer: &Value) {
        let listed = answer.get("entries").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        self.rows = listed.iter().filter_map(ScheduledRow::read).take(MOST).collect();
        if let Some(id) = self.confirm
            && !self.rows.iter().any(|row| row.id == id)
        {
            self.confirm = None;
        }
    }

    /// The entry whose cancel waits for the owner's confirmation.
    #[must_use]
    pub fn confirming(&self) -> Option<u64> {
        self.confirm
    }

    /// Asks the owner to confirm the cancel of entry `id`.
    pub fn ask_cancel(&mut self, id: u64) {
        self.confirm = Some(id);
        self.note = None;
    }

    /// The cancel was not confirmed.
    pub fn keep(&mut self) {
        self.confirm = None;
    }

    /// Why the last cancel or read was not done.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Says why the last cancel or read was not done; `None` when it was.
    pub fn set_note(&mut self, note: Option<String>) {
        self.note = note;
    }

    /// Asks that the keyboard go to `hand` when the page is drawn next.
    pub fn hand_to(&mut self, hand: Hand) {
        self.hand = Some(hand);
    }

    /// Where the keyboard was asked to go, taken.
    pub fn take_hand(&mut self) -> Option<Hand> {
        self.hand.take()
    }

    /// Another place or page is shown: the confirmation closes.
    pub fn leave(&mut self) {
        self.confirm = None;
        self.hand = None;
    }
}
