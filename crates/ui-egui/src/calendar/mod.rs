// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen's state, behind the rail's Calendar button: the calendars of every account (`calendar.list`)
//! and the events of the days shown (`event.list`, repeating events expanded), in one of four views: a day, a week,
//! a month or an agenda of the next [`AGENDA_DAYS`] days. A calendar can be hidden; an event can be picked, and the
//! side panel then shows its details. Without a calendar the screen shows its empty state. The screen is drawn by
//! `draw/calendar/`.
//!
//! New event opens the [`Editor`] on the day the view is on; Edit opens it on the picked event, Delete deletes that.
//! Each saved change and each delete is an act of the engine's history with a toast whose Undo takes it back
//! ([`Mailbox::event_act`]); the server is asked to delete only once the undo window is over.
//!
//! The times are the computer's wall clock as the engine gives them (`local_start`, `local_end`); the event's own
//! zone comes along only when its clock shows another time ([`Zone`]). Event text is the organiser's data: it is
//! shown on one line ([`shown`]) and never acted on.

pub mod day;
pub mod editor;
pub mod layout;
pub mod sample;
pub mod sync;

use std::collections::BTreeSet;

use inboxcraft_time::{Offset, Timestamp, WeekRule};
use serde_json::{Value, json};

pub use self::day::Day;
pub use self::editor::{Alert, Editor};

use crate::mailbox::{Mailbox, shown};
use crate::tr;

/// How many days the agenda lists, from its first day.
pub const AGENDA_DAYS: i64 = 30;

/// The minutes of a day.
pub const DAY_MINUTES: u16 = 24 * 60;

/// How the calendar shows its days.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    /// One day, hour by hour.
    Day,
    /// The seven days of a week from the week's first day ([`Calendar::week_rule`]), hour by hour.
    #[default]
    Week,
    /// The weeks of a month.
    Month,
    /// A list of what is coming, day by day.
    Agenda,
}

impl View {
    /// Every view, in the toolbar's order.
    pub const ALL: [Self; 4] = [Self::Day, Self::Week, Self::Month, Self::Agenda];

    /// Its name, on its pill.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Day => tr!("Day"),
            Self::Week => tr!("Week"),
            Self::Month => tr!("Month"),
            Self::Agenda => tr!("Agenda"),
        }
    }

    /// Its id in `ui_shot` scripts: `day`, `week`, `month`, `agenda`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Agenda => "agenda",
        }
    }

    /// The view with the id `id`.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.id() == id)
    }
}

/// A calendar of an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarRow {
    /// Its id.
    pub id: u64,
    /// The account it belongs to.
    pub account: u64,
    /// Its name.
    pub name: String,
    /// Its colour as the server gives it, `[r, g, b]`, when it gives one the window reads (`#rrggbb`, `#rrggbbaa`).
    pub colour: Option<[u8; 3]>,
}

/// A wall-clock moment: a day and the minutes since its midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment {
    /// The day.
    pub day: Day,
    /// The minutes since its midnight, 0 to 1439.
    pub minute: u16,
}

impl Moment {
    /// The moment an ISO 8601 date and time starts with (`2026-11-09T09:30:00+01:00`); midnight for a date alone.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let day = Day::parse(text)?;
        let Some(time) = text.get(10..).filter(|rest| !rest.is_empty()) else { return Some(Self { day, minute: 0 }) };
        let two = |from: usize| time.get(from..from.saturating_add(2)).filter(|part| part.bytes().all(|b| b.is_ascii_digit()))?.parse::<u16>().ok();
        if time.get(0..1) != Some("T") || time.get(3..4) != Some(":") {
            return None;
        }
        let (hour, minute) = (two(1)?, two(4)?);
        (hour < 24 && minute < 60).then_some(Self { day, minute: hour * 60 + minute })
    }

    /// The time on its own: `09:30`.
    #[must_use]
    pub fn clock(self) -> String {
        format!("{:02}:{:02}", self.minute / 60, self.minute % 60)
    }
}

/// The event's own zone with its wall-clock times, shown when its clock shows another time than the computer's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    /// Its name (`TZID`): `Atlantic/Azores`.
    pub name: String,
    /// The start on the zone's clock.
    pub start: Moment,
    /// The end on the zone's clock.
    pub end: Moment,
}

/// A guest of an event and their answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guest {
    /// The name, or the address without one.
    pub name: String,
    /// The address, or empty when the event gives none.
    pub address: String,
    /// The answer: `accepted`, `declined`, `tentative`, `needs_action`, `delegated` or `other`.
    pub status: String,
}

