// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use super::{MAX_TYPED, long, parse, short};

/// A UTC time.
fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).and_then(|civil| civil.to_timestamp(Offset::UTC)).unwrap_or(Timestamp::MIN)
}

/// Friday 6 November 2026, 14:30 UTC.
fn now() -> Timestamp {
    at(2026, 11, 6, 14, 30)
}

fn utc(text: &str) -> Option<Timestamp> {
    parse(text, now(), Offset::UTC)
}

#[test]
fn a_weekday_and_an_hour_is_the_next_such_day_at_that_hour() {
    assert_eq!(utc("thu 9"), Some(at(2026, 11, 12, 9, 0)));
    assert_eq!(utc("Thursday 9:30"), Some(at(2026, 11, 12, 9, 30)));
    assert_eq!(utc("  THU   9  "), Some(at(2026, 11, 12, 9, 0)));
    assert_eq!(utc("9 thurs"), Some(at(2026, 11, 12, 9, 0)));
}

#[test]
fn todays_weekday_is_today_while_its_time_is_ahead_and_next_week_after() {
    assert_eq!(utc("fri 16"), Some(at(2026, 11, 6, 16, 0)));
    assert_eq!(utc("fri 9"), Some(at(2026, 11, 13, 9, 0)));
    assert_eq!(utc("fri"), Some(at(2026, 11, 13, 8, 0)), "a day without a time is its morning, 08:00");
}

#[test]
fn a_time_alone_is_today_while_it_is_ahead_and_tomorrow_after() {
    assert_eq!(utc("17:30"), Some(at(2026, 11, 6, 17, 30)));
    assert_eq!(utc("17.30"), Some(at(2026, 11, 6, 17, 30)));
    assert_eq!(utc("9"), Some(at(2026, 11, 7, 9, 0)));
    assert_eq!(utc("14:30"), Some(at(2026, 11, 7, 14, 30)), "now itself is not ahead");
    assert_eq!(utc("5pm"), Some(at(2026, 11, 6, 17, 0)));
    assert_eq!(utc("5 PM"), Some(at(2026, 11, 6, 17, 0)));
    assert_eq!(utc("5:15am"), Some(at(2026, 11, 7, 5, 15)));
    assert_eq!(utc("12am"), Some(at(2026, 11, 7, 0, 0)));
    assert_eq!(utc("12pm"), Some(at(2026, 11, 7, 12, 0)));
    assert_eq!(utc("noon"), Some(at(2026, 11, 7, 12, 0)));
    assert_eq!(utc("midnight"), Some(at(2026, 11, 7, 0, 0)));
}

#[test]
fn today_and_tomorrow_name_their_day() {
    assert_eq!(utc("tomorrow"), Some(at(2026, 11, 7, 8, 0)));
    assert_eq!(utc("tomorrow 9:30"), Some(at(2026, 11, 7, 9, 30)));
    assert_eq!(utc("tmrw 6pm"), Some(at(2026, 11, 7, 18, 0)));
    assert_eq!(utc("today 18"), Some(at(2026, 11, 6, 18, 0)));
    assert_eq!(utc("today"), None, "this morning is behind");
    assert_eq!(utc("today 9"), None);
}

#[test]
fn a_day_and_a_month_is_the_next_such_date() {
    assert_eq!(utc("12 nov"), Some(at(2026, 11, 12, 8, 0)));
    assert_eq!(utc("nov 12 9"), Some(at(2026, 11, 12, 9, 0)));
    assert_eq!(utc("12 November 9:30"), Some(at(2026, 11, 12, 9, 30)));
    assert_eq!(utc("1 jan"), Some(at(2027, 1, 1, 8, 0)));
    assert_eq!(utc("6 nov 10"), Some(at(2027, 11, 6, 10, 0)), "this year's is behind");
    assert_eq!(utc("6 nov 16"), Some(at(2026, 11, 6, 16, 0)));
    assert_eq!(utc("29 feb"), Some(at(2028, 2, 29, 8, 0)), "the next year that has one");
    assert_eq!(utc("31 feb"), None);
    assert_eq!(utc("0 nov"), None);
}

#[test]
fn an_iso_date_is_that_date() {
    assert_eq!(utc("2026-11-20"), Some(at(2026, 11, 20, 8, 0)));
    assert_eq!(utc("2026-11-20 9:15"), Some(at(2026, 11, 20, 9, 15)));
    assert_eq!(utc("2025-01-01"), None, "behind");
    assert_eq!(utc("2026-02-30"), None);
    assert_eq!(utc("2026-1-x"), None);
}

#[test]
fn in_a_number_of_units_counts_from_now_to_the_minute() {
    assert_eq!(utc("in 3 days"), Some(at(2026, 11, 9, 14, 30)));
    assert_eq!(utc("in 1 day"), Some(at(2026, 11, 7, 14, 30)));
    assert_eq!(utc("in 2 hours"), Some(at(2026, 11, 6, 16, 30)));
    assert_eq!(utc("in 45 min"), Some(at(2026, 11, 6, 15, 15)));
    assert_eq!(utc("in 1 week"), Some(at(2026, 11, 13, 14, 30)));
    assert_eq!(utc("3d"), Some(at(2026, 11, 9, 14, 30)));
    assert_eq!(utc("2h"), Some(at(2026, 11, 6, 16, 30)));
    assert_eq!(utc("45m"), Some(at(2026, 11, 6, 15, 15)));
    assert_eq!(utc("2w"), Some(at(2026, 11, 20, 14, 30)));
    let seconds = now().checked_add(std::time::Duration::from_secs(25)).unwrap_or(Timestamp::MIN);
    assert_eq!(parse("in 1 hour", seconds, Offset::UTC), Some(at(2026, 11, 6, 15, 30)), "the seconds are dropped");
}

