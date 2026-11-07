// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Time zones a calendar carries (`VTIMEZONE`, RFC 5545 section 3.6.5) and how a [`When`] becomes a moment.

use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp};

use crate::value::{DAY_SECS, civil_from_seconds, civil_seconds};
use crate::{Component, End, IcalError, Rule, When, read_offset};

/// The most onsets of one observance [`TimeZone`] walks through to find the offset at a moment.
pub const MAX_ONSETS: usize = 10_000;

/// How far before a moment the onset that holds it is looked for: more than a year, so a yearly rule's last onset
/// is found.
const LOOK_BACK_DAYS: i64 = 400;

/// Zone names that stand for UTC itself, which need no `VTIMEZONE`.
const UTC_NAMES: [&str; 9] = ["UTC", "Etc/UTC", "GMT", "Etc/GMT", "Z", "Zulu", "Etc/Zulu", "Universal", "Etc/Universal"];

/// One `STANDARD` or `DAYLIGHT` part of a zone: from its onsets on, the wall clock is `to` from UTC.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Observance {
    /// The first onset, on the wall clock before it (at `from`).
    start: CivilDateTime,
    from: Offset,
    to: Offset,
    rules: Vec<Rule>,
    /// More onsets (`RDATE`), on the wall clock before them.
    dates: Vec<CivilDateTime>,
}

impl Observance {
    fn read(component: &Component) -> Result<Self, IcalError> {
        let missing = |property| IcalError::Missing { component: "VTIMEZONE", property };
        let start = component.property("DTSTART").ok_or(missing("DTSTART"))?;
        let start = wall_clock(&When::from_property(start)?).ok_or_else(|| IcalError::bad_value("DTSTART"))?;
        let from = read_offset(component.property("TZOFFSETFROM").ok_or(missing("TZOFFSETFROM"))?)?;
        let to = read_offset(component.property("TZOFFSETTO").ok_or(missing("TZOFFSETTO"))?)?;
        let rules = component.properties_named("RRULE").map(Rule::from_property).collect::<Result<_, _>>()?;
        let mut dates = Vec::new();
        for property in component.properties_named("RDATE") {
            dates.extend(When::list_from_property(property)?.iter().filter_map(wall_clock));
        }
        Ok(Self { start, from, to, rules, dates })
    }

    fn onset(&self, civil: CivilDateTime) -> Option<Timestamp> {
        civil.to_timestamp(self.from)
    }

    /// The last onset at or before `at`.
    fn last_onset(&self, at: Timestamp) -> Option<Timestamp> {
        let mut last = [self.start].iter().chain(&self.dates).filter_map(|civil| self.onset(*civil)).filter(|onset| *onset <= at).max();
        let look_from = at.to_civil(self.from).map(civil_seconds).and_then(|secs| civil_from_seconds(secs.saturating_sub(LOOK_BACK_DAYS * DAY_SECS)));
        for rule in &self.rules {
            let mut onsets = rule.iter(self.start);
            if let Some(look_from) = look_from {
                onsets.skip_to(look_from);
            }
            for civil in onsets.take(MAX_ONSETS) {
                let Some(onset) = self.onset(civil) else { break };
                if onset > at || ends_before(&rule.end, civil, onset) {
                    break;
                }
                last = last.max(Some(onset));
            }
        }
        last
    }
}

/// Whether a rule that ends with `end` has ended before the onset at `civil` (`onset` in UTC).
fn ends_before(end: &End, civil: CivilDateTime, onset: Timestamp) -> bool {
    match end {
        End::Until(When::Utc(until)) => onset > *until,
        End::Until(When::Date(date)) => crate::Date::of(civil) > *date,
        End::Until(When::Floating(until) | When::Zoned(until, _)) => civil > *until,
        _ => false,
    }
}

/// The wall-clock time of a value, whatever its form: a VTIMEZONE's times are local.
fn wall_clock(when: &When) -> Option<CivilDateTime> {
    match when {
        When::Date(date) => Some(date.midnight()),
        When::Floating(civil) | When::Zoned(civil, _) => Some(*civil),
        When::Utc(at) => at.to_civil(Offset::UTC),
    }
}

