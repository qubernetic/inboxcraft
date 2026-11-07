// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The computer's offset from UTC over time: the hook a front end fills, and a table of changes for tests.

use std::fmt;
use std::time::Duration;

use crate::{CivilDateTime, Offset, Timestamp};

/// How far around a wall-clock time [`Offsets::resolve`] looks for the offsets that may apply: wider than any real
/// offset (-12:00 to +14:00), narrower than the time between two changes of a real zone.
const LOOK: Duration = Duration::from_secs(2 * 86_400);

/// The offset from UTC a wall clock shows at each moment: the computer's time zone, with its summer time.
///
/// The engine asks it whenever it works in the owner's wall-clock time, as the time presets do. An [`Offset`] is one
/// fixed offset; an [`OffsetTable`] lists changes, as a zone with summer time has.
pub trait Offsets: fmt::Debug + Send + Sync {
    /// The offset at `at`.
    fn offset_at(&self, at: Timestamp) -> Offset;

    /// What the wall clock shows at `at`, or `None` outside years 0 to 10000.
    fn civil(&self, at: Timestamp) -> Option<CivilDateTime> {
        at.to_civil(self.offset_at(at))
    }

    /// The moment the wall clock shows `civil`, or `None` outside the range of [`Timestamp`].
    ///
    /// A time the clock shows twice (when it is put back) is the first of the two. A time it never shows (when it is
    /// put forward) is read with the offset from before the change, so it lands as far after the change as it was
    /// meant after the hour that was skipped: 02:30 in a gap from 02:00 to 03:00 is 03:30.
    fn resolve(&self, civil: CivilDateTime) -> Option<Timestamp> {
        let naive = civil.to_timestamp(Offset::UTC)?;
        let before = self.offset_at(naive.checked_sub(LOOK).unwrap_or(Timestamp::MIN));
        let after = self.offset_at(naive.saturating_add(LOOK));
        let shows = |offset: Offset| civil.to_timestamp(offset).filter(|moment| self.offset_at(*moment) == offset);
        let candidates = [before, self.offset_at(naive), after];
        candidates.into_iter().filter_map(shows).min().or_else(|| civil.to_timestamp(before))
    }
}

impl Offsets for Offset {
    fn offset_at(&self, _at: Timestamp) -> Offset {
        *self
    }
}

/// An offset and the moments it changes, as a time zone's rules give them for a span of years.
///
/// ```
/// use inboxcraft_time::{CivilDateTime, Offset, OffsetTable, Offsets, Timestamp};
///
/// let summer = Timestamp::from_unix(1_774_746_000, 0).unwrap(); // 2026-03-29T01:00:00Z
/// let table = OffsetTable::new(Offset::from_minutes(60).unwrap()).with_change(summer, Offset::from_minutes(120).unwrap());
/// let eight = CivilDateTime::new(2026, 7, 1, 8, 0, 0).unwrap();
/// assert_eq!(table.civil(table.resolve(eight).unwrap()), Some(eight));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffsetTable {
    first: Offset,
    /// From when each offset holds, in time order, no two at one moment.
    changes: Vec<(Timestamp, Offset)>,
}

impl OffsetTable {
    /// `first` at every moment, until a change is added.
    #[must_use]
    pub fn new(first: Offset) -> Self {
        Self { first, changes: Vec::new() }
    }

    /// The table with `offset` from `from` on, until the next change; it replaces a change at the same moment.
    #[must_use]
    pub fn with_change(mut self, from: Timestamp, offset: Offset) -> Self {
        match self.changes.binary_search_by_key(&from, |(at, _)| *at) {
            Ok(index) => {
                if let Some(change) = self.changes.get_mut(index) {
                    change.1 = offset;
                }
            }
            Err(index) => self.changes.insert(index, (from, offset)),
        }
        self
    }

    /// The changes, in time order.
    #[must_use]
    pub fn changes(&self) -> &[(Timestamp, Offset)] {
        &self.changes
    }
}

impl Offsets for OffsetTable {
    fn offset_at(&self, at: Timestamp) -> Offset {
        let held = self.changes.partition_point(|(from, _)| *from <= at);
        held.checked_sub(1).and_then(|index| self.changes.get(index)).map_or(self.first, |(_, offset)| *offset)
    }
}

#[cfg(test)]
#[path = "zone_tests.rs"]
mod tests;
