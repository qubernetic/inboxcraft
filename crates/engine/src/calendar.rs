// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendars: what the CalDAV worker reports applied to the store, the events the store keeps read into
//! occurrences, new and edited events written, and the repeat presets.
//!
//! The store keeps each event as the iCalendar text the server sent (`inboxcraft_model::CalendarEvent`); it is read
//! here whenever it is shown or edited, with `inboxcraft_ical`, so nothing of it is lost on the way back. An
//! occurrence names its moments in UTC and on the computer's wall clock, its first and last day, and the event's own
//! zone when the zone's clock shows another time than the computer's. A calendar and an event keep their ids by their
//! URLs from one sync to the next; an event deleted here whose delete is still held for the undo window stays away
//! when a sync brings it again.

use std::collections::{BTreeMap, BTreeSet};

use inboxcraft_ical::{Attendee, Calendar, Class, Date, Event, Frequency, Occurrence, PartStat, Rule, WeekdayNum, When};
use inboxcraft_model::{
    AccountId, Calendar as Held, CalendarEvent, CalendarEventId, CalendarId, CalendarJob, CalendarListing, CalendarReport, EmailAddress,
    FetchedEvent, JobKind, KnownEvent,
};
use inboxcraft_store::{Change, Storage};
use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp, Weekday};
use serde_json::{Value, json};

use crate::EngineError;
use crate::sync::Plan;

/// A repeat the owner can pick with one click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Repeat {
    /// Once.
    None,
    /// Every day.
    Daily,
    /// Monday to Friday.
    Weekdays,
    /// Every week on the day it starts.
    Weekly,
    /// Every month on the day it starts.
    Monthly,
    /// Every year on the day it starts.
    Yearly,
}

impl Repeat {
    /// Every preset, in the order a picker shows them.
    pub(crate) const ALL: [Self; 6] = [Self::None, Self::Daily, Self::Weekdays, Self::Weekly, Self::Monthly, Self::Yearly];

    /// The preset's name in commands.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Daily => "daily",
            Self::Weekdays => "weekdays",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    /// The preset `name` names.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|repeat| repeat.name() == name)
    }

    /// The rule it writes; `None` for an event that does not repeat.
    pub(crate) fn rule(self) -> Option<Rule> {
        let frequency = match self {
            Self::None => return None,
            Self::Daily => Frequency::Daily,
            Self::Weekdays | Self::Weekly => Frequency::Weekly,
            Self::Monthly => Frequency::Monthly,
            Self::Yearly => Frequency::Yearly,
        };
        let mut rule = Rule::new(frequency);
        if self == Self::Weekdays {
            rule.by_day = [Weekday::Monday, Weekday::Tuesday, Weekday::Wednesday, Weekday::Thursday, Weekday::Friday]
                .into_iter()
                .map(|weekday| WeekdayNum { ordinal: 0, weekday })
                .collect();
        }
        Some(rule)
    }

    /// The preset `event` repeats by, however it ends: `None` when it repeats in another way.
    pub(crate) fn of(event: &Event) -> Option<Self> {
        match (event.rules.as_slice(), event.dates.is_empty()) {
            ([], true) => Some(Self::None),
            ([rule], true) => {
                let open = Rule { end: inboxcraft_ical::End::Never, ..rule.clone() };
                Self::ALL.into_iter().find(|repeat| repeat.rule().as_ref() == Some(&open))
            }
            _ => None,
        }
    }
}

/// The most calendars of one account kept from a discovery; a server listing more has the rest left out.
pub(crate) const MAX_CALENDARS: usize = 100;

/// The longest name taken from a calendar's URL when the server gives it none, in characters.
const MAX_URL_NAME_CHARS: usize = 64;

/// The job that finds an account's calendars at its CalDAV address `url`. The address goes with the job: a worker
/// that still holds the account as it was before the address changed refuses it rather than ask the old server.
pub(crate) fn discover(url: &str) -> JobKind {
    JobKind::Calendar(CalendarJob::Discover { url: Some(url.to_owned()) })
}

/// The origin of an `https` URL: its host in lower case and its port; `None` for another scheme.
pub(crate) fn origin(url: &str) -> Option<(String, u16)> {
    let rest = url.get(..8).filter(|scheme| scheme.eq_ignore_ascii_case("https://")).and_then(|_| url.get(8..))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default().to_ascii_lowercase();
    Some(match authority.rsplit_once(':') {
        // Not the colon of an IPv6 address in brackets.
        Some((host, port)) if !port.contains(']') => (host.to_owned(), port.parse().unwrap_or(443)),
        _ => (authority, 443),
    })
}