/// One occurrence of an event in the days shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRow {
    /// The event's id.
    pub id: u64,
    /// Its calendar.
    pub calendar: u64,
    /// What it is called (`(No title)` when it says nothing).
    pub summary: String,
    /// Where, or empty.
    pub location: String,
    /// What it says about itself, on one line, or empty.
    pub description: String,
    /// The start on the computer's clock.
    pub start: Moment,
    /// The end on the computer's clock.
    pub end: Moment,
    /// The first day it covers.
    pub first: Day,
    /// The last day it covers (an end at midnight belongs to the day before).
    pub last: Day,
    /// A whole day or days, without times.
    pub all_day: bool,
    /// Busy (or free) time.
    pub busy: bool,
    /// Private.
    pub private: bool,
    /// Its repeat preset (`weekly`), `custom`, or `None` when it does not repeat.
    pub repeat: Option<String>,
    /// The event's own zone, when its clock differs from the computer's.
    pub zone: Option<Zone>,
    /// The guests.
    pub guests: Vec<Guest>,
    /// Its alert.
    pub alert: Alert,
    /// Who organises it, by name or else address, when it names someone.
    pub organizer: Option<String>,
    /// Whether it is the owner's own: it names no organiser, or the owner organises it. Someone else's event opens
    /// read-only in the editor.
    pub own: bool,
    /// Whether the event has no title of its own (`summary` then says `(No title)`).
    pub untitled: bool,
}

impl EventRow {
    /// Whether it shows as a block over whole days (all-day, or over more than one day) rather than in the hours.
    #[must_use]
    pub fn is_block(&self) -> bool {
        self.all_day || self.last > self.first
    }

    /// Whether it covers `day`.
    #[must_use]
    pub fn covers(&self, day: Day) -> bool {
        self.first <= day && day <= self.last
    }

    /// The occurrence's key: the event and its start.
    #[must_use]
    pub fn key(&self) -> Picked {
        Picked { id: self.id, start: self.start }
    }

    /// Its times: `09:30 – 10:30`, or `All day`.
    #[must_use]
    pub fn times(&self) -> String {
        if self.all_day { tr!("All day").to_owned() } else { format!("{} – {}", self.start.clock(), self.end.clock()) }
    }

    /// Its days and times in full, both dates when it covers more than one day: `Thursday 5 November, 09:30 – 10:30`,
    /// `Monday 16 November – Wednesday 18 November`, `Sunday 8 November, 23:00 – Monday 9 November, 01:00`.
    #[must_use]
    pub fn when(&self) -> String {
        when(self.all_day, self.first, self.last, self.start, self.end)
    }

    /// What its accessible name says: the summary and when.
    #[must_use]
    pub fn spoken(&self) -> String {
        tr!("{summary}, {when}", summary = self.summary, when = self.when())
    }

    /// The line on the event's own zone, when it differs from the computer's: `Event zone Atlantic/Azores: 07:00 –
    /// 10:00`, with the dates when the zone's days are not the computer's.
    #[must_use]
    pub fn zone_line(&self) -> Option<String> {
        let zone = self.zone.as_ref()?;
        let same_days = zone.start.day == self.start.day && zone.end.day == self.end.day;
        let times = if same_days {
            format!("{} – {}", zone.start.clock(), zone.end.clock())
        } else {
            let last = if zone.end.minute == 0 && zone.end.day > zone.start.day { zone.end.day.plus(-1) } else { zone.end.day };
            when(false, zone.start.day, last.max(zone.start.day), zone.start, zone.end)
        };
        Some(tr!("Event zone {zone}: {times}", zone = zone.name, times = times))
    }

