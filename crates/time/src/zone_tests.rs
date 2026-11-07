// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{OffsetTable, Offsets};
use crate::{CivilDateTime, Offset, Timestamp};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(seconds, 0).unwrap()
}

fn hours(hours: i16) -> Offset {
    Offset::from_minutes(hours * 60).unwrap()
}

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap()
}

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    civil(year, month, day, hour, minute).to_timestamp(Offset::UTC).unwrap()
}

/// Central Europe in 2026: +01:00, +02:00 from 29 March 01:00 UTC, +01:00 again from 25 October 01:00 UTC.
fn europe() -> OffsetTable {
    OffsetTable::new(hours(1)).with_change(utc(2026, 10, 25, 1, 0), hours(1)).with_change(utc(2026, 3, 29, 1, 0), hours(2))
}

#[test]
fn a_fixed_offset_is_the_same_at_every_moment() {
    let offset = hours(-5);
    for moment in [Timestamp::MIN, Timestamp::UNIX_EPOCH, Timestamp::MAX] {
        assert_eq!(offset.offset_at(moment), offset);
    }
    assert_eq!(offset.resolve(civil(2026, 11, 5, 8, 0)), Some(utc(2026, 11, 5, 13, 0)));
    assert_eq!(offset.civil(utc(2026, 11, 5, 13, 0)), Some(civil(2026, 11, 5, 8, 0)));
}

#[test]
fn a_table_gives_the_offset_of_the_last_change_at_or_before_the_moment() {
    let table = europe();
    assert_eq!(table.offset_at(Timestamp::MIN), hours(1));
    assert_eq!(table.offset_at(at(utc(2026, 3, 29, 1, 0).unix_seconds() - 1)), hours(1));
    assert_eq!(table.offset_at(utc(2026, 3, 29, 1, 0)), hours(2), "a change counts from its own second");
    assert_eq!(table.offset_at(utc(2026, 7, 1, 12, 0)), hours(2));
    assert_eq!(table.offset_at(utc(2026, 10, 25, 1, 0)), hours(1));
    assert_eq!(table.offset_at(Timestamp::MAX), hours(1));
}

#[test]
fn a_change_at_the_same_moment_replaces_the_earlier_one() {
    let table = OffsetTable::new(Offset::UTC).with_change(at(100), hours(3)).with_change(at(100), hours(4));
    assert_eq!(table.offset_at(at(100)), hours(4));
    assert_eq!(table.offset_at(at(99)), Offset::UTC);
}

#[test]
fn a_wall_clock_time_that_shows_once_has_one_moment() {
    let table = europe();
    assert_eq!(table.resolve(civil(2026, 1, 15, 8, 0)), Some(utc(2026, 1, 15, 7, 0)));
    assert_eq!(table.resolve(civil(2026, 7, 15, 8, 0)), Some(utc(2026, 7, 15, 6, 0)));
    assert_eq!(table.resolve(civil(2026, 3, 29, 1, 59)), Some(utc(2026, 3, 29, 0, 59)), "just before the gap");
    assert_eq!(table.resolve(civil(2026, 3, 29, 3, 0)), Some(utc(2026, 3, 29, 1, 0)), "just after the gap");
}

#[test]
fn a_wall_clock_time_in_the_spring_gap_moves_forward_by_the_gap() {
    // 02:30 never shows on 29 March: the clocks jump from 02:00 to 03:00. It is read with the offset before the
    // jump, which a wall clock shows as 03:30.
    let table = europe();
    let moment = table.resolve(civil(2026, 3, 29, 2, 30)).unwrap();
    assert_eq!(moment, utc(2026, 3, 29, 1, 30));
    assert_eq!(table.civil(moment), Some(civil(2026, 3, 29, 3, 30)));
}

#[test]
fn a_wall_clock_time_that_shows_twice_in_autumn_is_the_first_one() {
    // 02:30 on 25 October shows at 00:30 UTC (+02:00) and again at 01:30 UTC (+01:00).
    let table = europe();
    assert_eq!(table.resolve(civil(2026, 10, 25, 2, 30)), Some(utc(2026, 10, 25, 0, 30)));
    assert_eq!(table.civil(utc(2026, 10, 25, 1, 30)), Some(civil(2026, 10, 25, 2, 30)));
}

#[test]
fn the_edges_of_time_resolve_or_say_none() {
    let far_east = Offset::from_minutes(5999).unwrap();
    assert_eq!(far_east.resolve(civil(0, 1, 1, 0, 0)), None, "before Timestamp::MIN");
    assert_eq!(Offset::UTC.resolve(civil(0, 1, 1, 0, 0)), Some(Timestamp::MIN));
    let table = OffsetTable::new(far_east).with_change(at(0), Offset::UTC);
    assert_eq!(table.resolve(civil(0, 1, 1, 0, 0)), None, "the offset there is still far east");
    assert_eq!(table.resolve(civil(10_000, 12, 31, 23, 59)), Some(utc(10_000, 12, 31, 23, 59)));
    assert_eq!(far_east.civil(Timestamp::MAX), None);
}

#[test]
fn a_table_is_debug_and_clone_and_says_its_changes() {
    let table = europe();
    assert_eq!(table.clone(), table);
    assert_eq!(table.changes().len(), 2);
    assert!(table.changes().windows(2).all(|pair| pair[0].0 < pair[1].0), "kept in order");
    assert!(format!("{table:?}").contains("OffsetTable"));
}
