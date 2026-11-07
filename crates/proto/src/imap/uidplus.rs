// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UIDPLUS (RFC 4315): where copied and moved messages went (`COPYUID`), and where an appended one went
//! (`APPENDUID`).

use inboxcraft_model::{Uid, UidSet, UidValidity};

use super::search::MAX_SET_RANGES;
use super::value::number;

/// `COPYUID` (RFC 4315 section 3): the UIDs copied or moved messages have in the mailbox they went to.
///
/// The ranges are kept as the server wrote them, backwards ones too, because the n-th source UID went to the n-th
/// destination UID. A COPYUID is only kept when it fits what was asked for: its source UIDs are among them, none
/// twice, and as many as the destination UIDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyUid {
    uid_validity: UidValidity,
    source: Vec<(Uid, Uid)>,
    destination: Vec<(Uid, Uid)>,
}

impl CopyUid {
    /// The UIDVALIDITY of the mailbox the messages went to.
    #[must_use]
    pub fn uid_validity(&self) -> UidValidity {
        self.uid_validity
    }

    /// The source UIDs, as ranges in the server's order and direction (`6:4` is 6, 5, 4).
    #[must_use]
    pub fn source(&self) -> &[(Uid, Uid)] {
        &self.source
    }

    /// The destination UIDs, as [`source`](Self::source).
    #[must_use]
    pub fn destination(&self) -> &[(Uid, Uid)] {
        &self.destination
    }

    /// How many messages the COPYUID names.
    #[must_use]
    pub fn len(&self) -> u64 {
        count(&self.source)
    }

    /// Whether it names none; never true for one the client handed out.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.source.is_empty()
    }

    /// Each source UID with the UID it has in the mailbox it went to.
    pub fn pairs(&self) -> impl Iterator<Item = (Uid, Uid)> + '_ {
        expand(&self.source).zip(expand(&self.destination))
    }

    /// The UID that the message with `source` UID has in the mailbox it went to.
    #[must_use]
    pub fn destination_of(&self, source: Uid) -> Option<Uid> {
        let mut at = 0_u64;
        for &(first, last) in &self.source {
            let (low, high) = (first.min(last), first.max(last));
            if (low..=high).contains(&source) {
                let offset = u64::from(source.get().abs_diff(first.get()));
                return nth(&self.destination, at.saturating_add(offset));
            }
            at = at.saturating_add(span(first, last));
        }
        None
    }

    /// `<uidvalidity> <source set> <destination set>`, the argument of a `COPYUID` code, when it fits `asked`.
    pub(crate) fn parse(arg: &str, asked: &UidSet) -> Option<Self> {
        let mut words = arg.split_ascii_whitespace();
        let uid_validity = number(words.next()?).and_then(UidValidity::new)?;
        let source = ranges(words.next()?)?;
        let destination = ranges(words.next()?)?;
        if words.next().is_some() {
            return None;
        }
        Self::checked(uid_validity, source, destination, asked)
    }

    /// This COPYUID and a later one for the same move (a server may move the messages in parts), when together they
    /// still fit `asked`.
    pub(crate) fn merged(mut self, other: Self, asked: &UidSet) -> Option<Self> {
        if other.uid_validity != self.uid_validity {
            return None;
        }
        self.source.extend(other.source);
        self.destination.extend(other.destination);
        Self::checked(self.uid_validity, self.source, self.destination, asked)
    }

    fn checked(uid_validity: UidValidity, source: Vec<(Uid, Uid)>, destination: Vec<(Uid, Uid)>, asked: &UidSet) -> Option<Self> {
        let set: UidSet = source.iter().copied().collect();
        // Not one UID twice, each asked for, and one destination for each.
        let fits = set.len() == count(&source) && count(&source) == count(&destination) && within(&set, asked);
        fits.then_some(Self { uid_validity, source, destination })
    }
}

/// `APPENDUID` (RFC 4315 section 3): the UID an appended message got, and the UIDVALIDITY it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppendUid {
    /// The UIDVALIDITY of the mailbox the message went to.
    pub uid_validity: UidValidity,
    /// Its UID there.
    pub uid: Uid,
}

impl AppendUid {
    /// `<uidvalidity> <uid>`, the argument of an `APPENDUID` code for one message; a set (MULTIAPPEND) is none.
    pub(crate) fn parse(arg: &str) -> Option<Self> {
        let mut words = arg.split_ascii_whitespace();
        let uid_validity = number(words.next()?).and_then(UidValidity::new)?;
        let uid = number(words.next()?).and_then(Uid::new)?;
        words.next().is_none().then_some(Self { uid_validity, uid })
    }
}

/// How many UIDs a range names, `6:4` as three.
fn span(first: Uid, last: Uid) -> u64 {
    u64::from(first.get().abs_diff(last.get())).saturating_add(1)
}

fn count(ranges: &[(Uid, Uid)]) -> u64 {
    ranges.iter().map(|&(first, last)| span(first, last)).fold(0, u64::saturating_add)
}

/// The UIDs of the ranges one by one, in their order and direction.
fn expand(ranges: &[(Uid, Uid)]) -> impl Iterator<Item = Uid> + '_ {
    ranges.iter().flat_map(|&(first, last)| {
        let (start, step_up) = (first.get(), first <= last);
        (0..span(first, last)).filter_map(move |i| {
            let i = u32::try_from(i).ok()?;
            let uid = if step_up { start.checked_add(i) } else { start.checked_sub(i) };
            uid.and_then(Uid::new)
        })
    })
}

/// The `n`-th UID (from zero) of the ranges.
fn nth(ranges: &[(Uid, Uid)], mut n: u64) -> Option<Uid> {
    for &(first, last) in ranges {
        let length = span(first, last);
        if n < length {
            let n = u32::try_from(n).ok()?;
            let uid = if first <= last { first.get().checked_add(n) } else { first.get().checked_sub(n) };
            return uid.and_then(Uid::new);
        }
        n = n.saturating_sub(length);
    }
    None
}

/// Whether every UID of `inner` is in `outer`.
fn within(inner: &UidSet, outer: &UidSet) -> bool {
    // `outer` is merged, so a range of `inner` lies inside one of its ranges or is not inside it.
    inner.ranges().iter().all(|&(first, last)| outer.ranges().iter().any(|&(low, high)| low <= first && last <= high))
}

/// A UID set as the server wrote it, `304,6:4`: no `*`, no zero, at most [`MAX_SET_RANGES`] ranges.
pub(crate) fn ranges(text: &str) -> Option<Vec<(Uid, Uid)>> {
    let uid = |part: &str| number(part).and_then(Uid::new);
    let parts: Vec<&str> = text.split(',').take(MAX_SET_RANGES.saturating_add(1)).collect();
    if parts.len() > MAX_SET_RANGES {
        return None;
    }
    parts
        .into_iter()
        .map(|range| match range.split_once(':') {
            Some((first, last)) => Some((uid(first)?, uid(last)?)),
            None => uid(range).map(|one| (one, one)),
        })
        .collect()
}

#[cfg(test)]
#[path = "uidplus_tests.rs"]
mod tests;
