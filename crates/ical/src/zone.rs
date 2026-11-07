// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Time zones a calendar carries (`VTIMEZONE`, RFC 5545 section 3.6.5) and how a [`When`] becomes a moment.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp};

use crate::{Component, End, Frequency, IcalError, Rule, When, read_offset};

/// The most onsets a zone lists: its observances' `DTSTART`s and `RDATE`s, and the onsets of its rules with a
/// `COUNT`.
pub const MAX_ONSETS: usize = 10_000;

/// The most `RRULE`s without a `COUNT` the observances of a zone have together.
pub const MAX_ZONE_RULES: usize = 16;

/// The most work (see [`crate::Recurrence::work`]) a rule with a `COUNT` may take to be walked to its end while its
/// zone is read.
const COUNT_WORK: usize = 2_000;

/// Zone names that stand for UTC itself, which need no `VTIMEZONE`.
const UTC_NAMES: [&str; 9] = ["UTC", "Etc/UTC", "GMT", "Etc/GMT", "Z", "Zulu", "Etc/Zulu", "Universal", "Etc/Universal"];

/// Whether `tzid` is one of UTC's own names.
pub(crate) fn is_utc(tzid: &str) -> bool {
    UTC_NAMES.iter().any(|name| name.eq_ignore_ascii_case(tzid))
}

/// An onset of a `STANDARD` or `DAYLIGHT` part: from then on, the wall clock is `to` from UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Onset {
    at: Timestamp,
    /// The part's place in the zone: of two onsets at once, the later part wins.
    part: usize,
    to: Offset,
}

impl Onset {
    fn key(self) -> (Timestamp, usize) {
        (self.at, self.part)
    }
}

/// A rule of a `STANDARD` or `DAYLIGHT` part, without a `COUNT`: its onsets are worked out a year at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ZoneRule {
    rule: Rule,
    /// The part's `DTSTART`, on the wall clock before it.
    start: CivilDateTime,
    part: usize,
    from: Offset,
    to: Offset,
}

impl ZoneRule {
    /// The onsets in `year` (on the wall clock before them), and the work it took.
    fn onsets_in(&self, year: i32) -> (Vec<Timestamp>, usize) {
        let (Some(first), Some(last)) = (CivilDateTime::new(year, 1, 1, 0, 0, 0), CivilDateTime::new(year, 12, 31, 23, 59, 59)) else {
            return (Vec::new(), 1);
        };
        let mut walk = self.rule.iter(self.start).stop_after(last);
        walk.skip_to(first);
        let onsets = walk
            .by_ref()
            .filter_map(|civil| Some((civil, civil.to_timestamp(self.from)?)))
            .filter(|(civil, onset)| !ends_before(&self.rule.end, *civil, *onset));
        let onsets = onsets.map(|(_, onset)| onset).collect();
        (onsets, walk.work().saturating_add(1))
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
///
/// Looking up an offset is cheap whatever the zone says: it takes the last onset of a sorted list, and works out at
/// most this year and the year before of at most [`MAX_ZONE_RULES`] yearly rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZone {
    tzid: String,
    /// The offset before every onset: the one the earliest part changes from.
    before: Offset,
    /// The onsets of `DTSTART`s, `RDATE`s and rules with a `COUNT`, in order.
    onsets: Vec<Onset>,
    rules: Vec<ZoneRule>,
    component: Component,
}

impl TimeZone {
    /// The zone a `VTIMEZONE` component describes.
    ///
    /// # Errors
    ///
    /// [`IcalError::Missing`] without a `TZID`, a `STANDARD` or `DAYLIGHT` part, or one of their `DTSTART`,
    /// `TZOFFSETFROM` and `TZOFFSETTO`; [`IcalError::BadValue`] when one of them cannot be read, for a rule other than
    /// yearly at one time of day, a rule with a `COUNT` that does not end soon, more than [`MAX_ZONE_RULES`] other
    /// rules, or more than [`MAX_ONSETS`] onsets.
    pub fn from_component(component: &Component) -> Result<Self, IcalError> {
        let tzid = component.property("TZID").ok_or(IcalError::Missing { component: "VTIMEZONE", property: "TZID" })?;
        let mut zone =
            Self { tzid: tzid.value().to_owned(), before: Offset::UTC, onsets: Vec::new(), rules: Vec::new(), component: component.clone() };
        let mut earliest: Option<CivilDateTime> = None;
        let parts = component.components().iter().filter(|part| part.name() == "STANDARD" || part.name() == "DAYLIGHT");
        for (index, part) in parts.enumerate() {
            let (start, from) = zone.read_part(index, part)?;
            if earliest.is_none_or(|earliest| start < earliest) {
                earliest = Some(start);
                zone.before = from;
            }
        }
        if earliest.is_none() {
            return Err(IcalError::Missing { component: "VTIMEZONE", property: "STANDARD" });
        }
        zone.onsets.sort_unstable_by_key(|onset| onset.key());
        Ok(zone)
    }

