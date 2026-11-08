// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Event alerts: when an event's alerts go off, kept in the store as [`Alert`]s with a schedule entry each, so the
//! scheduler fires them and [`crate::notify`] tells the owner.
//!
//! - An event's own `DISPLAY` and `AUDIO` alarms (RFC 5545 section 3.6.6) count; an event without any alarm gets the
//!   defaults: [`DEFAULT_LEAD`] before a timed event and a day before an all-day one. Mail and other alarms are the
//!   server's business, so an event with only those alerts nothing here.
//! - A trigger's days follow the wall clock of the event's zone (the computer's for a floating time or a whole day),
//!   its hours and minutes do not: a day before noon is noon the day before even when the clock is put back between.
//! - A cancelled event, and one the owner declined, alerts nothing. An alarm further than [`MAX_LEAD`] from its event
//!   is left out.
//! - The store holds an event's alerts of the next [`HORIZON`], at most [`MAX_ALERTS`]: they are worked out again
//!   whenever the event changes, and for every event when the session starts and every [`SWEEP`] after, so a repeating
//!   or a far event comes into the horizon in time. An alert whose time came and that has not fired yet stays until
//!   the scheduler fires it.

use std::collections::BTreeMap;
use std::time::Duration;

use inboxcraft_ical::{Calendar, Event, PartStat, Span, Status, Trigger};
use inboxcraft_model::{AccountId, Alert, AlertId, CalendarEvent, CalendarEventId, CalendarId, Due, ScheduleId, Scheduled};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{CivilDateTime, Offsets, Timestamp};

use crate::EngineError;
use crate::calendar::{address_of, calendar_by_id, read};

/// How far ahead the store holds alerts.
pub(crate) const HORIZON: Duration = Duration::from_secs(14 * 86_400);

/// How often every event's alerts are worked out again while the session runs.
pub(crate) const SWEEP: Duration = Duration::from_secs(86_400);

/// The most alerts the store holds for one event; the earliest are kept.
pub(crate) const MAX_ALERTS: usize = 64;

/// The furthest an alarm goes off from its event; one further is left out.
pub(crate) const MAX_LEAD: Duration = Duration::from_secs(366 * 86_400);

/// How long before a timed event without alarms its alert goes off.
pub(crate) const DEFAULT_LEAD: Duration = Duration::from_secs(600);

/// The most alarms read of one event.
const MAX_ALARMS: usize = 16;

/// A nominal day may be an hour or two longer or shorter than 24 hours; the span looked at is this much wider.
const SLACK: i64 = 86_400;

/// An alert of one occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ring {
    /// When it goes off.
    pub(crate) at: Timestamp,
    /// When the occurrence starts.
    pub(crate) start: Timestamp,
    /// When it ends.
    pub(crate) end: Timestamp,
    /// The event's index in the calendar object.
    pub(crate) event: usize,
}

/// The alerts of `calendar` that go off after `from` and no later than `to`, by time; `local` is the computer's
/// clock, `owner` the address of the account it belongs to, if known.
pub(crate) fn rings(calendar: &Calendar, from: Timestamp, to: Timestamp, local: &dyn Offsets, owner: Option<&str>) -> Vec<Ring> {
    let triggers: Vec<Vec<Trigger>> = calendar.events.iter().map(|event| if alerts(event, owner) { triggers(event) } else { Vec::new() }).collect();
    let (mut before, mut after) = (0_i64, 0_i64);
    for trigger in triggers.iter().flatten() {
        if let Trigger::Start(span) | Trigger::End(span) = trigger {
            let seconds = seconds(*span).unwrap_or_default();
            before = before.max(seconds.saturating_neg());
            after = after.max(seconds);
        }
    }
    // An occurrence alerts in the span when it overlaps the span widened by the furthest triggers either way.
    let look_from = Timestamp::from_unix(from.unix_seconds().saturating_sub(after.saturating_add(SLACK)), 0).unwrap_or(from);
    let look_to = Timestamp::from_unix(to.unix_seconds().saturating_add(before.saturating_add(SLACK)), 0).unwrap_or(to);
    let mut found = Vec::new();
    let within = |at: Timestamp| at > from && at <= to;
    for occurrence in calendar.occurrences(look_from, look_to, local).occurrences {
        let (Some(event), Some(triggers)) = (calendar.events.get(occurrence.event), triggers.get(occurrence.event)) else { continue };
        let zone = calendar.zones.offsets(&event.start, local);
        for trigger in triggers {
            let at = match trigger {
                Trigger::Start(span) => shift(occurrence.start, *span, zone),
                Trigger::End(span) => shift(occurrence.end, *span, zone),
                Trigger::At(_) => None,
            };
            if let Some(at) = at.filter(|at| within(*at)) {
                found.push(Ring { at, start: occurrence.start, end: occurrence.end, event: occurrence.event });
            }
        }
    }
    for (index, (event, triggers)) in calendar.events.iter().zip(&triggers).enumerate() {
        for trigger in triggers {
            if let Trigger::At(at) = trigger
                && within(*at)
                && let Some(start) = calendar.zones.resolve(&event.start, local)
            {
                let end = event.end.as_ref().and_then(|end| calendar.zones.resolve(end, local)).filter(|end| *end >= start).unwrap_or(start);
                found.push(Ring { at: *at, start, end, event: index });
            }
        }
    }
    found.sort_by_key(|ring| (ring.at, ring.start, ring.event));
    found.dedup_by_key(|ring| (ring.at, ring.start, ring.event));
    found
}

