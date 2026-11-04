// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::UidSet;
use crate::Uid;

fn uid(n: u32) -> Uid {
    Uid::new(n).unwrap()
}

fn set(ranges: &[(u32, u32)]) -> UidSet {
    ranges.iter().map(|&(a, b)| (uid(a), uid(b))).collect()
}

fn plain(set: &UidSet) -> Vec<(u32, u32)> {
    set.ranges().iter().map(|(a, b)| (a.get(), b.get())).collect()
}

#[test]
fn ranges_are_sorted_merged_and_turned_round() {
    assert_eq!(plain(&set(&[(10, 12), (1, 3), (4, 4), (20, 15), (11, 13)])), [(1, 4), (10, 13), (15, 20)]);
    assert_eq!(plain(&set(&[(5, 5), (5, 5)])), [(5, 5)]);
    assert_eq!(plain(&set(&[(1, u32::MAX), (7, 9)])), [(1, u32::MAX)]);
    assert_eq!(plain(&set(&[(u32::MAX, u32::MAX), (u32::MAX - 1, u32::MAX - 1)])), [(u32::MAX - 1, u32::MAX)]);
}

#[test]
fn a_set_knows_its_members_without_listing_them() {
    let all = set(&[(1, u32::MAX)]);
    assert_eq!(all.len(), u64::from(u32::MAX));
    assert!(all.contains(uid(123_456)));
    let some = set(&[(2, 4), (8, 8)]);
    assert_eq!(some.len(), 4);
    for (n, member) in [(1, false), (2, true), (4, true), (5, false), (8, true), (9, false)] {
        assert_eq!(some.contains(uid(n)), member, "{n}");
    }
    assert!(UidSet::default().is_empty());
    assert!(!some.is_empty());
    assert_eq!(UidSet::default().len(), 0);
}

#[test]
fn single_uids_make_a_set() {
    let single: UidSet = [uid(3), uid(1), uid(2), uid(7)].into_iter().collect();
    assert_eq!(plain(&single), [(1, 3), (7, 7)]);
}

#[test]
fn a_set_serialises_as_its_ranges_and_is_normalised_on_the_way_in() {
    let json = serde_json::to_string(&set(&[(1, 3), (7, 7)])).unwrap();
    assert_eq!(json, "[[1,3],[7,7]]");
    assert_eq!(plain(&serde_json::from_str::<UidSet>("[[9,7],[1,1],[2,2]]").unwrap()), [(1, 2), (7, 9)]);
    assert!(serde_json::from_str::<UidSet>("[[0,3]]").is_err());
}
