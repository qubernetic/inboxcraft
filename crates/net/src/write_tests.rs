// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The UIDs a read-back after a store did not find; the jobs themselves are tested against the fake server in
//! `tests/write.rs`.

use inboxcraft_model::{Uid, UidSet};

use super::absent;

fn set(ranges: &[(u32, u32)]) -> UidSet {
    ranges.iter().filter_map(|&(first, last)| Uid::new(first).zip(Uid::new(last))).collect()
}

#[test]
fn what_came_back_is_not_absent() {
    assert_eq!(absent(&set(&[(2, 3)]), &set(&[(2, 3)])), UidSet::default());
    assert_eq!(absent(&set(&[(2, 3), (7, 7)]), &set(&[(1, 10)])), UidSet::default());
}

#[test]
fn nothing_back_is_all_absent() {
    assert_eq!(absent(&set(&[(2, 3), (7, 9)]), &UidSet::default()), set(&[(2, 3), (7, 9)]));
}

#[test]
fn gaps_before_between_and_after_are_absent() {
    let asked = set(&[(1, 10), (20, 25)]);
    let present = set(&[(3, 4), (6, 6), (9, 21), (24, 24)]);
    assert_eq!(absent(&asked, &present), set(&[(1, 2), (5, 5), (7, 8), (22, 23), (25, 25)]));
}

#[test]
fn ranges_outside_what_was_asked_count_for_nothing() {
    assert_eq!(absent(&set(&[(5, 6)]), &set(&[(1, 2), (8, 9)])), set(&[(5, 6)]));
}

#[test]
fn the_highest_uid_ends_the_sweep() {
    let top = u32::MAX;
    assert_eq!(absent(&set(&[(top - 2, top)]), &set(&[(top - 1, top)])), set(&[(top - 2, top - 2)]));
    assert_eq!(absent(&set(&[(top - 2, top)]), &set(&[(top - 2, top - 2)])), set(&[(top - 1, top)]));
}
