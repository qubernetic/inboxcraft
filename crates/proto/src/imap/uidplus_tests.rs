// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{Uid, UidSet};

use super::{CopyUid, nth};
use crate::imap::MAX_SET_RANGES;

fn uid(n: u32) -> Uid {
    Uid::new(n).unwrap()
}

fn set(ranges: &[(u32, u32)]) -> UidSet {
    ranges.iter().map(|(a, b)| (uid(*a), uid(*b))).collect()
}

fn pairs(copy: &CopyUid) -> Vec<(u32, u32)> {
    copy.pairs().map(|(a, b)| (a.get(), b.get())).collect()
}

#[test]
fn pairs_follow_the_order_the_server_wrote() {
    let copy = CopyUid::parse("38505 304,319:320 3956:3958", &set(&[(304, 320)]));
    let copy = copy.as_ref();
    assert_eq!(copy.map(pairs), Some(vec![(304, 3956), (319, 3957), (320, 3958)]));
    assert_eq!(copy.map(|c| c.uid_validity().get()), Some(38505));
    assert_eq!(copy.map(CopyUid::len), Some(3));
    assert_eq!(copy.map(CopyUid::is_empty), Some(false));

    let backwards = CopyUid::parse("7 6:4 12:10", &set(&[(4, 6)]));
    let backwards = backwards.as_ref();
    assert_eq!(backwards.map(pairs), Some(vec![(6, 12), (5, 11), (4, 10)]));
    assert_eq!(backwards.and_then(|c| c.destination_of(uid(5))), Some(uid(11)));
    assert_eq!(backwards.and_then(|c| c.destination_of(uid(4))), Some(uid(10)));
}

#[test]
fn destination_of_a_uid_counts_through_the_ranges() {
    let copy = CopyUid::parse("1 10,20:22,9 100:102,50,40", &set(&[(1, 100)]));
    let copy = copy.as_ref();
    let found = |n| copy.and_then(|c| c.destination_of(uid(n))).map(Uid::get);
    assert_eq!(found(10), Some(100));
    assert_eq!(found(21), Some(102));
    assert_eq!(found(22), Some(50));
    assert_eq!(found(9), Some(40));
    assert_eq!(found(11), None);
    assert_eq!(copy.map(|c| c.source().len()), Some(3));
    assert_eq!(copy.map(|c| c.destination().len()), Some(3));
}

#[test]
fn a_copyuid_that_does_not_fit_is_left_out() {
    let asked = set(&[(4, 6)]);
    for arg in [
        "",
        "7",
        "7 4",
        "7 4 10 11",
        "x 4 10",
        "0 4 10",
        "7 4 0",
        "7 0 10",
        "7 4 *",
        "7 4: 10",
        "7 3 10",
        "7 4:7 10:13",
        "7 4,4 10:11",
        "7 4:5 10",
        "7 4 10:11",
        "7 -4 10",
    ] {
        assert_eq!(CopyUid::parse(arg, &asked), None, "{arg:?}");
    }
    let many = vec!["4"; MAX_SET_RANGES + 1].join(",");
    assert_eq!(CopyUid::parse(&format!("7 4 {many}"), &asked), None);
}

#[test]
fn the_largest_uids_do_not_overflow() {
    let copy = CopyUid::parse("1 4294967294:4294967295 2:1", &set(&[(4_294_967_290, 4_294_967_295)]));
    assert_eq!(copy.as_ref().map(pairs), Some(vec![(4_294_967_294, 2), (4_294_967_295, 1)]));
    assert_eq!(nth(&[(uid(5), uid(6))], 2), None);
    assert_eq!(nth(&[], 0), None);
}