    pub(crate) fn read(value: &Value) -> Option<Self> {
        let text = |key: &str| line(value.get(key).and_then(Value::as_str).unwrap_or_default());
        let moment = |key: &str| value.get(key).and_then(Value::as_str).and_then(Moment::parse);
        let day = |key: &str| value.get(key).and_then(Value::as_str).and_then(Day::parse);
        let (start, end) = (moment("local_start")?, moment("local_end")?);
        let first = day("start_date").unwrap_or(start.day);
        let last = day("end_date").unwrap_or(end.day).max(first);
        let summary = text("summary");
        let zone = value.get("zone").and_then(|zone| {
            let name = shown(zone.get("name")?.as_str()?);
            Some(Zone { name, start: Moment::parse(zone.get("start")?.as_str()?)?, end: Moment::parse(zone.get("end")?.as_str()?)? })
        });
        let guests = value
            .get("guests")
            .and_then(Value::as_array)
            .map(|guests| {
                guests
                    .iter()
                    .filter(|guest| guest.is_object())
                    .map(|guest| {
                        let name = guest.get("name").and_then(Value::as_str).filter(|name| !name.trim().is_empty());
                        let name = name.or_else(|| guest.get("address").and_then(Value::as_str)).unwrap_or_default();
                        Guest {
                            name: shown(name),
                            address: shown(guest.get("address").and_then(Value::as_str).unwrap_or_default()),
                            status: shown(guest.get("status").and_then(Value::as_str).unwrap_or_default()),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            id: value.get("id").and_then(Value::as_u64)?,
            calendar: value.get("calendar").and_then(Value::as_u64)?,
            untitled: summary.trim().is_empty(),
            summary: if summary.trim().is_empty() { tr!("(No title)").to_owned() } else { summary },
            location: text("location"),
            description: paragraphs(value.get("description").and_then(Value::as_str).unwrap_or_default()),
            start,
            end,
            first,
            last,
            all_day: value.get("all_day").and_then(Value::as_bool).unwrap_or(false),
            busy: value.get("busy").and_then(Value::as_bool).unwrap_or(true),
            private: value.get("private").and_then(Value::as_bool).unwrap_or(false),
            repeat: value.get("repeat").and_then(Value::as_str).map(shown),
            zone,
            guests,
            alert: Alert::read(value.get("alert")),
            organizer: value.get("organizer").and_then(|organizer| {
                let name = organizer.get("name").and_then(Value::as_str).filter(|name| !name.trim().is_empty());
                name.or_else(|| organizer.get("address").and_then(Value::as_str)).filter(|name| !name.trim().is_empty()).map(shown)
            }),
            // A row that does not say is the owner's, as before the engine said it.
            own: value.get("own").and_then(Value::as_bool).unwrap_or(true),
        })
    }
}

/// When an occurrence from `start` to `end` takes place, on the clock of `offset`, as an alert's notification says it:
/// [`EventRow::when`]'s words; empty for a time the clock cannot show.
#[must_use]
pub fn alert_when(all_day: bool, start: Timestamp, end: Timestamp, offset: Offset) -> String {
    let end = end.max(start);
    // The last day it covers: an end at midnight belongs to the day before.
    let last = end.checked_sub(std::time::Duration::from_secs(1)).filter(|last| *last >= start).unwrap_or(start);
    match (moment(start, offset), moment(end, offset), Day::of(last, offset)) {
        (Some(from), Some(to), Some(last)) => when(all_day, from.day, if all_day { last } else { to.day }, from, to),
        _ => String::new(),
    }
}

/// `at` on the clock of `offset`.
fn moment(at: Timestamp, offset: Offset) -> Option<Moment> {
    let civil = at.to_civil(offset)?;
    Some(Moment { day: Day::new(civil.year(), civil.month(), civil.day())?, minute: u16::from(civil.hour()) * 60 + u16::from(civil.minute()) })
}

/// Days and times in full, both dates when they differ.
fn when(all_day: bool, first: Day, last: Day, start: Moment, end: Moment) -> String {
    match (all_day, first == last) {
        (true, true) => day::dated(first),
        (true, false) => tr!("{first} – {last}", first = day::dated(first), last = day::dated(last)),
        (false, true) => tr!("{day}, {from} – {to}", day = day::dated(first), from = start.clock(), to = end.clock()),
        (false, false) => {
            tr!("{first}, {from} – {last}, {to}", first = day::dated(start.day), from = start.clock(), last = day::dated(end.day), to = end.clock())
        }
    }
}

/// An occurrence picked on the screen: its event and its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picked {
    /// The event's id.
    pub id: u64,
    /// The occurrence's start.
    pub start: Moment,
}

/// What the calendar screen shows.
#[derive(Debug, Default)]
pub struct Calendar {
    /// The calendars of every account.
    calendars: Vec<CalendarRow>,
    /// The calendars hidden by the owner, by id.
    hidden: BTreeSet<u64>,
    /// The occurrences in the days shown, by start.
    events: Vec<EventRow>,
    view: View,
    /// The day the view is on; `None` until the first read, which puts it on today.
    focus: Option<Day>,
    /// Today on the computer's clock, at the last read.
    today: Option<Day>,
    /// The time now on the computer's clock, at the last read.
    now: Option<Moment>,
    /// The first day of the month the mini month shows.
    month: Option<Day>,
    picked: Option<Picked>,
    /// Whether `event.list` said it left occurrences out.
    truncated: bool,
    /// How many events could not be read.
    unreadable: u64,
    /// The event editor, while it is open.
    editor: Option<Editor>,
    /// Changes whenever New event or Edit opens an editor, one in the place of another too ([`Calendar::editor_opened`]).
    opened: u64,
    /// The accounts the calendars belong to, as of the last read: id, place in the account list, name.
    accounts: Vec<(u64, usize, String)>,
    /// The day the arrows picked in the month or the mini month, if they did.
    picked_day: Option<Day>,
    /// The day whose events show in a float ("+N more"), while it is open.
    more: Option<Day>,
    /// The seconds of the minute now, at the last tick: how long the now line rests.
    second: u8,
    /// How the owner's weeks are counted, as the session said at the last read ([`Calendar::follow_week`]).
    week: WeekRule,
    /// Whether the calendars were read once ([`Calendar::refresh`]): a calendar never read follows no week rule.
    read: bool,
    /// Changes whenever the events shown change (a read, a calendar shown or hidden): what the views laid out of the
    /// events is kept while it stays the same.
    serial: u64,
    /// The calendars' sync at the side panel's foot, shared with the mailbox that runs it ([`sync`]).
    sync: sync::SyncHandle,
}

/// The next [`Calendar::serial`]: never the same twice in a run, whichever calendar takes it.
fn next_serial() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// What the keyboard is on in the calendar: the button the next frame hands the focus to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// An occurrence's block.
    Event(Picked),
    /// A day's number in the month view.
    Day(Day),
    /// A day of the mini month.
    Mini(Day),
    /// The picked event's details (their first control).
    Details,
}

/// Which way an arrow moves the pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// Left: a day back, or the event of the day before.
    Left,
    /// Right: a day on, or the event of the next day.
    Right,
    /// Up: a week back, or the event before.
    Up,
    /// Down: a week on, or the next event.
    Down,
}

