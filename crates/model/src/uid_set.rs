// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sets of UIDs, kept as ranges: a server can name four billion UIDs in one `1:*`.

use serde::{Deserialize, Serialize};

use crate::Uid;

/// A set of UIDs as sorted, disjoint, non-adjacent ranges, both ends included.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<(Uid, Uid)>", into = "Vec<(Uid, Uid)>")]
pub struct UidSet {
    ranges: Vec<(Uid, Uid)>,
}

impl UidSet {
    /// The ranges, sorted.
    #[must_use]
    pub fn ranges(&self) -> &[(Uid, Uid)] {
        &self.ranges
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// How many UIDs the set holds.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.ranges
            .iter()
            .map(|(first, last)| u64::from(last.get()).saturating_sub(u64::from(first.get())).saturating_add(1))
            .fold(0, u64::saturating_add)
    }

    /// Whether `uid` is in the set.
    #[must_use]
    pub fn contains(&self, uid: Uid) -> bool {
        let after = self.ranges.partition_point(|(first, _)| *first <= uid);
        after.checked_sub(1).and_then(|at| self.ranges.get(at)).is_some_and(|(_, last)| uid <= *last)
    }
}

impl FromIterator<(Uid, Uid)> for UidSet {
    /// The set of the ranges, in any order; a range written backwards (`20:15`, as IMAP allows) is turned round.
    fn from_iter<T: IntoIterator<Item = (Uid, Uid)>>(ranges: T) -> Self {
        let mut sorted: Vec<(Uid, Uid)> = ranges.into_iter().map(|(a, b)| (a.min(b), a.max(b))).collect();
        sorted.sort_unstable();
        let mut merged: Vec<(Uid, Uid)> = Vec::with_capacity(sorted.len());
        for (first, last) in sorted {
            match merged.last_mut() {
                Some((_, end)) if first.get().saturating_sub(1) <= end.get() => *end = (*end).max(last),
                _ => merged.push((first, last)),
            }
        }
        Self { ranges: merged }
    }
}

impl FromIterator<Uid> for UidSet {
    fn from_iter<T: IntoIterator<Item = Uid>>(uids: T) -> Self {
        uids.into_iter().map(|uid| (uid, uid)).collect()
    }
}

impl From<Vec<(Uid, Uid)>> for UidSet {
    fn from(ranges: Vec<(Uid, Uid)>) -> Self {
        ranges.into_iter().collect()
    }
}

impl From<UidSet> for Vec<(Uid, Uid)> {
    fn from(set: UidSet) -> Self {
        set.ranges
    }
}

#[cfg(test)]
#[path = "uid_set_tests.rs"]
mod tests;
