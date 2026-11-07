// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A calendar object (`VCALENDAR`): its zones and events, and the occurrences of the events in a span of time.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::sync::atomic::{self, AtomicUsize};
use std::time::Duration;

use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp};

use crate::value::{DAY_SECS, civil_from_seconds, civil_seconds};
use crate::zone::{ZoneClock, is_utc};
use crate::{Component, Date, End, Event, IcalError, Property, Rule, Span, TimeZone, When, Zones, parse};

/// The most occurrences [`Calendar::occurrences`] gives for one span; it holds no more while it looks.
pub const MAX_OCCURRENCES: usize = 10_000;

/// The most work [`Calendar::occurrences`] does for one span, all events together: the work of their rules and of
/// their zones' rules (see [`crate::Recurrence::work`]), and ten for each event, date and instance it looks at.
pub const MAX_STEPS: usize = 2_000_000;

/// What looking at one event, date or instance costs out of [`MAX_STEPS`]: it resolves times in a zone.
const LOOK_STEPS: usize = 10;

/// A calendar object: what one `.ics` file or one CalDAV resource holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calendar {
    /// The `PRODID`: the program that wrote it.
    pub product: String,
    /// The iTIP `METHOD` (`REQUEST`, `REPLY`, `CANCEL` ...), for an invitation.
    pub method: Option<String>,
    /// The events that can be read.
    pub events: Vec<Event>,
    /// The time zones that can be read.
    pub zones: Zones,
    /// The calendar's properties other than `VERSION`, `PRODID` and `METHOD`, kept to be written back.
    pub other: Vec<Property>,
    /// The components it does not read (`VTODO`, `VJOURNAL` ...) and the events and zones that cannot be read,
    /// kept to be written back.
    pub components: Vec<Component>,
}

/// One time an event takes place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// The event's index in [`Calendar::events`].
    pub event: usize,
    /// When it starts.
    pub start: Timestamp,
    /// When it ends, never before the start.
    pub end: Timestamp,
    /// Which instance of a recurring event this is, in the terms of its `DTSTART` (for a changed instance, its
    /// `RECURRENCE-ID`); `None` for an event that does not recur.
    pub recurrence_id: Option<When>,
}

/// The occurrences in a span of time, by start.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expansion {
    /// The occurrences.
    pub occurrences: Vec<Occurrence>,
    /// Whether a limit left some out: more than [`MAX_OCCURRENCES`], more than [`MAX_STEPS`] of work, or a rule
    /// that ran into a limit of its own ([`crate::Recurrence::truncated`]).
    pub truncated: bool,
}

/// How long each occurrence of an event lasts.
#[derive(Debug, Clone, Copy)]
enum Length {
    /// Days on the wall clock, then seconds: a `DURATION`, or the days between two dates.
    Nominal(Span),
    /// Seconds: between a `DTSTART` and a `DTEND` with a time.
    Exact(i64),
}

impl Length {
    /// A safe guess of the most seconds it can be.
    fn most_secs(self) -> i64 {
        match self {
            Self::Nominal(span) => span.days().saturating_add(1).saturating_mul(DAY_SECS).saturating_add(span.seconds()),
            Self::Exact(secs) => secs,
        }
    }
}

impl Calendar {
    /// An empty calendar written by `product`.
    #[must_use]
    pub fn new(product: &str) -> Self {
        Self { product: product.to_owned(), method: None, events: Vec::new(), zones: Zones::new(), other: Vec::new(), components: Vec::new() }
    }

    /// Reads a calendar object.
    ///
    /// # Errors
    ///
    /// What [`parse`] refuses, and [`IcalError::NotACalendar`] when the component at the top is no `VCALENDAR`.
    /// An event or a zone that cannot be read is no error: it is kept in [`Calendar::components`].
    pub fn parse(input: &[u8]) -> Result<Self, IcalError> {
        Self::from_component(&parse(input)?)
    }