/// Whether `url` names something inside the collection `href`: below it, on its origin, without a `.` or `..` step.
fn inside(href: &str, url: &str) -> bool {
    let base = href.trim_end_matches('/');
    let Some(rest) = url.get(base.len()..).filter(|_| url.get(..base.len()) == Some(base)) else { return false };
    let Some(path) = rest.strip_prefix('/') else { return false };
    !path.is_empty() && !path.contains(['?', '#']) && path.split('/').all(|step| step != "." && step != ".." && !step.is_empty())
}

/// The job that brings `calendar` in step from `token`, naming every event held with its entity tag. An event
/// without one (written, its tag not known yet) is left out, so the server sends it again.
pub(crate) fn sync_job(store: &dyn Storage, calendar: &Held, token: Option<String>) -> Result<JobKind, EngineError> {
    let known = store.calendar_events(calendar.id)?.into_iter().filter_map(|event| Some(KnownEvent { etag: event.etag?, url: event.href })).collect();
    Ok(JobKind::Calendar(CalendarJob::Sync { calendar: calendar.id, url: calendar.href.clone(), token, known }))
}

/// Applies what a calendar job of `account` reports; `false` when it changes nothing or names a calendar or event
/// the account does not hold.
pub(crate) fn report(store: &dyn Storage, plan: &mut Plan, account: AccountId, report: &CalendarReport) -> Result<bool, EngineError> {
    match report {
        CalendarReport::Found { home, calendars } => {
            // From the account's current server only: a late answer of a server it left, or after its address was
            // cleared, would put the old calendars back and remove the new ones.
            let Some(own) = store.accounts()?.into_iter().find(|held| held.id() == account).and_then(|held| held.caldav().and_then(origin)) else {
                return Ok(false);
            };
            let elsewhere = |url: &str| origin(url).as_ref() != Some(&own);
            if (!home.is_empty() && elsewhere(home)) || calendars.iter().any(|listing| elsewhere(&listing.url)) {
                return Ok(false);
            }
            found(store, plan, account, calendars)
        }
        CalendarReport::Synced { calendar, changed, removed, token, .. } => {
            let Some(calendar) = calendar_of(store, account, *calendar)? else { return Ok(false) };
            synced(store, plan, &calendar, changed, removed)?;
            plan.change(Change::SetSyncToken(calendar.id, token.clone()));
            Ok(true)
        }
        CalendarReport::Written { calendar, event, etag } => {
            let Some((calendar, held)) = event_of(store, account, *calendar, *event)? else { return Ok(false) };
            match etag {
                Some(etag) => plan.change(Change::PutCalendarEvent(CalendarEvent { etag: Some(etag.clone()), ..held })),
                // The next sync brings the tag; until then the event is not named as known.
                None => plan.job(account, sync_job(store, &calendar, store.sync_token(calendar.id)?)?),
            }
            Ok(true)
        }
        CalendarReport::Deleted { calendar, event } => {
            let Some((_, held)) = event_of(store, account, *calendar, *event)? else { return Ok(false) };
            plan.change(Change::RemoveCalendarEvent(held.id));
            Ok(true)
        }
        CalendarReport::Conflict { calendar, .. } => {
            // Changed elsewhere: compare everything, so the server's copy replaces ours.
            let Some(calendar) = calendar_of(store, account, *calendar)? else { return Ok(false) };
            plan.change(Change::SetSyncToken(calendar.id, None));
            plan.job(account, sync_job(store, &calendar, None)?);
            Ok(true)
        }
        CalendarReport::Failed { .. } => Ok(false),
    }
}

