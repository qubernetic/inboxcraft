// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An invitation in a message, as its card in the reader shows it: what `invite.show` answers, read into plain fields,
//! and the lines the card says. The times are the engine's, on the computer's wall clock.
//!
//! An invitation's text is the organiser's data: it is shown, never acted on. The card's buttons run the owner's
//! commands, `invite.answer` and `invite.remove`.

use serde_json::Value;

use crate::calendar::day::{dated, month_name};
use crate::calendar::{Day, Moment};
use crate::mailbox::shown;
use crate::tr;

/// What an invitation does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// An invitation, new or changed.
    Request,
    /// The event is off.
    Cancel,
    /// A guest's answer.
    Reply,
    /// Anything else: shown, nothing to answer.
    Other,
}

/// An answer to an invitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rsvp {
    /// Going.
    Going,
    /// Maybe.
    Maybe,
    /// Not going.
    No,
}

impl Rsvp {
    /// Every answer, in the order the card offers them.
    pub const ALL: [Self; 3] = [Self::Going, Self::Maybe, Self::No];

    /// The answer as `invite.answer` names it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Going => "accepted",
            Self::Maybe => "tentative",
            Self::No => "declined",
        }
    }

    /// The answer `invite.show` names `id`.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|answer| answer.id() == id)
    }

    /// Its button.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Going => tr!("Going"),
            Self::Maybe => tr!("Maybe"),
            Self::No => tr!("No"),
        }
    }
}

/// One version of the event: its title, place and times.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Version {
    /// The title.
    pub summary: String,
    /// The place.
    pub location: String,
    /// When it starts, on the computer's clock.
    pub start: Option<Moment>,
    /// When it ends.
    pub end: Option<Moment>,
    /// Whole days.
    pub all_day: bool,
    /// The day the message with this version came, on the computer's clock.
    pub date: Option<Day>,
}

impl Version {
    fn read(value: &Value) -> Self {
        let text = |name: &str| shown(value.get(name).and_then(Value::as_str).unwrap_or_default());
        let moment = |name: &str| value.get(name).and_then(Value::as_str).and_then(Moment::parse);
        Self {
            summary: text("summary"),
            location: text("location"),
            start: moment("local_start"),
            end: moment("local_end"),
            all_day: value.get("all_day").and_then(Value::as_bool).unwrap_or(false),
            date: value.get("date").and_then(Value::as_str).and_then(Moment::parse).map(|moment| moment.day),
        }
    }

    /// The last day of a whole-day event: the day before its end.
    fn last_day(&self) -> Option<Day> {
        let (start, end) = (self.start?, self.end?);
        Some(if end.day > start.day { end.day.plus(-1) } else { start.day })
    }

    /// When it is: `Saturday 7 November · 12:30 – 13:30`, a whole day's date, or the days of a longer one.
    #[must_use]
    pub fn when(&self) -> String {
        let Some(start) = self.start else { return String::new() };
        if self.all_day {
            return match self.last_day().filter(|last| *last != start.day) {
                Some(last) => tr!("{first} – {last}", first = dated(start.day), last = dated(last)),
                None => dated(start.day),
            };
        }
        match self.end {
            Some(end) if end.day == start.day => tr!("{day} · {start} – {end}", day = dated(start.day), start = start.clock(), end = end.clock()),
            Some(end) => tr!("{day} {start} – {last} {end}", day = dated(start.day), start = start.clock(), last = dated(end.day), end = end.clock()),
            None => tr!("{day} · {start}", day = dated(start.day), start = start.clock()),
        }
    }

    /// The time it starts, as a change shows it: the clock, with the day when `other` starts on another day.
    fn start_against(&self, other: &Self) -> String {
        match (self.start, other.start) {
            (Some(start), Some(theirs)) if start.day == theirs.day && !self.all_day => start.clock(),
            (Some(start), _) if self.all_day => dated(start.day),
            (Some(start), _) => tr!("{day} {time}", day = dated(start.day), time = start.clock()),
            (None, _) => String::new(),
        }
    }
}

/// A guest's answer, on a card of a reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// The guest's name, or their address.
    pub name: String,
    /// What they answered, if it is one of the three.
    pub answer: Option<Rsvp>,
}

/// An invitation, as its card shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    /// The message.
    pub message: u64,
    /// What it does.
    pub method: Method,
    /// The event as it says.
    pub event: Version,
    /// The organiser's name, or their address.
    pub organizer: Option<String>,
    /// The organiser's address.
    pub organizer_address: Option<String>,
    /// The owner's answer to it.
    pub answer: Option<Rsvp>,
    /// The owner's last answer to an earlier version.
    pub earlier_answer: Option<Rsvp>,
    /// The version before it.
    pub earlier: Option<Version>,
    /// What changed since: `time`, `repeat`, `place`, `title`.
    pub changes: Vec<String>,
    /// A later version came.
    pub outdated: bool,
    /// The event in the owner's calendar.
    pub calendar_event: Option<u64>,
    /// It can be answered.
    pub can_answer: bool,
    /// Why not, when it cannot.
    pub why_not: Option<String>,
    /// The cancelled event can be taken out of the calendar.
    pub can_remove: bool,
    /// The guests' answers, on a reply.
    pub replies: Vec<Reply>,
}