    /// The calendar a `VCALENDAR` component describes.
    ///
    /// # Errors
    ///
    /// [`IcalError::NotACalendar`] when it is no `VCALENDAR`.
    pub fn from_component(component: &Component) -> Result<Self, IcalError> {
        if component.name() != "VCALENDAR" {
            return Err(IcalError::NotACalendar);
        }
        let mut calendar = Self::new(component.property("PRODID").map_or("", Property::value));
        calendar.method = component.property("METHOD").map(|method| method.value().to_owned());
        calendar.other =
            component.properties().iter().filter(|property| !matches!(property.name(), "VERSION" | "PRODID" | "METHOD")).cloned().collect();
        for inner in component.components() {
            match inner.name() {
                "VTIMEZONE" => match TimeZone::from_component(inner) {
                    Ok(zone) if calendar.zones.get(zone.tzid()).is_none() => calendar.zones.push(zone),
                    _ => calendar.components.push(inner.clone()),
                },
                "VEVENT" => match Event::from_component(inner) {
                    Ok(event) => calendar.events.push(event),
                    Err(_) => calendar.components.push(inner.clone()),
                },
                _ => calendar.components.push(inner.clone()),
            }
        }
        Ok(calendar)
    }

    /// The `VCALENDAR` component: `VERSION:2.0`, the zones, the events, then what it keeps.
    #[must_use]
    pub fn to_component(&self) -> Component {
        let mut component =
            Component::new("VCALENDAR").with_property(Property::new("VERSION", "2.0")).with_property(Property::new("PRODID", self.product.clone()));
        if let Some(method) = &self.method {
            component.push_property(Property::new("METHOD", method.clone()));
        }
        let component = self.other.iter().cloned().fold(component, Component::with_property);
        let component = self.zones.all().iter().map(|zone| zone.component().clone()).fold(component, Component::with_component);
        let component = self.events.iter().map(Event::to_component).fold(component, Component::with_component);
        self.components.iter().cloned().fold(component, Component::with_component)
    }

    /// The calendar as iCalendar text.
    ///
    /// # Errors
    ///
    /// [`IcalError::BadName`] when a name it keeps cannot be written.
    pub fn write(&self) -> Result<String, IcalError> {
        self.to_component().write()
    }

    /// The occurrences of every event that overlap `from` to `to` (an occurrence without length counts when it
    /// starts in it), by start. Whole days and floating times are on the person's wall clock, `local`; a `TZID`
    /// without a `VTIMEZONE` too (see [`Zones::offsets`]).
    ///
    /// A recurring event's set is its `DTSTART`, its rules' occurrences (`UNTIL` and `COUNT` kept) and its
    /// `RDATE`s, less its `EXDATE`s and the instances a changed instance (same `UID`, a `RECURRENCE-ID`) takes the
    /// place of; changed instances count with their own times.
    ///
    /// The work is bounded by [`MAX_STEPS`] for the whole calendar, and at most [`MAX_OCCURRENCES`] are held, the
    /// earliest; when either limit is reached the expansion says so.
    #[must_use]
    pub fn occurrences(&self, from: Timestamp, to: Timestamp, local: &dyn Offsets) -> Expansion {
        self.occurrences_counted(from, to, local).0
    }

    /// [`Calendar::occurrences`] and the work it took.
    fn occurrences_counted(&self, from: Timestamp, to: Timestamp, local: &dyn Offsets) -> (Expansion, usize) {
        let zone_work = AtomicUsize::new(0);
        let view = View::new(&self.zones, (from, to), local, &zone_work);
        let mut found = Found::new(&zone_work);
        let mut changed: BTreeMap<&str, Vec<&When>> = BTreeMap::new();
        for event in &self.events {
            if let Some(id) = &event.recurrence_id {
                changed.entry(event.uid.as_str()).or_default().push(id);
            }
        }
        for (index, event) in self.events.iter().enumerate() {
            if !found.charge(LOOK_STEPS) {
                break;
            }
            if event.recurrence_id.is_some() {
                view.changed_instance(index, event, &mut found);
            } else {
                let gone = changed.get(event.uid.as_str()).map_or(&[][..], Vec::as_slice);
                view.expand(index, event, gone, &mut found);
            }
        }
        let used = found.used();
        (found.finish(), used)
    }
}