/// The calendars a discovery found: each one that may hold events kept under its id by its URL or added under a new
/// one, renamed as the server names it, and synced; the account's calendars it did not list removed.
fn found(store: &dyn Storage, plan: &mut Plan, account: AccountId, calendars: &[CalendarListing]) -> Result<bool, EngineError> {
    let held = store.calendars(account)?;
    let mut seen = BTreeSet::new();
    let mut kept = BTreeSet::new();
    for listing in calendars.iter().filter(|listing| listing.events) {
        if seen.len() >= MAX_CALENDARS {
            break;
        }
        if !seen.insert(listing.url.as_str()) {
            continue;
        }
        let (id, token) = match held.iter().find(|calendar| calendar.href == listing.url) {
            Some(calendar) => (calendar.id, store.sync_token(calendar.id)?),
            None => {
                let id = CalendarId::new(plan.state.next_calendar);
                plan.state.next_calendar = plan.state.next_calendar.saturating_add(1);
                (id, None)
            }
        };
        kept.insert(id);
        let name = listing.name.clone().unwrap_or_else(|| url_name(&listing.url));
        let calendar = Held { id, account, href: listing.url.clone(), name, colour: listing.colour.clone() };
        plan.job(account, sync_job(store, &calendar, token)?);
        plan.change(Change::PutCalendar(calendar));
    }
    for calendar in held.iter().filter(|calendar| !kept.contains(&calendar.id)) {
        plan.change(Change::RemoveCalendar(calendar.id));
    }
    Ok(true)
}

/// The events a sync brought to `calendar`: each kept under its id by its URL or added under a new one; the ones
/// gone removed.
fn synced(store: &dyn Storage, plan: &mut Plan, calendar: &Held, changed: &[FetchedEvent], removed: &[String]) -> Result<(), EngineError> {
    let held = store.calendar_events(calendar.id)?;
    // An event deleted here whose delete waits out its undo window is still on the server: it stays away.
    let deleting: BTreeSet<String> = plan
        .state
        .held
        .iter()
        .filter_map(|(_, job)| match &job.kind {
            JobKind::Calendar(CalendarJob::Delete { calendar: of, url, .. }) if *of == calendar.id => Some(url.clone()),
            _ => None,
        })
        .collect();
    let mut ids: BTreeMap<&str, CalendarEventId> = held.iter().map(|event| (event.href.as_str(), event.id)).collect();
    // An event with a write of the owner's out keeps the owner's copy: the server's is older.
    let writing: BTreeSet<&str> =
        changed.iter().filter(|fetched| plan.state.writes.busy(calendar.id, &fetched.url)).map(|fetched| fetched.url.as_str()).collect();
    // An event the server names outside the calendar's collection is not the calendar's.
    let mine = |url: &str| inside(&calendar.href, url);
    for fetched in changed.iter().filter(|fetched| mine(&fetched.url) && !deleting.contains(&fetched.url) && !writing.contains(fetched.url.as_str()))
    {
        let id = match ids.get(fetched.url.as_str()) {
            Some(id) => *id,
            None => {
                let id = CalendarEventId::new(plan.state.next_event);
                plan.state.next_event = plan.state.next_event.saturating_add(1);
                ids.insert(&fetched.url, id);
                id
            }
        };
        let event = CalendarEvent { id, calendar: calendar.id, href: fetched.url.clone(), etag: fetched.etag.clone(), ical: fetched.ical.clone() };
        plan.change(Change::PutCalendarEvent(event));
    }
    for url in removed.iter().filter(|url| mine(url)) {
        if let Some(event) = held.iter().find(|event| event.href == **url) {
            plan.change(Change::RemoveCalendarEvent(event.id));
        }
    }
    Ok(())
}

/// Calendar `id` of `account`, if the account holds it.
pub(crate) fn calendar_of(store: &dyn Storage, account: AccountId, id: CalendarId) -> Result<Option<Held>, EngineError> {
    Ok(store.calendars(account)?.into_iter().find(|calendar| calendar.id == id))
}

/// Event `id` of calendar `calendar` of `account`, with its calendar, if the account holds them.
fn event_of(
    store: &dyn Storage,
    account: AccountId,
    calendar: CalendarId,
    id: CalendarEventId,
) -> Result<Option<(Held, CalendarEvent)>, EngineError> {
    let Some(calendar) = calendar_of(store, account, calendar)? else { return Ok(None) };
    Ok(store.calendar_event(id)?.filter(|event| event.calendar == calendar.id).map(|event| (calendar, event)))
}

/// A name for a calendar the server gives none: the last part of its URL, or `Calendar`.
fn url_name(url: &str) -> String {
    let segment = url.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
    let name: String = segment.chars().filter(|c| !c.is_control()).take(MAX_URL_NAME_CHARS).collect();
    if name.is_empty() || segment.contains(':') { "Calendar".to_owned() } else { name }
}

/// How long a deleted event waits before the server is asked to delete it: as long as the toast with Undo shows.
pub(crate) const UNDO_WINDOW: std::time::Duration = std::time::Duration::from_secs(6);