/// The times `calendar`'s alerts go off in the [`HORIZON`] after `now`, each once, at most [`MAX_ALERTS`].
pub(crate) fn upcoming(calendar: &Calendar, now: Timestamp, local: &dyn Offsets, owner: Option<&str>) -> Vec<Timestamp> {
    let to = now.checked_add(HORIZON).unwrap_or(now);
    let mut times: Vec<Timestamp> = rings(calendar, now, to, local, owner).into_iter().map(|ring| ring.at).collect();
    times.dedup();
    times.truncate(MAX_ALERTS);
    times
}

/// Whether `event` alerts the owner at all: not cancelled, not declined by `owner`.
fn alerts(event: &Event, owner: Option<&str>) -> bool {
    let declined = owner.is_some_and(|owner| {
        event.attendees.iter().any(|attendee| attendee.status == PartStat::Declined && address_of(attendee).eq_ignore_ascii_case(owner))
    });
    event.status != Some(Status::Cancelled) && !declined
}

/// The triggers of `event`'s alerts: its display and sound alarms, or the defaults when it has no alarm.
fn triggers(event: &Event) -> Vec<Trigger> {
    if event.alarms.is_empty() {
        let lead = i64::try_from(DEFAULT_LEAD.as_secs()).unwrap_or_default();
        let span = if event.all_day() { Span::new(-1, 0) } else { Span::new(0, lead.saturating_neg()) };
        return span.map(Trigger::Start).into_iter().collect();
    }
    event
        .alarms
        .iter()
        .take(MAX_ALARMS)
        .filter(|alarm| alarm.action.eq_ignore_ascii_case("DISPLAY") || alarm.action.eq_ignore_ascii_case("AUDIO"))
        .map(|alarm| alarm.trigger)
        .filter(|trigger| match trigger {
            Trigger::Start(span) | Trigger::End(span) => seconds(*span).is_some(),
            Trigger::At(_) => true,
        })
        .collect()
}

/// `span` in seconds, days as 24 hours; `None` beyond [`MAX_LEAD`].
fn seconds(span: Span) -> Option<i64> {
    let seconds = span.days().checked_mul(86_400)?.checked_add(span.seconds())?;
    (seconds.unsigned_abs() <= MAX_LEAD.as_secs()).then_some(seconds)
}

/// `at` moved by `span`: its days on the wall clock of `zone`, then its seconds.
fn shift(at: Timestamp, span: Span, zone: &dyn Offsets) -> Option<Timestamp> {
    seconds(span)?;
    let moved = if span.days() == 0 {
        at
    } else {
        let civil = zone.civil(at)?;
        let day = inboxcraft_ical::Date::of(civil).add_days(span.days())?;
        zone.resolve(CivilDateTime::new(day.year(), day.month(), day.day(), civil.hour(), civil.minute(), civil.second())?)?
    };
    Timestamp::from_unix(moved.unix_seconds().checked_add(span.seconds())?, 0)
}

/// The ids the next alert and its schedule entry get.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ids {
    pub(crate) alert: u64,
    pub(crate) schedule: u64,
}

impl Ids {
    /// From the session's next ids, past every alert and entry `batch` puts.
    pub(crate) fn past(alert: u64, schedule: u64, batch: &Batch) -> Self {
        let mut ids = Self { alert, schedule };
        for change in batch.changes() {
            match change {
                Change::PutAlert(held) => ids.alert = ids.alert.max(held.id.get().saturating_add(1)),
                Change::PutScheduled(entry) => ids.schedule = ids.schedule.max(entry.id.get().saturating_add(1)),
                _ => {}
            }
        }
        ids
    }
}