/// What an expansion looks through: its span, the calendar's zones (each worked out once) and the person's wall
/// clock.
struct View<'a> {
    span: (Timestamp, Timestamp),
    local: &'a dyn Offsets,
    clocks: Vec<ZoneClock<'a>>,
    by_tzid: BTreeMap<&'a str, usize>,
}

impl<'a> View<'a> {
    fn new(zones: &'a Zones, span: (Timestamp, Timestamp), local: &'a dyn Offsets, work: &'a AtomicUsize) -> Self {
        let mut by_tzid = BTreeMap::new();
        for (index, zone) in zones.all().iter().enumerate() {
            by_tzid.entry(zone.tzid()).or_insert(index);
        }
        Self { span, local, clocks: zones.all().iter().map(|zone| ZoneClock::new(zone, work)).collect(), by_tzid }
    }

    /// As [`Zones::offsets`].
    fn offsets(&self, when: &When) -> &dyn Offsets {
        match when {
            When::Utc(_) => &Offset::UTC,
            When::Zoned(_, tzid) => match self.by_tzid.get(tzid.as_str()).and_then(|index| self.clocks.get(*index)) {
                Some(clock) => clock,
                None if is_utc(tzid) => &Offset::UTC,
                None => self.local,
            },
            When::Date(_) | When::Floating(_) => self.local,
        }
    }

    /// As [`Zones::resolve`].
    fn resolve(&self, when: &When) -> Option<Timestamp> {
        match when {
            When::Utc(at) => Some(*at),
            When::Date(date) => self.local.resolve(date.midnight()),
            When::Floating(civil) | When::Zoned(civil, _) => self.offsets(when).resolve(*civil),
        }
    }

    fn changed_instance(&self, index: usize, event: &Event, found: &mut Found<'_>) {
        let Some(start) = self.resolve(&event.start) else { return };
        let length = self.length(event);
        let end = self.end_of(length, &event.start, start);
        if overlaps(start, end, self.span) {
            found.push(Occurrence { event: index, start, end, recurrence_id: event.recurrence_id.clone() });
        }
    }

    /// Adds the occurrences of the series `event`, less the instances in `gone` (the `RECURRENCE-ID`s of its changed
    /// instances) and its `EXDATE`s.
    fn expand(&self, index: usize, event: &Event, gone: &[&When], found: &mut Found<'_>) {
        let offsets = self.offsets(&event.start);
        let Some(first) = wall_clock(&event.start) else { return };
        let mut series = Series {
            index,
            event,
            length: self.length(event),
            recurring: !event.rules.is_empty() || !event.dates.is_empty(),
            gone_days: BTreeSet::new(),
            gone_at: BTreeSet::new(),
        };
        for gone in gone.iter().copied().chain(&event.exceptions) {
            if !found.charge(LOOK_STEPS) {
                return;
            }
            match gone {
                When::Date(date) => {
                    series.gone_days.insert(*date);
                }
                other => series.gone_at.extend(self.resolve(other)),
            }
        }
        let mut dates = BTreeSet::from([first]);
        for date in &event.dates {
            if !found.charge(LOOK_STEPS) {
                return;
            }
            dates.extend(match date {
                When::Date(day) => Some(day.midnight()),
                other => self.resolve(other).and_then(|at| offsets.civil(at)),
            });
        }
        for civil in dates {
            if !self.add(&series, civil, found) {
                return;
            }
        }
        let look_from = self
            .span
            .0
            .checked_sub(Duration::from_secs(series.length.most_secs().saturating_add(2 * DAY_SECS).unsigned_abs()))
            .unwrap_or(Timestamp::MIN);
        let look_from = offsets.civil(look_from);
        let look_to = offsets
            .civil(self.span.1)
            .and_then(|civil| civil_from_seconds(civil_seconds(civil).saturating_add(DAY_SECS)))
            .or_else(|| CivilDateTime::new(10_000, 12, 31, 23, 59, 59));
        for rule in &event.rules {
            let mut recurrence = rule.iter(first).with_budget(MAX_STEPS.saturating_sub(found.used()));
            if let Some(stop) = [until(rule, &event.start, offsets), look_to].into_iter().flatten().min() {
                recurrence = recurrence.stop_after(stop);
            }
            if let Some(look_from) = look_from {
                recurrence.skip_to(look_from);
            }
            let mut charged = 0;
            loop {
                let next = recurrence.next();
                if !found.charge(recurrence.work().saturating_sub(charged)) {
                    return;
                }
                charged = recurrence.work();
                let Some(civil) = next else { break };
                if look_from.is_none_or(|look_from| civil >= look_from) && !self.add(&series, civil, found) {
                    return;
                }
            }
            found.truncated |= recurrence.truncated();
        }
    }