/// The product InboxCraft writes into a calendar object it makes.
const PRODUCT: &str = "-//Qubernetic//InboxCraft//EN";

/// The longest event InboxCraft writes, in days.
const MAX_DAYS: i64 = 400;

/// Calendar `id` of any account.
pub(crate) fn calendar_by_id(store: &dyn Storage, id: CalendarId) -> Result<Option<Held>, EngineError> {
    for account in store.accounts()? {
        if let Some(calendar) = calendar_of(store, account.id(), id)? {
            return Ok(Some(calendar));
        }
    }
    Ok(None)
}

/// When a new or edited event takes place, as the owner gave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Times {
    /// Whole days, the first and the last.
    Days(Date, Date),
    /// From one moment to a later one.
    Moments(Timestamp, Timestamp),
}

impl Times {
    /// The times, checked: the end not before the start (an event without length is a moment in time), at most
    /// [`MAX_DAYS`] long.
    pub(crate) fn checked(self) -> Result<Self, &'static str> {
        let days = match self {
            Self::Days(first, last) => days_between(first, last).filter(|days| *days >= 0).ok_or("the last day is before the first")?,
            Self::Moments(start, end) => {
                let length = end.duration_since(start).ok_or("the end is before the start")?;
                i64::try_from(length.as_secs() / 86_400).unwrap_or(i64::MAX)
            }
        };
        if days >= MAX_DAYS {
            return Err("the event is longer than 400 days");
        }
        Ok(self)
    }
}

/// Days from `first` to `last`, negative when `last` is earlier.
fn days_between(first: Date, last: Date) -> Option<i64> {
    let at = |date: Date| date.midnight().to_timestamp(Offset::UTC).map(Timestamp::unix_seconds);
    Some(at(last)?.checked_sub(at(first)?)? / 86_400)
}

/// A day from an ISO 8601 date (`2026-11-20`).
pub(crate) fn parse_day(text: &str) -> Option<Date> {
    if text.len() != 10 {
        return None;
    }
    let civil = crate::params::parse_date(text)?.to_civil(Offset::UTC)?;
    Date::new(civil.year(), civil.month(), civil.day())
}

/// A moment from an ISO 8601 date and time with its offset (`2026-11-20T09:30:00+01:00`).
pub(crate) fn parse_moment(text: &str) -> Option<Timestamp> {
    if text.len() <= 10 {
        return None;
    }
    crate::params::parse_date(text)
}

/// What an event says beside its times, as the owner gave it; `None` leaves it as it is.
#[derive(Debug, Default)]
pub(crate) struct Details {
    pub(crate) summary: Option<String>,
    /// Empty takes it off.
    pub(crate) location: Option<String>,
    /// Empty takes it off.
    pub(crate) description: Option<String>,
    pub(crate) repeat: Option<Repeat>,
    pub(crate) busy: Option<bool>,
    pub(crate) private: Option<bool>,
    /// The guests' addresses, each once.
    pub(crate) guests: Option<Vec<EmailAddress>>,
}

impl Details {
    /// Whether it changes nothing.
    pub(crate) fn is_empty(&self) -> bool {
        self.summary.is_none()
            && self.location.is_none()
            && self.description.is_none()
            && self.repeat.is_none()
            && self.busy.is_none()
            && self.private.is_none()
            && self.guests.is_none()
    }

    /// Writes them into `event`; `organizer` organises it once it has guests.
    fn apply(self, event: &mut Event, organizer: &EmailAddress) {
        let text = |value: String| Some(value).filter(|value| !value.is_empty());
        if let Some(summary) = self.summary {
            event.summary = Some(summary);
        }
        if let Some(location) = self.location {
            event.location = text(location);
        }
        if let Some(description) = self.description {
            event.description = text(description);
        }
        if let Some(repeat) = self.repeat {
            event.rules = repeat.rule().into_iter().collect();
            event.dates.clear();
        }
        if let Some(busy) = self.busy {
            event.free = !busy;
        }
        if let Some(private) = self.private {
            event.class = private.then_some(Class::Private);
        }
        if let Some(guests) = self.guests {
            event.attendees = guests
                .iter()
                .map(|address| {
                    event
                        .attendees
                        .iter()
                        .find(|held| address_of(held).eq_ignore_ascii_case(address.as_str()))
                        .cloned()
                        .unwrap_or_else(|| Attendee { rsvp: true, ..Attendee::new(&format!("mailto:{}", address.as_str())) })
                })
                .collect();
            if !event.attendees.is_empty() && event.organizer.is_none() {
                event.organizer = Some(Attendee::new(&format!("mailto:{}", organizer.as_str())));
            }
        }
    }
}