    /// Reads part `index` (a `STANDARD` or `DAYLIGHT`) into the zone: its `DTSTART` on the wall clock and its
    /// `TZOFFSETFROM`.
    fn read_part(&mut self, index: usize, component: &Component) -> Result<(CivilDateTime, Offset), IcalError> {
        let missing = |property| IcalError::Missing { component: "VTIMEZONE", property };
        let start = component.property("DTSTART").ok_or(missing("DTSTART"))?;
        let start = wall_clock(&When::from_property(start)?).ok_or_else(|| IcalError::bad_value("DTSTART"))?;
        let from = read_offset(component.property("TZOFFSETFROM").ok_or(missing("TZOFFSETFROM"))?)?;
        let to = read_offset(component.property("TZOFFSETTO").ok_or(missing("TZOFFSETTO"))?)?;
        let mut starts = vec![start];
        for property in component.properties_named("RDATE") {
            starts.extend(When::list_from_property(property)?.iter().filter_map(wall_clock));
            if self.onsets.len().saturating_add(starts.len()) > MAX_ONSETS {
                return Err(IcalError::bad_value("RDATE"));
            }
        }
        for property in component.properties_named("RRULE") {
            let rule = Rule::from_property(property)?;
            let one = |list: &[u8]| list.len() <= 1;
            if rule.frequency != Frequency::Yearly || !one(&rule.by_hour) || !one(&rule.by_minute) || !one(&rule.by_second) {
                return Err(IcalError::bad_value("RRULE"));
            }
            if matches!(rule.end, End::Count(_)) {
                let room = MAX_ONSETS.saturating_sub(self.onsets.len()).saturating_sub(starts.len());
                let mut walk = rule.iter(start).with_budget(COUNT_WORK);
                starts.extend(walk.by_ref().take(room.saturating_add(1)));
                if walk.truncated() || self.onsets.len().saturating_add(starts.len()) > MAX_ONSETS {
                    return Err(IcalError::bad_value("RRULE"));
                }
            } else if self.rules.len() < MAX_ZONE_RULES {
                self.rules.push(ZoneRule { rule, start, part: index, from, to });
            } else {
                return Err(IcalError::bad_value("RRULE"));
            }
        }
        self.onsets.extend(starts.iter().filter_map(|civil| Some(Onset { at: civil.to_timestamp(from)?, part: index, to })));
        Ok((start, from))
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

    /// The offset at `at`, the onsets of the rules in a year taken from `onsets_in` (rule, year).
    fn offset_with(&self, at: Timestamp, mut onsets_in: impl FnMut(usize, i32) -> Option<Timestamp>) -> Offset {
        let listed = self.onsets.partition_point(|onset| onset.at <= at).checked_sub(1).and_then(|last| self.onsets.get(last)).copied();
        let mut latest = listed;
        for (index, rule) in self.rules.iter().enumerate() {
            let Some(year) = at.to_civil(rule.from).map(|civil| civil.year()) else { continue };
            for year in [year.saturating_sub(1), year] {
                if let Some(onset) = onsets_in(index, year) {
                    let onset = Onset { at: onset, part: rule.part, to: rule.to };
                    if latest.is_none_or(|latest| onset.key() > latest.key()) {
                        latest = Some(onset);
                    }
                }
            }
        }
        latest.map_or(self.before, |onset| onset.to)
    }
}

/// The last of `onsets` (in order) at or before `at`.
fn last_at_or_before(onsets: &[Timestamp], at: Timestamp) -> Option<Timestamp> {
    onsets.partition_point(|onset| *onset <= at).checked_sub(1).and_then(|last| onsets.get(last)).copied()
}

impl Offsets for TimeZone {
    /// The offset of the part with the last onset at or before `at` (of its rules, this year's and last year's
    /// count); before every onset, the offset the earliest part changes from.
    fn offset_at(&self, at: Timestamp) -> Offset {
        self.offset_with(at, |index, year| {
            let rule = self.rules.get(index)?;
            last_at_or_before(&rule.onsets_in(year).0, at)
        })
    }
}

/// A [`TimeZone`] that keeps the onsets of its rules a year at a time as it works them out, and adds that work
/// (see [`crate::Recurrence::work`]) to a count, for one expansion.
#[derive(Debug)]
pub(crate) struct ZoneClock<'a> {
    zone: &'a TimeZone,
    years: Mutex<BTreeMap<(usize, i32), Vec<Timestamp>>>,
    work: &'a AtomicUsize,
}

impl<'a> ZoneClock<'a> {
    pub(crate) fn new(zone: &'a TimeZone, work: &'a AtomicUsize) -> Self {
        Self { zone, years: Mutex::new(BTreeMap::new()), work }
    }
}

impl Offsets for ZoneClock<'_> {
    fn offset_at(&self, at: Timestamp) -> Offset {
        let mut years = self.years.lock().unwrap_or_else(PoisonError::into_inner);
        self.zone.offset_with(at, |index, year| {
            let onsets = match years.entry((index, year)) {
                Entry::Occupied(known) => known.into_mut(),
                Entry::Vacant(new) => {
                    let (onsets, work) = self.zone.rules.get(index)?.onsets_in(year);
                    self.work.fetch_add(work, Ordering::Relaxed);
                    new.insert(onsets)
                }
            };
            last_at_or_before(onsets, at)
        })
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
                None if is_utc(tzid) => &Offset::UTC,
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