impl Calendar {
    /// The calendars shown, by name.
    #[must_use]
    pub fn calendars(&self) -> &[CalendarRow] {
        &self.calendars
    }

    /// Whether there is no calendar to show: the screen shows its empty state.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.calendars.is_empty()
    }

    /// The view.
    #[must_use]
    pub fn view(&self) -> View {
        self.view
    }

    /// Today, as of the last read.
    #[must_use]
    pub fn today(&self) -> Day {
        self.today.unwrap_or(Day::EPOCH)
    }

    /// The time now, as of the last read: the line across today's hours.
    #[must_use]
    pub fn now(&self) -> Option<Moment> {
        self.now
    }

    /// The day the view is on.
    #[must_use]
    pub fn focus(&self) -> Day {
        self.focus.or(self.today).unwrap_or(Day::EPOCH)
    }

    /// The first day of the month the mini month shows.
    #[must_use]
    pub fn month(&self) -> Day {
        self.month.unwrap_or_else(|| self.focus().first_of_month())
    }

    /// How the owner's weeks are counted, as of the last read: the week view's days, the month's and the mini month's
    /// first column and the week's number follow it.
    #[must_use]
    pub fn week_rule(&self) -> WeekRule {
        self.week
    }

    /// The number of the week the view is on, as [`Calendar::week_rule`] counts it; `None` out of the years known.
    #[must_use]
    pub fn week_number(&self) -> Option<u8> {
        self.focus().week_number(self.week)
    }

    /// The first and the last day the view shows: the day; the week from its first day ([`Calendar::week_rule`]); the
    /// six weeks from the week's first day on or before the month's first day; or [`AGENDA_DAYS`] days.
    #[must_use]
    pub fn span(&self) -> (Day, Day) {
        let focus = self.focus();
        match self.view {
            View::Day => (focus, focus),
            View::Week => {
                let first = focus.week_start(self.week);
                (first, first.plus(6))
            }
            View::Month => {
                let first = focus.first_of_month().week_start(self.week);
                (first, first.plus(41))
            }
            View::Agenda => (focus, focus.plus(AGENDA_DAYS - 1)),
        }
    }

    /// The title over the view: `Thursday 5 November 2026`, `2 – 8 November 2026`, `November 2026`, `From today`.
    #[must_use]
    pub fn title(&self) -> String {
        let focus = self.focus();
        match self.view {
            View::Day => day::long(focus),
            View::Week => {
                let (first, last) = self.span();
                day::span(first, last)
            }
            View::Month => tr!("{month} {year}", month = day::month_name(focus.month()), year = focus.year()),
            View::Agenda if Some(focus) == self.today => tr!("From today").to_owned(),
            View::Agenda => tr!("From {day}", day = day::dated(focus)),
        }
    }

    /// The title short, for a toolbar with little room: the day or the days without the year (`Thu 5 Nov`,
    /// `2 – 8 Nov`), the month's short name (`Nov 2026`).
    #[must_use]
    pub fn short_title(&self) -> String {
        let focus = self.focus();
        match self.view {
            View::Day => day::short_dated(focus),
            View::Week => {
                let (first, last) = self.span();
                day::span_short(first, last)
            }
            View::Month => tr!("{month} {year}", month = day::month_short(focus.month()), year = focus.year()),
            View::Agenda if Some(focus) == self.today => tr!("From today").to_owned(),
            View::Agenda => tr!("From {day}", day = day::short_dated(focus)),
        }
    }

    /// Puts `events` in the days shown in place of what was read: a test's hostile events (zero or negative length,
    /// far-off dates, hundreds at once) and the bench's thousands, which no server gives this way.
    #[doc(hidden)]
    pub fn set_events(&mut self, events: Vec<EventRow>) {
        self.events = events;
        self.serial = next_serial();
    }

    /// What the events shown are now: it changes whenever they change, and never comes back to an old value.
    #[must_use]
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// The occurrences of the shown calendars in the days shown, by start.
    pub fn events(&self) -> impl Iterator<Item = &EventRow> {
        self.events.iter().filter(|event| !self.hidden.contains(&event.calendar))
    }

    /// The calendar `id`.
    #[must_use]
    pub fn calendar(&self, id: u64) -> Option<&CalendarRow> {
        self.calendars.iter().find(|calendar| calendar.id == id)
    }

    /// The place of calendar `id` in the list, for a colour of its own when the server gives none.
    #[must_use]
    pub fn index_of(&self, id: u64) -> usize {
        self.calendars.iter().position(|calendar| calendar.id == id).unwrap_or(0)
    }

    /// Whether calendar `id` is shown.
    #[must_use]
    pub fn is_shown(&self, id: u64) -> bool {
        !self.hidden.contains(&id)
    }

    /// The picked occurrence, while it is among the events shown.
    #[must_use]
    pub fn picked(&self) -> Option<&EventRow> {
        let picked = self.picked?;
        self.events().find(|event| event.key() == picked)
    }

    /// Whether `event.list` left occurrences out, and how many events could not be read.
    #[must_use]
    pub fn left_out(&self) -> (bool, u64) {
        (self.truncated, self.unreadable)
    }

    /// The place of the account of calendar `id` among the accounts, which picks its colour (H8); 0 when unknown.
    #[must_use]
    pub fn account_place(&self, id: u64) -> u64 {
        self.calendar(id).map_or(0, |row| self.account_index(row.account))
    }

    /// The place of account `account` among the accounts; 0 when unknown.
    #[must_use]
    pub fn account_index(&self, account: u64) -> u64 {
        self.accounts.iter().find(|(id, _, _)| *id == account).map_or(0, |(_, index, _)| u64::try_from(*index).unwrap_or(0))
    }

    /// The name of account `account`, or `Account` when it is not known.
    #[must_use]
    pub fn account_label(&self, account: u64) -> String {
        self.accounts.iter().find(|(id, _, _)| *id == account).map_or_else(|| tr!("Account").to_owned(), |(_, _, name)| name.clone())
    }

    /// The day the arrows picked in the month or the mini month, if they did.
    #[must_use]
    pub fn picked_day(&self) -> Option<Day> {
        self.picked_day
    }

    /// Picks `day` in the month or the mini month, as the keyboard reaching its button does.
    pub fn pick_day(&mut self, day: Option<Day>) {
        self.picked_day = day;
    }

    /// The day whose events show in a float, while it is open.
    #[must_use]
    pub fn more(&self) -> Option<Day> {
        self.more
    }

    /// Shows the events of `day` in a float, or with `None` closes it.
    pub fn set_more(&mut self, day: Option<Day>) {
        self.more = day;
    }

    /// Takes the time now from the clock (today, the now line) without reading the calendars again: each frame.
    pub fn tick(&mut self, now: Timestamp, offset: Offset) {
        if let Some(civil) = now.to_civil(offset) {
            self.second = civil.second();
            if let Some(day) = Day::new(civil.year(), civil.month(), civil.day()) {
                self.today = Some(day);
                self.now = Some(Moment { day, minute: u16::from(civil.hour()).saturating_mul(60).saturating_add(u16::from(civil.minute())) });
            }
        }
    }

    /// How long until the minute turns: the now line asks for a repaint then, never sooner.
    #[must_use]
    pub fn until_next_minute(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60_u64.saturating_sub(u64::from(self.second.min(59))))
    }

    /// Moves the pick one step `way`: in the month (or the mini month, `mini`) the picked day a day or a week, in the
    /// day, week and agenda the picked event to the one before or after (left and right to the day before or after in
    /// the week, the day itself in the day view). The view follows a day that leaves it. Where the keyboard goes.
    pub fn move_pick(&mut self, mailbox: &mut Mailbox, way: Way, mini: bool) -> Option<Target> {
        if mini || self.view == View::Month {
            let from = self.picked_day.unwrap_or_else(|| self.focus());
            let to = from.plus(match way {
                Way::Left => -1,
                Way::Right => 1,
                Way::Up => -7,
                Way::Down => 7,
            });
            self.picked_day = Some(to);
            if mini {
                self.month = Some(to.first_of_month());
                return Some(Target::Mini(to));
            }
            let (first, last) = self.span();
            if to.month() != self.focus().month() || to < first || to > last {
                self.go_to(mailbox, to);
            } else {
                self.focus = Some(to);
                self.month = Some(to.first_of_month());
            }
            return Some(Target::Day(to));
        }
        if self.view == View::Day && matches!(way, Way::Left | Way::Right) {
            self.step(mailbox, way == Way::Right);
            return None;
        }
        let order: Vec<Picked> = self.events().map(EventRow::key).collect();
        let at = self.picked.and_then(|picked| order.iter().position(|key| *key == picked));
        let next = match (way, at) {
            (Way::Down, None) | (Way::Right, None) => order.first().copied(),
            (Way::Up, None) | (Way::Left, None) => order.last().copied(),
            (Way::Down, Some(at)) => order.get(at.saturating_add(1)).copied(),
            (Way::Up, Some(at)) => at.checked_sub(1).and_then(|before| order.get(before)).copied(),
            (Way::Right | Way::Left, Some(at)) if self.view == View::Week => {
                let day = order.get(at).map(|key| key.start.day);
                let forward = way == Way::Right;
                let wanted = day.map(|day| day.plus(if forward { 1 } else { -1 }));
                let found = |key: &&Picked| wanted.is_some_and(|wanted| if forward { key.start.day >= wanted } else { key.start.day <= wanted });
                if forward { order.iter().find(found).copied() } else { order.iter().rev().find(found).copied() }
            }
            (Way::Right, Some(at)) => order.get(at.saturating_add(1)).copied(),
            (Way::Left, Some(at)) => at.checked_sub(1).and_then(|before| order.get(before)).copied(),
        };
        let next = next?;
        self.picked = Some(next);
        Some(Target::Event(next))
    }

    /// `Enter` on the pick: the picked day of the month opens in the day view, one of the mini month puts the view
    /// there; a picked event's details take the keyboard. Where the keyboard goes.
    pub fn open_pick(&mut self, mailbox: &mut Mailbox, mini: bool) -> Option<Target> {
        if let Some(day) = self.picked_day.filter(|_| mini || self.view == View::Month) {
            if mini {
                self.go_to(mailbox, day);
                return Some(Target::Mini(day));
            }
            self.picked_day = None;
            self.open_day(mailbox, day);
            return None;
        }
        self.picked().map(|_| Target::Details)
    }

    /// `Esc`: the picked event's details close and the picked day is let go.
    pub fn leave_pick(&mut self) {
        self.picked = None;
        self.picked_day = None;
    }

    /// Takes the week rule from the session, once a frame: when it changed (`week.set`), the days shown are read again
    /// for the new week, the view staying on its day and the picked day and event staying picked; otherwise nothing
    /// happens. A calendar not read yet reads nothing: its first read takes the rule.
    pub fn follow_week(&mut self, mailbox: &mut Mailbox) {
        if self.read && mailbox.week_rule() != self.week {
            self.refresh(mailbox);
        }
    }

    /// Reads the calendars and the events of the days shown again; today comes from the list's clock and the week rule
    /// from the session. The first read puts the view on today. A refused command shows in the window's error banner.
    pub fn refresh(&mut self, mailbox: &mut Mailbox) {
        let (now, offset) = (mailbox.now(), mailbox.offset());
        self.tick(now, offset);
        self.week = mailbox.week_rule();
        self.read = true;
        self.sync = mailbox.calendar_sync();
        self.accounts = mailbox.accounts().iter().map(|account| (account.id, account.index, account.label.clone())).collect();
        if self.focus.is_none() {
            self.focus = self.today;
            self.month = self.today.map(Day::first_of_month);
        }
        if let Err(error) = self.load(mailbox) {
            mailbox.fail(error);
        }
    }

    fn load(&mut self, mailbox: &mut Mailbox) -> Result<(), crate::UiError> {
        let answer = mailbox.run("calendar.list", &json!({}))?;
        self.calendars =
            answer.get("calendars").and_then(Value::as_array).map(|rows| rows.iter().filter_map(calendar_row).collect()).unwrap_or_default();
        self.hidden.retain(|id| self.calendars.iter().any(|calendar| calendar.id == *id));
        self.events.clear();
        self.serial = next_serial();
        (self.truncated, self.unreadable) = (false, 0);
        if self.calendars.is_empty() {
            return Ok(());
        }
        // A day either side: the engine reads UTC midnights, the computer's days may start hours before or after.
        let (first, last) = self.span();
        let params = json!({ "from": first.plus(-1).iso_midnight(), "to": last.plus(2).iso_midnight() });
        let answer = mailbox.run("event.list", &params)?;
        let rows = answer.get("events").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        self.events = rows.iter().filter_map(EventRow::read).filter(|event| event.last >= first && event.first <= last).collect();
        self.truncated = answer.get("truncated").and_then(Value::as_bool).unwrap_or(false);
        self.unreadable = answer.get("unreadable").and_then(Value::as_u64).unwrap_or(0);
        self.serial = next_serial();
        Ok(())
    }

    /// What the side panel's foot shows of the calendars' sync.
    #[must_use]
    pub fn sync_view(&self) -> sync::SyncView {
        sync::with(&self.sync, |tracker| tracker.view())
    }

    /// The owner pressed "sync now" (or "try again"): `calendar.sync` runs now, unless a sync is under way. Whether it
    /// ran.
    pub fn sync_now(&mut self, mailbox: &mut Mailbox) -> bool {
        mailbox.sync_calendars()
    }

    /// Shows the days in `view`, around the same day.
    pub fn set_view(&mut self, mailbox: &mut Mailbox, view: View) {
        self.view = view;
        self.refresh(mailbox);
    }

    /// Moves the view a step on (or back): a day, a week, a month, or the agenda's days.
    pub fn step(&mut self, mailbox: &mut Mailbox, forward: bool) {
        let focus = self.focus();
        let sign = if forward { 1 } else { -1 };
        let next = match self.view {
            View::Day => focus.plus(sign),
            View::Week => focus.plus(7 * sign),
            View::Month => focus.months_later(i32::from(forward) * 2 - 1),
            View::Agenda => focus.plus(AGENDA_DAYS * sign),
        };
        self.go_to(mailbox, next);
    }

    /// Puts the view on `day`, and the mini month on its month.
    pub fn go_to(&mut self, mailbox: &mut Mailbox, day: Day) {
        self.focus = Some(day);
        self.month = Some(day.first_of_month());
        self.refresh(mailbox);
    }

    /// Puts the view on today.
    pub fn go_today(&mut self, mailbox: &mut Mailbox) {
        let (now, offset) = (mailbox.now(), mailbox.offset());
        let today = Day::of(now, offset).unwrap_or_else(|| self.today());
        self.go_to(mailbox, today);
    }

    /// Opens `day` in the day view, as a press on its number does.
    pub fn open_day(&mut self, mailbox: &mut Mailbox, day: Day) {
        self.view = View::Day;
        self.go_to(mailbox, day);
    }

    /// Moves the mini month a month on (or back), the view staying where it is.
    pub fn step_month(&mut self, forward: bool) {
        self.month = Some(self.month().months_later(i32::from(forward) * 2 - 1));
    }

    /// Shows or hides calendar `id`; a picked occurrence of a hidden calendar is let go.
    pub fn toggle(&mut self, id: u64) {
        if !self.hidden.remove(&id) {
            self.hidden.insert(id);
        }
        self.serial = next_serial();
        if self.picked().is_none() {
            self.picked = None;
        }
    }

    /// Puts the view on the day the occurrence of event `id` at `start` falls on and picks it, as a click on its alert
    /// asks; an occurrence that does not start at that minute any more is found by its day, and an event gone is not
    /// picked.
    pub fn open_event(&mut self, mailbox: &mut Mailbox, id: u64, start: Timestamp) {
        let Some(at) = moment(start, mailbox.offset()) else { return };
        self.go_to(mailbox, at.day);
        let exact = Picked { id, start: at };
        let found =
            self.events.iter().find(|event| event.key() == exact).or_else(|| self.events.iter().find(|event| event.id == id && event.covers(at.day)));
        self.picked = found.map(EventRow::key);
        self.hidden.retain(|calendar| found.is_none_or(|event| event.calendar != *calendar));
        self.serial = next_serial();
    }

    /// Picks an occurrence (`None` lets it go); the side panel shows its details.
    pub fn pick(&mut self, picked: Option<Picked>) {
        self.picked = picked;
    }

    /// The event editor, while it is open.
    #[must_use]
    pub fn editor(&self) -> Option<&Editor> {
        self.editor.as_ref()
    }

    /// Which opening of the editor is open: it changes whenever New event or Edit opens one, also in the place of an
    /// editor with nothing typed, so that the new one is met as new (its title takes the keyboard).
    #[must_use]
    pub fn editor_opened(&self) -> u64 {
        self.opened
    }

    /// The event editor, to type into.
    pub fn editor_mut(&mut self) -> Option<&mut Editor> {
        self.editor.as_mut()
    }

    /// Opens the editor on a new event on the day the view is on, in the first calendar shown (the first of all when
    /// every one is hidden). Without a calendar nothing opens.
    pub fn new_event(&mut self) {
        if self.keeps_typing() {
            return;
        }
        let Some(first) = self.calendars.iter().find(|row| self.is_shown(row.id)).or_else(|| self.calendars.first()) else { return };
        let choices = self.calendars.iter().map(|row| (row.id, row.name.clone())).collect();
        self.editor = Some(Editor::new_event(first.id, self.focus(), choices));
        self.opened = next_serial();
    }

    /// Opens the editor on the picked event; nothing opens without one.
    pub fn edit_picked(&mut self) {
        if self.keeps_typing() {
            return;
        }
        if let Some(event) = self.picked() {
            self.editor = Some(Editor::edit(event));
            self.opened = next_serial();
        }
    }

    /// Closes the editor; what was typed is let go (Cancel).
    pub fn close_editor(&mut self) {
        self.editor = None;
    }

    /// Esc (`escape`) or a press outside the editor. With nothing typed it closes. With typing, the typing is thrown
    /// away only by an act that says so: an Esc after the warning an Esc raised, with nothing changed since. A press
    /// outside never closes it, however often, and says how to keep or throw away the typing; the first Esc after
    /// that warning (or New event's or Edit's) warns as Esc does.
    pub fn leave_editor(&mut self, escape: bool) {
        match self.editor.as_mut() {
            Some(editor) if editor.edited() && !(escape && editor.warned_by_escape()) => editor.warn(escape),
            _ => self.editor = None,
        }
    }

    /// Whether an open editor holds typing that another editor would replace: it then stays, and says so.
    fn keeps_typing(&mut self) -> bool {
        match self.editor.as_mut() {
            Some(editor) if editor.edited() => {
                editor.warn(false);
                true
            }
            _ => false,
        }
    }

    /// Saves the editor's event: a new one is made (`event.create`), an edit sends what changed (`event.update`), and
    /// the toast says so with Undo. An edit that changed nothing closes the editor and runs nothing. What cannot be
    /// read or is refused keeps the editor open, saying why.
    pub fn save(&mut self, mailbox: &mut Mailbox) {
        let Some(editor) = self.editor.as_mut() else { return };
        let offset = mailbox.offset();
        let (command, params) = match editor.changes(offset) {
            Ok(Some(change)) => change,
            Ok(None) => {
                self.editor = None;
                return;
            }
            Err(why) => return editor.refused(why),
        };
        let text = match command {
            "event.create" => tr!("Event added"),
            _ if params.get("notify").is_some() => tr!("Event saved; the guests are told"),
            _ => tr!("Event saved"),
        };
        match mailbox.event_act(command, &params, text) {
            Ok(_) => {
                self.editor = None;
                self.refresh(mailbox);
            }
            Err(error) => editor.refused(error.to_string()),
        }
    }

    /// Deletes the picked event (`event.delete`), with a toast whose Undo brings it back; the server deletes it once
    /// the undo window is over. A refusal shows in the window's error banner.
    pub fn delete_picked(&mut self, mailbox: &mut Mailbox) {
        let Some(id) = self.picked().map(|event| event.id) else { return };
        match mailbox.event_act("event.delete", &json!({ "id": id }), tr!("Event deleted")) {
            Ok(_) => {
                self.picked = None;
                // What was typed in the editor stays; an editor with nothing typed goes with the event.
                if !self.editor.as_ref().is_some_and(Editor::edited) {
                    self.editor = None;
                }
                self.refresh(mailbox);
            }
            Err(error) => mailbox.fail(error),
        }
    }
}