/// A time zone as a calendar describes it: its `TZID` and the offsets over time, read from the `VTIMEZONE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZone {
    tzid: String,
    observances: Vec<Observance>,
    component: Component,
}

impl TimeZone {
    /// The zone a `VTIMEZONE` component describes.
    ///
    /// # Errors
    ///
    /// [`IcalError::Missing`] without a `TZID`, a `STANDARD` or `DAYLIGHT` part, or one of their `DTSTART`,
    /// `TZOFFSETFROM` and `TZOFFSETTO`; [`IcalError::BadValue`] when one of them cannot be read.
    pub fn from_component(component: &Component) -> Result<Self, IcalError> {
        let tzid = component.property("TZID").ok_or(IcalError::Missing { component: "VTIMEZONE", property: "TZID" })?;
        let observances = component
            .components()
            .iter()
            .filter(|part| part.name() == "STANDARD" || part.name() == "DAYLIGHT")
            .map(Observance::read)
            .collect::<Result<Vec<_>, _>>()?;
        if observances.is_empty() {
            return Err(IcalError::Missing { component: "VTIMEZONE", property: "STANDARD" });
        }
        Ok(Self { tzid: tzid.value().to_owned(), observances, component: component.clone() })
    }

    /// The `TZID`.
    #[must_use]
    pub fn tzid(&self) -> &str {
        &self.tzid
    }

    /// The `VTIMEZONE` as it was read, to be written back.
    #[must_use]
    pub fn component(&self) -> &Component {
        &self.component
    }
}

impl Offsets for TimeZone {
    /// The offset of the observance with the last onset at or before `at`; before every onset, the offset the
    /// earliest observance changes from.
    fn offset_at(&self, at: Timestamp) -> Offset {
        let latest =
            self.observances.iter().filter_map(|observance| Some((observance.last_onset(at)?, observance.to))).max_by_key(|(onset, _)| *onset);
        latest.map_or_else(
            || self.observances.iter().min_by_key(|observance| observance.start).map_or(Offset::UTC, |first| first.from),
            |(_, offset)| offset,
        )
    }
}

/// The time zones of a calendar, found by `TZID`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Zones {
    zones: Vec<TimeZone>,
}

impl Zones {
    /// No zones.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `zone`, replacing a zone with its `TZID`.
    pub fn push(&mut self, zone: TimeZone) {
        self.zones.retain(|known| known.tzid != zone.tzid);
        self.zones.push(zone);
    }

    /// The zone `tzid` names.
    #[must_use]
    pub fn get(&self, tzid: &str) -> Option<&TimeZone> {
        self.zones.iter().find(|zone| zone.tzid == tzid)
    }

    /// Every zone, in the order they came.
    #[must_use]
    pub fn all(&self) -> &[TimeZone] {
        &self.zones
    }

    /// The offsets the wall-clock times of `when` follow: UTC for the `Z` form and for UTC's own names, the zone a
    /// `TZID` names, and `local` (the person's own) for whole days, floating times and a `TZID` no `VTIMEZONE`
    /// describes.
    #[must_use]
    pub fn offsets<'a>(&'a self, when: &When, local: &'a dyn Offsets) -> &'a dyn Offsets {
        match when {
            When::Utc(_) => &Offset::UTC,
            When::Zoned(_, tzid) => match self.get(tzid) {
                Some(zone) => zone,
                None if UTC_NAMES.iter().any(|name| name.eq_ignore_ascii_case(tzid)) => &Offset::UTC,
                None => local,
            },
            When::Date(_) | When::Floating(_) => local,
        }
    }

    /// The moment `when` stands for, whole days at their start, or `None` outside the range of [`Timestamp`].
    #[must_use]
    pub fn resolve(&self, when: &When, local: &dyn Offsets) -> Option<Timestamp> {
        match when {
            When::Utc(at) => Some(*at),
            When::Date(date) => local.resolve(date.midnight()),
            When::Floating(civil) | When::Zoned(civil, _) => self.offsets(when, local).resolve(*civil),
        }
    }
}

#[cfg(test)]
#[path = "zone_tests.rs"]
pub(crate) mod tests;
