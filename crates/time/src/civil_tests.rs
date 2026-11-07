// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{CivilDateTime, Offset, Weekday, days_in_month, is_leap_year};
use crate::Timestamp;

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, hour, minute, second).unwrap()
}

#[test]
fn leap_years_follow_the_gregorian_rule() {
    assert!(is_leap_year(2024));
    assert!(is_leap_year(2000));
    assert!(is_leap_year(0));
    assert!(!is_leap_year(1900));
    assert!(!is_leap_year(2026));
    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2026, 2), 28);
    assert_eq!(days_in_month(2026, 4), 30);
    assert_eq!(days_in_month(2026, 12), 31);
    assert_eq!(days_in_month(2026, 13), 0);
}

#[test]
fn impossible_dates_and_times_are_refused() {
    assert!(CivilDateTime::new(2026, 2, 29, 0, 0, 0).is_none());
    assert!(CivilDateTime::new(2026, 0, 1, 0, 0, 0).is_none());
    assert!(CivilDateTime::new(2026, 13, 1, 0, 0, 0).is_none());
    assert!(CivilDateTime::new(2026, 1, 0, 0, 0, 0).is_none());
    assert!(CivilDateTime::new(2026, 1, 1, 24, 0, 0).is_none());
    assert!(CivilDateTime::new(2026, 1, 1, 0, 60, 0).is_none());
    assert!(CivilDateTime::new(2026, 1, 1, 0, 0, 60).is_none());
    assert!(CivilDateTime::new(-1, 1, 1, 0, 0, 0).is_none());
    assert!(CivilDateTime::new(10_001, 1, 1, 0, 0, 0).is_none());
    assert!(CivilDateTime::new(2024, 2, 29, 23, 59, 59).is_some());
}

#[test]
fn the_fields_read_back() {
    let dt = civil(2026, 11, 4, 13, 5, 9);
    assert_eq!((dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute(), dt.second()), (2026, 11, 4, 13, 5, 9));
}

#[test]
fn known_dates_have_known_timestamps() {
    let utc = Offset::UTC;
    assert_eq!(civil(1970, 1, 1, 0, 0, 0).to_timestamp(utc), Some(Timestamp::UNIX_EPOCH));
    assert_eq!(civil(2026, 11, 4, 12, 0, 0).to_timestamp(utc).map(Timestamp::unix_seconds), Some(1_793_793_600));
    assert_eq!(civil(2000, 2, 29, 0, 0, 0).to_timestamp(utc).map(Timestamp::unix_seconds), Some(951_782_400));
    assert_eq!(civil(1969, 12, 31, 23, 59, 59).to_timestamp(utc).map(Timestamp::unix_seconds), Some(-1));
    assert_eq!(civil(0, 1, 1, 0, 0, 0).to_timestamp(utc), Some(Timestamp::MIN));
    assert_eq!(civil(10_000, 12, 31, 23, 59, 59).to_timestamp(utc).map(Timestamp::unix_seconds), Some(253_433_923_199));
}

#[test]
fn an_offset_moves_the_local_time_away_from_utc() {
    let plus_one = Offset::from_minutes(60).unwrap();
    let local = civil(2026, 11, 4, 13, 0, 0);
    assert_eq!(local.to_timestamp(plus_one).map(Timestamp::unix_seconds), Some(1_793_793_600));
    let ts = Timestamp::from_unix(1_793_793_600, 0).unwrap();
    assert_eq!(ts.to_civil(plus_one), Some(local));
    let minus = Offset::from_minutes(-(5 * 60 + 30)).unwrap();
    assert_eq!(ts.to_civil(minus), Some(civil(2026, 11, 4, 6, 30, 0)));
}

#[test]
fn timestamps_near_the_ends_of_the_range_fall_outside_some_offsets() {
    let plus = Offset::from_minutes(60).unwrap();
    let minus = Offset::from_minutes(-60).unwrap();
    assert_eq!(Timestamp::MIN.to_civil(minus), None);
    assert_eq!(Timestamp::MAX.to_civil(plus), None);
    assert_eq!(civil(0, 1, 1, 0, 0, 0).to_timestamp(plus), None);
    assert_eq!(civil(10_000, 12, 31, 23, 0, 0).to_timestamp(minus), None);
    assert_eq!(Timestamp::MIN.to_civil(Offset::UTC), Some(civil(0, 1, 1, 0, 0, 0)));
    assert_eq!(Timestamp::MAX.to_civil(Offset::UTC), Some(civil(10_000, 12, 31, 23, 59, 59)));
}

#[test]
fn converting_there_and_back_is_lossless_across_the_range() {
    let offsets = [-5999, -720, -1, 0, 1, 345, 5999].map(|m| Offset::from_minutes(m).unwrap());
    let mut secs = Timestamp::MIN.unix_seconds();
    while secs <= Timestamp::MAX.unix_seconds() {
        let ts = Timestamp::from_unix(secs, 0).unwrap();
        for offset in offsets {
            if let Some(local) = ts.to_civil(offset) {
                assert_eq!(local.to_timestamp(offset), Some(ts), "{secs} {offset:?}");
            }
        }
        secs += 86_400 * 37 + 3_671;
    }
}

#[test]
fn the_weekday_comes_from_the_date() {
    assert_eq!(civil(1970, 1, 1, 0, 0, 0).weekday(), Weekday::Thursday);
    assert_eq!(civil(2026, 11, 4, 0, 0, 0).weekday(), Weekday::Wednesday);
    assert_eq!(civil(2000, 1, 1, 0, 0, 0).weekday(), Weekday::Saturday);
    assert_eq!(civil(0, 1, 1, 0, 0, 0).weekday(), Weekday::Saturday);
    assert_eq!(civil(1969, 12, 28, 0, 0, 0).weekday(), Weekday::Sunday);
    assert_eq!(civil(2026, 11, 2, 0, 0, 0).weekday(), Weekday::Monday);
    assert_eq!(civil(2026, 11, 3, 0, 0, 0).weekday(), Weekday::Tuesday);
    assert_eq!(civil(2026, 11, 6, 0, 0, 0).weekday(), Weekday::Friday);
}

#[test]
fn offsets_reach_ninety_nine_hours_and_fifty_nine_minutes() {
    assert_eq!(Offset::from_minutes(5999).map(Offset::minutes), Some(5999));
    assert_eq!(Offset::from_minutes(-5999).map(Offset::minutes), Some(-5999));
    assert!(Offset::from_minutes(6000).is_none());
    assert!(Offset::from_minutes(-6000).is_none());
    assert_eq!(Offset::UTC.minutes(), 0);
    assert_eq!(Offset::default(), Offset::UTC);
}