/// The changes that keep the alerts of the events `batch` puts in step with them, made after it in the same batch.
/// An event the batch takes away again (with its calendar or an account) is left alone; one it took away before
/// putting it back has lost its alerts.
pub(crate) fn follow(store: &dyn Storage, batch: &Batch, now: Timestamp, local: &dyn Offsets, ids: &mut Ids) -> Result<Vec<Change>, EngineError> {
    let mut events: BTreeMap<CalendarEventId, (Option<&CalendarEvent>, bool)> = BTreeMap::new();
    let mut calendars: BTreeMap<CalendarId, AccountId> = BTreeMap::new();
    for change in batch.changes() {
        match change {
            Change::PutCalendarEvent(event) => {
                let gone = events.get(&event.id).is_some_and(|(_, gone)| *gone);
                events.insert(event.id, (Some(event), gone));
            }
            Change::RemoveCalendarEvent(id) => {
                events.insert(*id, (None, true));
            }
            Change::RemoveCalendar(calendar) => {
                for (held, gone) in events.values_mut() {
                    if held.is_some_and(|event| event.calendar == *calendar) {
                        (*held, *gone) = (None, true);
                    }
                }
            }
            Change::RemoveAccount(_) => events.values_mut().for_each(|(held, gone)| (*held, *gone) = (None, true)),
            Change::PutCalendar(calendar) => {
                calendars.insert(calendar.id, calendar.account);
            }
            _ => {}
        }
    }
    let mut changes = Vec::new();
    if events.values().all(|(held, _)| held.is_none()) {
        return Ok(changes);
    }
    let accounts = store.accounts()?;
    for (held, gone) in events.into_values() {
        let Some(held) = held else { continue };
        let account = match calendars.get(&held.calendar) {
            Some(account) => Some(*account),
            None => calendar_by_id(store, held.calendar)?.map(|calendar| calendar.account),
        };
        let owner = account.and_then(|id| accounts.iter().find(|account| account.id() == id)).map(|account| account.address().as_str());
        let alerts = if gone { Vec::new() } else { store.alerts(held.id)? };
        changes.extend(refresh(alerts, held, owner, now, local, ids));
    }
    Ok(changes)
}

/// The changes that bring the alerts of every event the store holds in step at `now`.
pub(crate) fn sweep(store: &dyn Storage, now: Timestamp, local: &dyn Offsets, ids: &mut Ids) -> Result<Vec<Change>, EngineError> {
    let mut changes = Vec::new();
    for account in store.accounts()? {
        let owner = account.address().as_str();
        for calendar in store.calendars(account.id())? {
            for held in store.calendar_events(calendar.id)? {
                let alerts = store.alerts(held.id)?;
                changes.extend(refresh(alerts, &held, Some(owner), now, local, ids));
            }
        }
    }
    Ok(changes)
}

/// The changes that make the store hold `held`'s alerts of the [`HORIZON`] after `now`, where it holds `alerts`: an
/// alert already held at its time stays, one whose time came stays for the scheduler, the others go, and the missing
/// ones come with their schedule entries. An event whose text cannot be read alerts nothing.
fn refresh(alerts: Vec<Alert>, held: &CalendarEvent, owner: Option<&str>, now: Timestamp, local: &dyn Offsets, ids: &mut Ids) -> Vec<Change> {
    let mut wanted = read(held).map(|calendar| upcoming(&calendar, now, local, owner)).unwrap_or_default();
    let mut changes = Vec::new();
    for alert in alerts {
        if alert.at <= now {
            continue;
        }
        match wanted.iter().position(|at| *at == alert.at) {
            Some(index) => {
                wanted.remove(index);
            }
            None => changes.push(Change::RemoveAlert(alert.id)),
        }
    }
    for at in wanted {
        let id = AlertId::new(ids.alert);
        ids.alert = ids.alert.saturating_add(1);
        let entry = ScheduleId::new(ids.schedule);
        ids.schedule = ids.schedule.saturating_add(1);
        changes.push(Change::PutAlert(Alert { id, event: held.id, at, dismissed: false }));
        changes.push(Change::PutScheduled(Scheduled { id: entry, due: at, what: Due::Alert(id) }));
    }
    changes
}

#[cfg(test)]
#[path = "alert_tests.rs"]
mod tests;