/// The longest description read, in characters: a dial-in with its agenda fits many times over.
pub const MAX_DESCRIPTION: usize = 10_000;

/// A description from a server fit to be read as text: its line breaks kept, every other control character a space,
/// the bidi controls that could reorder it gone, cut to [`MAX_DESCRIPTION`] characters.
fn paragraphs(text: &str) -> String {
    text.replace("\r\n", "\n")
        .chars()
        .filter(|c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'))
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .take(MAX_DESCRIPTION)
        .collect()
}

/// An event's title or place from a server fit for one line, and whole: every control character (line breaks, tabs,
/// NUL) a space, the bidi controls that could reorder it gone, cut only past the engine's own cap
/// ([`editor::MAX_FIELD`], 1 000 characters). The details wrap it; an Edit opens on it, so it sends nothing it did not
/// change.
fn line(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'))
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(editor::MAX_FIELD)
        .collect()
}

fn calendar_row(value: &Value) -> Option<CalendarRow> {
    Some(CalendarRow {
        id: value.get("id").and_then(Value::as_u64)?,
        account: value.get("account").and_then(Value::as_u64).unwrap_or(0),
        name: shown(value.get("name").and_then(Value::as_str).unwrap_or_default()),
        colour: value.get("colour").and_then(Value::as_str).and_then(colour),
    })
}

/// A colour as CalDAV servers write it: `#rrggbb` or `#rrggbbaa` (the alpha is dropped).
#[must_use]
pub fn colour(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#')?;
    if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let part = |at: usize| u8::from_str_radix(hex.get(at..at.saturating_add(2))?, 16).ok();
    Some([part(0)?, part(2)?, part(4)?])
}

#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "week_tests.rs"]
mod week_tests;