/// A new event `uid` of `times` with `details`, written by `organizer` at `now`, as a calendar object: whole days as
/// dates, a single event's moments in UTC, a repeating event's on the computer's wall clock `local` so it keeps its
/// time of day across a summer-time change.
pub(crate) fn create(uid: &str, times: Times, details: Details, organizer: &EmailAddress, now: Timestamp, local: &dyn Offsets) -> Option<String> {
    let repeating = details.repeat.is_some_and(|repeat| repeat != Repeat::None);
    let (start, end) = placed(times, if repeating { &Form::Floating } else { &Form::Utc }, local, local)?;
    let mut event = Event::new(uid, start);
    event.end = Some(end);
    event.stamp = Some(now);
    details.apply(&mut event, organizer);
    unscheduled(&mut event);
    let mut calendar = Calendar::new(PRODUCT);
    calendar.events.push(event);
    calendar.write().ok()
}

/// How an edited event's times were given.
pub(crate) struct Edit {
    /// The new start: a day or a moment, as the event is or becomes all-day.
    pub(crate) start: Option<String>,
    /// The new end: the last day, or a moment.
    pub(crate) end: Option<String>,
    /// All-day from now on, or no longer.
    pub(crate) all_day: Option<bool>,
}

/// Why an edit cannot be made.
pub(crate) enum EditError {
    /// The event's text cannot be read.
    Unreadable,
    /// What the owner gave cannot be used, and why.
    Bad(&'static str),
}

/// `held` edited by `change` and `details` at `now`: its times kept in their own form (its zone, the computer's wall
/// clock, or UTC; a single moment in UTC that starts repeating moves to the wall clock), and `SEQUENCE` counted on
/// when the guests are told (`notify`). The new text and the number of guests.
pub(crate) fn edit(
    held: &CalendarEvent,
    change: &Edit,
    details: Details,
    notify: bool,
    organizer: &EmailAddress,
    now: Timestamp,
    local: &dyn Offsets,
) -> Result<(String, usize), EditError> {
    let mut calendar = read(held).ok_or(EditError::Unreadable)?;
    let index = calendar.events.iter().position(|event| event.recurrence_id.is_none()).ok_or(EditError::Unreadable)?;
    let event = calendar.events.get(index).ok_or(EditError::Unreadable)?;
    let times = times(&calendar, event, change, local)?;
    let repeating = match details.repeat {
        Some(repeat) => repeat != Repeat::None,
        None => !event.rules.is_empty() || !event.dates.is_empty(),
    };
    let form = match &event.start {
        When::Zoned(_, tzid) => Form::Zoned(tzid.clone()),
        When::Floating(_) => Form::Floating,
        When::Utc(_) | When::Date(_) if repeating => Form::Floating,
        When::Utc(_) | When::Date(_) => Form::Utc,
    };
    let zone = calendar.zones.offsets(&event.start, local);
    let (start, end) = placed(times, &form, zone, local).ok_or(EditError::Bad("the times cannot be written"))?;
    let event = calendar.events.get_mut(index).ok_or(EditError::Unreadable)?;
    event.start = start;
    event.end = Some(end);
    event.duration = None;
    event.stamp = Some(now);
    details.apply(event, organizer);
    unscheduled(event);
    let guests = event.attendees.len();
    if notify && guests > 0 {
        event.sequence = event.sequence.saturating_add(1);
    }
    let text = calendar.write().map_err(|_| EditError::Bad("the event cannot be written"))?;
    Ok((text, guests))
}

/// The edited event's times: what `change` gives, the rest as they were (an end that keeps the length), checked.
fn times(calendar: &Calendar, event: &Event, change: &Edit, local: &dyn Offsets) -> Result<Times, EditError> {
    let was_all_day = event.all_day();
    let all_day = change.all_day.unwrap_or(was_all_day);
    let switching = all_day != was_all_day;
    if switching && change.start.is_none() {
        return Err(EditError::Bad("start is needed to make an event all-day or no longer"));
    }
    let start_at = calendar.zones.resolve(&event.start, local).ok_or(EditError::Unreadable)?;
    let end_at = match (&event.end, event.duration) {
        (Some(end), _) => calendar.zones.resolve(end, local),
        (None, Some(span)) => after(start_at, span),
        (None, None) if was_all_day => start_at.checked_add(std::time::Duration::from_secs(86_400)),
        (None, None) => Some(start_at),
    }
    .filter(|end| *end >= start_at)
    .unwrap_or(start_at);
    let times = if all_day {
        let first = match &change.start {
            Some(text) => parse_day(text).ok_or(EditError::Bad("start is no ISO 8601 day"))?,
            None => local.civil(start_at).map(Date::of).ok_or(EditError::Unreadable)?,
        };
        let last = match &change.end {
            Some(text) => parse_day(text).ok_or(EditError::Bad("end is no ISO 8601 day"))?,
            None if switching => first,
            None => {
                let days = end_at.duration_since(start_at).map_or(1, |length| length.as_secs().div_ceil(86_400)).max(1);
                first.add_days(i64::try_from(days).unwrap_or(1).saturating_sub(1)).ok_or(EditError::Bad("the last day cannot be written"))?
            }
        };
        Times::Days(first, last)
    } else {
        let start = match &change.start {
            Some(text) => parse_moment(text).ok_or(EditError::Bad("start is no ISO 8601 date and time"))?,
            None => start_at,
        };
        let length = if switching { std::time::Duration::from_secs(3600) } else { end_at.duration_since(start_at).unwrap_or_default() };
        let end = match &change.end {
            Some(text) => parse_moment(text).ok_or(EditError::Bad("end is no ISO 8601 date and time"))?,
            None => start.checked_add(length).ok_or(EditError::Bad("the end cannot be written"))?,
        };
        Times::Moments(start, end)
    };
    times.checked().map_err(EditError::Bad)
}

/// `span` after `at`, days as 24 hours.
fn after(at: Timestamp, span: inboxcraft_ical::Span) -> Option<Timestamp> {
    let seconds = span.days().checked_mul(86_400)?.checked_add(span.seconds())?;
    Timestamp::from_unix(at.unix_seconds().checked_add(seconds)?, 0)
}

/// The form an event's wall-clock times take.
enum Form {
    /// Moments in UTC.
    Utc,
    /// The computer's wall clock.
    Floating,
    /// The wall clock of a zone.
    Zoned(String),
}

/// `times` as an event's start and its end (whole days: the day after the last), in `form`; a zone's times on the
/// clock of `zone`, floating ones on `local`.
fn placed(times: Times, form: &Form, zone: &dyn Offsets, local: &dyn Offsets) -> Option<(When, When)> {
    match times {
        Times::Days(first, last) => Some((When::Date(first), When::Date(last.add_days(1)?))),
        Times::Moments(start, end) => {
            let at = |moment: Timestamp| match form {
                Form::Utc => Some(When::Utc(moment)),
                Form::Floating => local.civil(moment).map(When::Floating),
                Form::Zoned(tzid) => zone.civil(moment).map(|civil| When::Zoned(civil, tzid.clone())),
            };
            Some((at(start)?, at(end)?))
        }
    }
}

/// `event` marked so that a CalDAV server that schedules on its own (RFC 6638) mails nobody of it: the organiser and
/// every guest carry `SCHEDULE-AGENT=CLIENT`, the client's task, until InboxCraft sends the invitations itself.
fn unscheduled(event: &mut Event) {
    for person in event.attendees.iter_mut().chain(event.organizer.as_mut()) {
        person.set_param("SCHEDULE-AGENT", "CLIENT");
    }
}

/// An attendee's address without its `mailto:`.
fn address_of(attendee: &Attendee) -> &str {
    attendee
        .address
        .get(..7)
        .filter(|scheme| scheme.eq_ignore_ascii_case("mailto:"))
        .map_or(&*attendee.address, |_| attendee.address.get(7..).unwrap_or_default())
}

/// The calendar object `held` keeps, or `None` when its text cannot be read.
pub(crate) fn read(held: &CalendarEvent) -> Option<Calendar> {
    Calendar::parse(held.ical.as_bytes()).ok()
}

/// The series of a calendar object: its first event without a `RECURRENCE-ID`.
pub(crate) fn master(calendar: &Calendar) -> Option<&Event> {
    calendar.events.iter().find(|event| event.recurrence_id.is_none())
}

/// One occurrence of `held` (read into `calendar`) as `event.list` shows it, on the computer's clock `local`.
pub(crate) fn row(held: &CalendarEvent, calendar: &Calendar, occurrence: &Occurrence, local: &dyn Offsets) -> Value {
    let Some(event) = calendar.events.get(occurrence.event) else { return Value::Null };
    let series = master(calendar).filter(|master| master.uid == event.uid).unwrap_or(event);
    let repeat = match Repeat::of(series) {
        Some(Repeat::None) => Value::Null,
        Some(repeat) => json!(repeat.name()),
        None => json!("custom"),
    };
    let (start, end) = (occurrence.start, occurrence.end);
    // The last day it covers: an end at midnight belongs to the day before.
    let last = end.checked_sub(std::time::Duration::from_secs(1)).filter(|last| *last >= start).unwrap_or(start);
    json!({
        "id": held.id.get(),
        "calendar": held.calendar.get(),
        "uid": event.uid,
        "summary": event.summary,
        "location": event.location,
        "description": event.description,
        "start": crate::json::date(start),
        "end": crate::json::date(end),
        "local_start": wall(start, local),
        "local_end": wall(end, local),
        "start_date": day(start, local),
        "end_date": day(last, local),
        "all_day": event.all_day(),
        "busy": !event.free,
        "private": matches!(event.class, Some(Class::Private | Class::Confidential)),
        "repeat": repeat,
        "recurrence_id": occurrence.recurrence_id.as_ref().map(when),
        "zone": zone(calendar, event, start, end, local),
        "guests": event.attendees.iter().map(guest).collect::<Vec<_>>(),
    })
}

/// The event's own zone with its wall-clock times, when its clock shows another time than the computer's at the
/// start or the end; `null` for a moment in UTC, a whole day, a floating time and a zone the object does not describe.
fn zone(calendar: &Calendar, event: &Event, start: Timestamp, end: Timestamp, local: &dyn Offsets) -> Value {
    let When::Zoned(_, tzid) = &event.start else { return Value::Null };
    let Some(zone) = calendar.zones.get(tzid) else { return Value::Null };
    let differs = [start, end].into_iter().any(|at| zone.offset_at(at) != local.offset_at(at));
    if !differs {
        return Value::Null;
    }
    json!({ "name": tzid, "start": wall(start, zone), "end": wall(end, zone) })
}

/// A guest: the address without its `mailto:`, the name and the answer.
fn guest(attendee: &Attendee) -> Value {
    let address = address_of(attendee);
    let status = match &attendee.status {
        PartStat::NeedsAction => "needs_action",
        PartStat::Accepted => "accepted",
        PartStat::Declined => "declined",
        PartStat::Tentative => "tentative",
        PartStat::Delegated => "delegated",
        PartStat::Other(_) => "other",
    };
    json!({ "address": address, "name": attendee.name, "status": status })
}

/// `at` on the wall clock of `offsets`, with the offset: `2026-11-09T09:30:00+01:00`.
pub(crate) fn wall(at: Timestamp, offsets: &dyn Offsets) -> Value {
    let offset = offsets.offset_at(at);
    let Some(civil) = at.to_civil(offset) else { return Value::Null };
    let minutes = offset.minutes();
    let sign = if minutes < 0 { '-' } else { '+' };
    let (hours, minutes) = (minutes.unsigned_abs() / 60, minutes.unsigned_abs() % 60);
    Value::String(format!("{}{sign}{hours:02}:{minutes:02}", civil_text(civil)))
}

/// The day `at` falls on, on the wall clock of `offsets`: `2026-11-09`.
fn day(at: Timestamp, offsets: &dyn Offsets) -> Value {
    offsets.civil(at).map_or(Value::Null, |civil| Value::String(date_text(civil.year(), civil.month(), civil.day())))
}

/// A `RECURRENCE-ID` in the event's own terms: a day, a wall-clock time, or a moment in UTC.
fn when(when: &When) -> String {
    match when {
        When::Date(date) => date_text(date.year(), date.month(), date.day()),
        When::Floating(civil) | When::Zoned(civil, _) => civil_text(*civil),
        When::Utc(at) => at.to_civil(Offset::UTC).map(|civil| format!("{}Z", civil_text(civil))).unwrap_or_default(),
    }
}

fn civil_text(civil: CivilDateTime) -> String {
    format!("{}T{:02}:{:02}:{:02}", date_text(civil.year(), civil.month(), civil.day()), civil.hour(), civil.minute(), civil.second())
}

fn date_text(year: i32, month: u8, day: u8) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;