impl Invite {
    /// The card `invite.show` answers for message `message`; `None` for a message without an invitation.
    #[must_use]
    pub fn read(message: u64, answer: &Value) -> Option<Self> {
        let invite = answer.get("invite").filter(|invite| invite.is_object())?;
        let method = match invite.get("method").and_then(Value::as_str) {
            Some("request") => Method::Request,
            Some("cancel") => Method::Cancel,
            Some("reply") => Method::Reply,
            _ => Method::Other,
        };
        let rsvp = |name: &str| invite.get(name).and_then(Value::as_str).and_then(Rsvp::named);
        let flag = |name: &str| invite.get(name).and_then(Value::as_bool).unwrap_or(false);
        let organizer = invite.get("organizer").filter(|organizer| organizer.is_object());
        let address = organizer.and_then(|organizer| organizer.get("address")).and_then(Value::as_str).map(shown);
        let name = organizer.and_then(|organizer| organizer.get("name")).and_then(Value::as_str).map(shown).filter(|name| !name.is_empty());
        let replies = invite
            .get("replies")
            .and_then(Value::as_array)
            .map(|replies| {
                replies
                    .iter()
                    .map(|reply| {
                        let address = reply.get("address").and_then(Value::as_str).unwrap_or_default();
                        let name = reply.get("name").and_then(Value::as_str).filter(|name| !name.is_empty()).unwrap_or(address);
                        let answer = reply.get("status").and_then(Value::as_str).and_then(Rsvp::named);
                        Reply { name: shown(name), answer }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            message,
            method,
            event: Version::read(invite),
            organizer: name.or_else(|| address.clone()),
            organizer_address: address,
            answer: rsvp("answer"),
            earlier_answer: rsvp("earlier_answer"),
            earlier: invite.get("earlier").filter(|earlier| earlier.is_object()).map(Version::read),
            changes: invite
                .get("changes")
                .and_then(Value::as_array)
                .map(|changes| changes.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default(),
            outdated: flag("outdated"),
            calendar_event: invite.get("calendar_event").and_then(Value::as_u64),
            can_answer: flag("can_answer"),
            why_not: invite.get("why_not").and_then(Value::as_str).map(shown),
            can_remove: flag("can_remove"),
            replies,
        })
    }

    /// The tile's month, short and in capitals: `NOV`.
    #[must_use]
    pub fn month(&self) -> String {
        self.event.start.map(|start| month_name(start.day.month()).chars().take(3).collect::<String>().to_uppercase()).unwrap_or_default()
    }

    /// The tile's day of the month.
    #[must_use]
    pub fn day(&self) -> String {
        self.event.start.map(|start| start.day.date().to_string()).unwrap_or_default()
    }

    /// The line under the title: when, and where.
    #[must_use]
    pub fn line(&self) -> String {
        let when = self.event.when();
        match (when.is_empty(), self.event.location.is_empty()) {
            (false, false) => tr!("{when} · {place}", when = when, place = self.event.location),
            (false, true) => when,
            (true, _) => self.event.location.clone(),
        }
    }

    /// What changed since the version before: `Changed: time 11:00 → 12:30, place`.
    #[must_use]
    pub fn changed(&self) -> Option<String> {
        if self.changes.is_empty() {
            return None;
        }
        let earlier = self.earlier.as_ref();
        let parts: Vec<String> = self
            .changes
            .iter()
            .map(|change| match (change.as_str(), earlier) {
                ("time", Some(was)) => {
                    tr!("time {was} → {now}", was = was.start_against(&self.event), now = self.event.start_against(was))
                }
                ("time", None) => tr!("time").to_owned(),
                ("place", Some(was)) if !was.location.is_empty() && !self.event.location.is_empty() => {
                    tr!("place {was} → {now}", was = was.location, now = self.event.location)
                }
                ("place", _) => tr!("place").to_owned(),
                ("title", _) => tr!("title").to_owned(),
                ("repeat", _) => tr!("repeat").to_owned(),
                (other, _) => shown(other),
            })
            .collect();
        Some(tr!("Changed: {what}", what = parts.join(", ")))
    }

    /// The line about the version before, under the card: `Out of date: the invitation from Monday showed 11:00. You
    /// answered Going.`
    #[must_use]
    pub fn earlier_line(&self) -> Option<String> {
        let was = self.earlier.as_ref()?;
        let from = was.date.map(|day| crate::mailbox::weekday_text(day.weekday()));
        let showed = was.start_against(&self.event);
        let line = match from {
            Some(from) => tr!("Out of date: the invitation from {from} showed {showed}.", from = from, showed = showed),
            None => tr!("Out of date: the invitation before showed {showed}.", showed = showed),
        };
        Some(match self.earlier_answer {
            Some(answer) => tr!("{line} You answered {answer}.", line = line, answer = answer.text()),
            None => line,
        })
    }

    /// Who the answer goes to: `Your answer goes to odile@example.org at once.`
    #[must_use]
    pub fn goes_to(&self) -> Option<String> {
        self.organizer_address.as_ref().map(|address| tr!("Your answer goes to {address} at once.", address = address))
    }

    /// What the owner answered: `You answered Going.`
    #[must_use]
    pub fn answered(&self) -> Option<String> {
        self.answer.map(|answer| tr!("You answered {answer}.", answer = answer.text()))
    }

    /// What a guest's answer says: `Béla is going.`
    #[must_use]
    pub fn reply_line(reply: &Reply) -> String {
        match reply.answer {
            Some(Rsvp::Going) => tr!("{name} is going.", name = reply.name),
            Some(Rsvp::Maybe) => tr!("{name} might go.", name = reply.name),
            Some(Rsvp::No) => tr!("{name} is not going.", name = reply.name),
            None => tr!("{name} has not answered.", name = reply.name),
        }
    }
}

#[cfg(test)]
#[path = "invite_tests.rs"]
mod tests;
