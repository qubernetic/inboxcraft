// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::Timestamp;

#[test]
fn a_timestamp_keeps_its_seconds_and_nanoseconds() {
    let ts = Timestamp::from_unix(1_000, 5).unwrap();
    assert_eq!(ts.unix_seconds(), 1_000);
    assert_eq!(ts.subsec_nanos(), 5);
    assert_eq!(Timestamp::UNIX_EPOCH.unix_seconds(), 0);
}

#[test]
fn out_of_range_timestamps_are_refused() {
    assert!(Timestamp::from_unix(0, 1_000_000_000).is_none());
    assert!(Timestamp::from_unix(Timestamp::MAX.unix_seconds() + 1, 0).is_none());
    assert!(Timestamp::from_unix(Timestamp::MIN.unix_seconds() - 1, 0).is_none());
    assert!(Timestamp::from_unix(i64::MAX, 0).is_none());
    assert!(Timestamp::from_unix(i64::MIN, 0).is_none());
    assert_eq!(Timestamp::from_unix(Timestamp::MIN.unix_seconds(), 0), Some(Timestamp::MIN));
}

#[test]
fn the_range_covers_years_zero_to_ten_thousand() {
    // 0000-01-01T00:00:00Z and 10000-12-31T23:59:59.999999999Z, proleptic Gregorian.
    assert_eq!(Timestamp::MIN.unix_seconds(), -62_167_219_200);
    assert_eq!(Timestamp::MAX.unix_seconds(), 253_433_923_199);
    assert_eq!(Timestamp::MAX.subsec_nanos(), 999_999_999);
}

#[test]
fn adding_carries_the_nanoseconds_and_stops_at_the_range() {
    let ts = Timestamp::from_unix(10, 600_000_000).unwrap();
    assert_eq!(ts.checked_add(Duration::from_millis(500)), Timestamp::from_unix(11, 100_000_000));
    assert_eq!(Timestamp::MAX.checked_add(Duration::from_nanos(1)), None);
    assert_eq!(ts.checked_add(Duration::MAX), None);
    assert_eq!(Timestamp::MAX.saturating_add(Duration::from_secs(1)), Timestamp::MAX);
    assert_eq!(ts.saturating_add(Duration::ZERO), ts);
}

#[test]
fn subtracting_borrows_the_nanoseconds_and_stops_at_the_range() {
    let ts = Timestamp::from_unix(10, 100_000_000).unwrap();
    assert_eq!(ts.checked_sub(Duration::from_millis(500)), Timestamp::from_unix(9, 600_000_000));
    assert_eq!(Timestamp::MIN.checked_sub(Duration::from_nanos(1)), None);
    assert_eq!(ts.checked_sub(Duration::MAX), None);
}

#[test]
fn the_distance_between_timestamps_is_a_duration() {
    let a = Timestamp::from_unix(10, 100_000_000).unwrap();
    let b = Timestamp::from_unix(12, 0).unwrap();
    assert_eq!(b.duration_since(a), Some(Duration::from_millis(1_900)));
    assert_eq!(a.duration_since(b), None);
    assert_eq!(Timestamp::MAX.duration_since(Timestamp::MIN).map(|d| d.as_secs()), Some(315_601_142_399));
}

#[test]
fn timestamps_order_by_time() {
    let a = Timestamp::from_unix(-1, 999_999_999).unwrap();
    let b = Timestamp::from_unix(0, 0).unwrap();
    assert!(a < b);
    assert!(Timestamp::MIN < Timestamp::MAX);
}