    /// Adds the instance of `series` at the wall-clock time `civil` unless it is gone or outside the span; false
    /// when the work is spent.
    fn add(&self, series: &Series<'_>, civil: CivilDateTime, found: &mut Found<'_>) -> bool {
        if !found.charge(LOOK_STEPS) {
            return false;
        }
        let when = instance(&series.event.start, civil);
        let Some(start) = self.resolve(&when) else { return true };
        if series.gone_days.contains(&Date::of(civil)) || series.gone_at.contains(&start) {
            return true;
        }
        let end = self.end_of(series.length, &when, start);
        if overlaps(start, end, self.span) {
            found.push(Occurrence { event: series.index, start, end, recurrence_id: series.recurring.then_some(when) });
        }
        true
    }

    fn length(&self, event: &Event) -> Length {
        match (&event.start, &event.end, event.duration) {
            (When::Date(first), Some(When::Date(last)), _) => {
                let days = (civil_seconds(last.midnight()) - civil_seconds(first.midnight())) / DAY_SECS;
                Length::Nominal(Span::new(days.max(0), 0).unwrap_or_default())
            }
            (start, Some(end), _) => {
                let secs = self.resolve(end).zip(self.resolve(start)).map_or(0, |(end, start)| end.unix_seconds() - start.unix_seconds());
                Length::Exact(secs.max(0))
            }
            (_, None, Some(span)) if !span.is_negative() => Length::Nominal(span),
            (When::Date(_), None, None) => Length::Nominal(Span::new(1, 0).unwrap_or_default()),
            _ => Length::Exact(0),
        }
    }

    fn end_of(&self, length: Length, when: &When, start: Timestamp) -> Timestamp {
        let end = match length {
            Length::Exact(secs) => start.checked_add(Duration::from_secs(secs.unsigned_abs())),
            Length::Nominal(span) => wall_clock(when)
                .and_then(|civil| civil_from_seconds(civil_seconds(civil).saturating_add(span.days().saturating_mul(DAY_SECS))))
                .and_then(|civil| self.resolve(&instance(when, civil)))
                .and_then(|moved| moved.checked_add(Duration::from_secs(span.seconds().unsigned_abs()))),
        };
        end.unwrap_or(Timestamp::MAX).max(start)
    }
}

/// A recurring event while it is expanded.
struct Series<'e> {
    index: usize,
    event: &'e Event,
    length: Length,
    recurring: bool,
    /// The days of `EXDATE`s and `RECURRENCE-ID`s in the date form: every instance on them is gone.
    gone_days: BTreeSet<Date>,
    /// The moments of the other `EXDATE`s and `RECURRENCE-ID`s.
    gone_at: BTreeSet<Timestamp>,
}

