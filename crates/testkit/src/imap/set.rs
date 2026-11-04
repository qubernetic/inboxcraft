// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UID sets (RFC 3501 `sequence-set` over UIDs): reading them and writing them.

/// The most ranges in one set.
const MAX_RANGES: usize = 1000;

/// A set of UIDs: ranges with both ends included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UidSet(Vec<(u32, u32)>);

impl UidSet {
    /// Reads `text`; `*` is `highest`, the highest UID in use. `None` for anything else than numbers above zero,
    /// `*`, `:` and `,`, or more than [`MAX_RANGES`] ranges.
    pub(super) fn parse(text: &str, highest: u32) -> Option<Self> {
        let mut ranges = Vec::new();
        for part in text.split(',') {
            if ranges.len() >= MAX_RANGES {
                return None;
            }
            let (first, last) = part.split_once(':').unwrap_or((part, part));
            let (first, last) = (number(first, highest)?, number(last, highest)?);
            ranges.push((first.min(last), first.max(last)));
        }
        Some(Self(ranges))
    }

    pub(super) fn contains(&self, uid: u32) -> bool {
        self.0.iter().any(|&(first, last)| (first..=last).contains(&uid))
    }
}

fn number(text: &str, highest: u32) -> Option<u32> {
    if text == "*" {
        return Some(highest);
    }
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|&number| number > 0)
}

/// `uids`, sorted, as a set with ranges (`3,5:7`).
pub(super) fn compact(uids: &[u32]) -> String {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &uid in uids {
        match ranges.last_mut() {
            Some((_, last)) if last.checked_add(1) == Some(uid) => *last = uid,
            _ => ranges.push((uid, uid)),
        }
    }
    let text: Vec<String> = ranges.iter().map(|&(first, last)| if first == last { first.to_string() } else { format!("{first}:{last}") }).collect();
    text.join(",")
}