#[test]
fn what_is_not_a_time_is_nothing() {
    for text in [
        "",
        "   ",
        "hello",
        "25",
        "9:60",
        "24:00",
        "13pm",
        "0am",
        "thu fri",
        "thu 9 10",
        "12 nov dec",
        "in",
        "in 3",
        "in days",
        "in 0 days",
        "in 3 fortnights",
        "in 99999999999 days",
        "in 999999 weeks",
        "3x",
        "9::30",
        ":30",
        "9:3",
        "9:300",
        "pm",
        "th",
        "2026-11-20-1",
        "thu 9 extra",
    ] {
        assert_eq!(utc(text), None, "{text:?}");
    }
}

#[test]
fn hostile_text_is_nothing_and_never_panics() {
    let long_text = "9 ".repeat(MAX_TYPED);
    assert_eq!(utc(&long_text), None);
    for text in ["\u{0}", "thu\u{0}9", "ﬁ 9", "９", "thü 9", "12 nov\u{202E}", "-9", "+9", "in -3 days", "\u{1F600}"] {
        assert_eq!(utc(text), None, "{text:?}");
    }
    assert_eq!(parse("in 10 days", Timestamp::MAX, Offset::UTC), None);
    assert_eq!(parse("tomorrow", Timestamp::MAX, Offset::UTC), None);
    assert_eq!(parse("1 jan", Timestamp::MAX, Offset::UTC), None);
    assert_eq!(parse("thu 9", Timestamp::MIN, Offset::UTC).map(|t| t > Timestamp::MIN), Some(true));
}

#[test]
fn a_parsed_time_is_always_ahead_of_now() {
    for text in ["9", "23:59", "0:00", "fri", "sat", "today 23", "6 nov", "6 nov 23:59", "in 1 min", "1m"] {
        if let Some(time) = utc(text) {
            assert!(time > now(), "{text:?}");
        }
    }
}

#[test]
fn the_wall_clock_is_the_given_offsets() {
    let budapest = Offset::from_minutes(60).unwrap_or(Offset::UTC);
    assert_eq!(parse("thu 9", now(), budapest), Some(at(2026, 11, 12, 8, 0)));
    // 23:30 UTC on Friday is already Saturday in Budapest: "sat 9" is the next day, not a week on.
    let late = at(2026, 11, 6, 23, 30);
    assert_eq!(parse("sat 9", late, budapest), Some(at(2026, 11, 7, 8, 0)));
    assert_eq!(parse("tomorrow 9", late, budapest), Some(at(2026, 11, 8, 8, 0)));
}

#[test]
fn long_names_the_day_date_and_time() {
    assert_eq!(long(at(2026, 11, 12, 9, 0), now(), Offset::UTC), "Thursday 12 November, 09:00");
    assert_eq!(long(at(2027, 1, 1, 18, 5), now(), Offset::UTC), "Friday 1 January 2027, 18:05", "another year names it");
    let budapest = Offset::from_minutes(60).unwrap_or(Offset::UTC);
    assert_eq!(long(at(2026, 11, 12, 9, 0), now(), budapest), "Thursday 12 November, 10:00");
}

#[test]
fn short_is_the_time_today_the_weekday_this_week_and_the_date_after() {
    let utc = Offset::UTC;
    assert_eq!(short(at(2026, 11, 6, 17, 30), now(), utc), "17:30");
    assert_eq!(short(at(2026, 11, 7, 8, 0), now(), utc), "Sat 08:00");
    assert_eq!(short(at(2026, 11, 12, 9, 0), now(), utc), "Thu 09:00");
    assert_eq!(short(at(2026, 11, 13, 8, 0), now(), utc), "Fri 13 Nov");
    assert_eq!(short(at(2027, 1, 1, 8, 0), now(), utc), "Fri 1 Jan 2027");
}

#[test]
fn a_time_out_of_range_formats_as_nothing() {
    let west = Offset::from_minutes(-5999).unwrap_or(Offset::UTC);
    assert_eq!(long(Timestamp::MIN, now(), west), "");
    assert_eq!(short(Timestamp::MIN, now(), west), "");
    assert_eq!(parse("9", Timestamp::MIN, west), None);
}

#[test]
fn every_month_and_weekday_has_its_name() {
    let months: Vec<String> = (1..=12).map(|month| long(at(2026, month, 1, 9, 0), now(), Offset::UTC)).collect();
    for (shown, name) in
        months.iter().zip(["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"])
    {
        assert!(shown.contains(name), "{shown}");
    }
    let days: Vec<String> = (7..=13).map(|day| short(at(2026, 11, day, 9, 0), at(2026, 11, 7, 0, 0), Offset::UTC)).collect();
    assert_eq!(days, ["09:00", "Sun 09:00", "Mon 09:00", "Tue 09:00", "Wed 09:00", "Thu 09:00", "Fri 09:00"]);
}