/// An occurrence held, by start, then event, then the order it was found in.
struct Held {
    key: (Timestamp, usize, usize),
    occurrence: Occurrence,
}

impl PartialEq for Held {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Held {}

impl PartialOrd for Held {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Held {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

/// What an expansion has found so far: the earliest [`MAX_OCCURRENCES`] occurrences, and the work it took.
struct Found<'a> {
    held: BinaryHeap<Held>,
    /// The event and start of each occurrence held, so an instance two sources make counts once.
    seen: BTreeSet<(usize, Timestamp)>,
    made: usize,
    steps: usize,
    zone_work: &'a AtomicUsize,
    truncated: bool,
}

impl<'a> Found<'a> {
    fn new(zone_work: &'a AtomicUsize) -> Self {
        Self { held: BinaryHeap::new(), seen: BTreeSet::new(), made: 0, steps: 0, zone_work, truncated: false }
    }

    fn used(&self) -> usize {
        self.steps.saturating_add(self.zone_work.load(atomic::Ordering::Relaxed))
    }

    /// Counts `steps` more work; false, truncated, once the work is over [`MAX_STEPS`].
    fn charge(&mut self, steps: usize) -> bool {
        self.steps = self.steps.saturating_add(steps);
        if self.used() > MAX_STEPS {
            self.truncated = true;
            return false;
        }
        true
    }

    fn push(&mut self, occurrence: Occurrence) {
        if !self.seen.insert((occurrence.event, occurrence.start)) {
            return;
        }
        self.held.push(Held { key: (occurrence.start, occurrence.event, self.made), occurrence });
        self.made = self.made.saturating_add(1);
        if self.held.len() > MAX_OCCURRENCES {
            self.truncated = true;
            if let Some(latest) = self.held.pop() {
                self.seen.remove(&(latest.occurrence.event, latest.occurrence.start));
            }
        }
    }

    fn finish(self) -> Expansion {
        Expansion { occurrences: self.held.into_sorted_vec().into_iter().map(|held| held.occurrence).collect(), truncated: self.truncated }
    }
}

/// Whether an occurrence from `start` to `end` overlaps `span`; one without length counts when it starts in it.
fn overlaps(start: Timestamp, end: Timestamp, span: (Timestamp, Timestamp)) -> bool {
    start < span.1 && (end > span.0 || (end == start && start >= span.0))
}

/// The wall-clock time of a value (a `Z` value on UTC's).
fn wall_clock(when: &When) -> Option<CivilDateTime> {
    match when {
        When::Date(date) => Some(date.midnight()),
        When::Floating(civil) | When::Zoned(civil, _) => Some(*civil),
        When::Utc(at) => at.to_civil(Offset::UTC),
    }
}

/// The instance at the wall-clock time `civil`, in the form of `start`.
fn instance(start: &When, civil: CivilDateTime) -> When {
    match start {
        When::Date(_) => When::Date(Date::of(civil)),
        When::Floating(_) => When::Floating(civil),
        When::Zoned(_, tzid) => When::Zoned(civil, tzid.clone()),
        When::Utc(_) => civil.to_timestamp(Offset::UTC).map_or(When::Floating(civil), When::Utc),
    }
}

/// A rule's `UNTIL` on the wall clock of `start`: a date ends after that day.
fn until(rule: &Rule, start: &When, offsets: &dyn Offsets) -> Option<CivilDateTime> {
    match &rule.end {
        End::Until(When::Utc(at)) => offsets.civil(*at),
        End::Until(When::Date(date)) if start.is_date() => Some(date.midnight()),
        End::Until(When::Date(date)) => civil_from_seconds(civil_seconds(date.midnight()).saturating_add(DAY_SECS - 1)),
        End::Until(When::Floating(civil) | When::Zoned(civil, _)) => Some(*civil),
        _ => None,
    }
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;
