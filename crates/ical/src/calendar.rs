// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A calendar object (`VCALENDAR`): its zones and events, and the occurrences of the events in a span of time.

use std::collections::BTreeSet;
use std::time::Duration;

use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp};

use crate::value::{DAY_SECS, civil_from_seconds, civil_seconds};
use crate::{Component, Date, End, Event, IcalError, Property, Rule, Span, TimeZone, When, Zones, parse};

/// The most occurrences [`Calendar::occurrences`] gives for one span.
pub const MAX_OCCURRENCES: usize = 10_000;

/// The most occurrences of one rule [`Calendar::occurrences`] walks through, before the span included.
pub const MAX_STEPS: usize = 1_000_000;

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
    /// Whether a limit left some out: more than [`MAX_OCCURRENCES`], a rule that walked more than [`MAX_STEPS`]
    /// or ran into a limit of its own ([`crate::Recurrence::truncated`]).
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
    #[must_use]
    pub fn occurrences(&self, from: Timestamp, to: Timestamp, local: &dyn Offsets) -> Expansion {
        let mut expansion = Expansion::default();
        for (index, event) in self.events.iter().enumerate() {
            if event.recurrence_id.is_some() {
                self.changed_instance(index, event, (from, to), local, &mut expansion);
            } else {
                self.expand(index, event, (from, to), local, &mut expansion);
            }
        }
        expansion.occurrences.sort_by_key(|occurrence| (occurrence.start, occurrence.event));
        if expansion.occurrences.len() > MAX_OCCURRENCES {
            expansion.occurrences.truncate(MAX_OCCURRENCES);
            expansion.truncated = true;
        }
        expansion
    }

    fn changed_instance(&self, index: usize, event: &Event, span: (Timestamp, Timestamp), local: &dyn Offsets, expansion: &mut Expansion) {
        let Some(start) = self.zones.resolve(&event.start, local) else { return };
        let length = self.length(event, local);
        let end = self.end_of(length, &event.start, start, local);
        if overlaps(start, end, span) {
            expansion.occurrences.push(Occurrence { event: index, start, end, recurrence_id: event.recurrence_id.clone() });
        }
    }

    fn expand(&self, index: usize, event: &Event, span: (Timestamp, Timestamp), local: &dyn Offsets, expansion: &mut Expansion) {
        let offsets = self.zones.offsets(&event.start, local);
        let length = self.length(event, local);
        let Some(first) = wall_clock(&event.start) else { return };
        let recurring = !event.rules.is_empty() || !event.dates.is_empty();
        let mut instances = BTreeSet::from([first]);
        for date in &event.dates {
            let civil = match date {
                When::Date(day) => Some(day.midnight()),
                other => self.zones.resolve(other, local).and_then(|at| offsets.civil(at)),
            };
            instances.extend(civil);
        }
        let look_from =
            span.0.checked_sub(Duration::from_secs(length.most_secs().saturating_add(2 * DAY_SECS).unsigned_abs())).unwrap_or(Timestamp::MIN);
        let look_from = offsets.civil(look_from);
        let look_to = offsets
            .civil(span.1)
            .and_then(|civil| civil_from_seconds(civil_seconds(civil).saturating_add(DAY_SECS)))
            .or_else(|| CivilDateTime::new(10_000, 12, 31, 23, 59, 59));
        for rule in &event.rules {
            let mut recurrence = rule.iter(first);
            if let Some(stop) = [until(rule, &event.start, offsets), look_to].into_iter().flatten().min() {
                recurrence = recurrence.stop_after(stop);
            }
            if let Some(look_from) = look_from {
                recurrence.skip_to(look_from);
            }
            let mut steps = 0usize;
            for civil in recurrence.by_ref() {
                steps = steps.saturating_add(1);
                if steps > MAX_STEPS {
                    expansion.truncated = true;
                    break;
                }
                if look_from.is_none_or(|look_from| civil >= look_from) {
                    instances.insert(civil);
                }
            }
            expansion.truncated |= recurrence.truncated();
        }
        let replaced: Vec<&When> = self
            .events
            .iter()
            .filter(|other| other.uid == event.uid)
            .filter_map(|other| other.recurrence_id.as_ref())
            .chain(&event.exceptions)
            .collect();
        for civil in instances {
            let when = instance(&event.start, civil);
            let Some(start) = self.zones.resolve(&when, local) else { continue };
            if replaced.iter().any(|gone| self.same_instance(gone, civil, start, local)) {
                continue;
            }
            let end = self.end_of(length, &when, start, local);
            if overlaps(start, end, span) {
                expansion.occurrences.push(Occurrence { event: index, start, end, recurrence_id: recurring.then_some(when) });
            }
        }
    }

    /// Whether `gone` (an `EXDATE` or a `RECURRENCE-ID`) names the instance at `civil`, `start`: a date names every
    /// instance on that day.
    fn same_instance(&self, gone: &When, civil: CivilDateTime, start: Timestamp, local: &dyn Offsets) -> bool {
        match gone {
            When::Date(date) => Date::of(civil) == *date,
            other => self.zones.resolve(other, local) == Some(start),
        }
    }

    fn length(&self, event: &Event, local: &dyn Offsets) -> Length {
        match (&event.start, &event.end, event.duration) {
            (When::Date(first), Some(When::Date(last)), _) => {
                let days = (civil_seconds(last.midnight()) - civil_seconds(first.midnight())) / DAY_SECS;
                Length::Nominal(Span::new(days.max(0), 0).unwrap_or_default())
            }
            (start, Some(end), _) => {
                let secs = self
                    .zones
                    .resolve(end, local)
                    .zip(self.zones.resolve(start, local))
                    .map_or(0, |(end, start)| end.unix_seconds() - start.unix_seconds());
                Length::Exact(secs.max(0))
            }
            (_, None, Some(span)) if !span.is_negative() => Length::Nominal(span),
            (When::Date(_), None, None) => Length::Nominal(Span::new(1, 0).unwrap_or_default()),
            _ => Length::Exact(0),
        }
    }

    fn end_of(&self, length: Length, when: &When, start: Timestamp, local: &dyn Offsets) -> Timestamp {
        let end = match length {
            Length::Exact(secs) => start.checked_add(Duration::from_secs(secs.unsigned_abs())),
            Length::Nominal(span) => wall_clock(when)
                .and_then(|civil| civil_from_seconds(civil_seconds(civil).saturating_add(span.days().saturating_mul(DAY_SECS))))
                .and_then(|civil| self.zones.resolve(&instance(when, civil), local))
                .and_then(|moved| moved.checked_add(Duration::from_secs(span.seconds().unsigned_abs()))),
        };
        end.unwrap_or(Timestamp::MAX).max(start)
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
