// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, Offset, Timestamp};
use proptest::prelude::*;

use super::{Date, Span, When, escape_text, list_to_property, read_offset, unescape_text, unescape_text_list, write_offset};
use crate::{IcalError, Property};

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, hour, minute, second).unwrap()
}

fn when(property: Property) -> Result<When, IcalError> {
    When::from_property(&property)
}

#[test]
fn text_escapes_both_ways() {
    assert_eq!(escape_text("a\rb\nc\r\nd\u{7}\te"), "a\\nb\\nc\\nd\te");
    assert_eq!(unescape_text("x\\y\\"), "xy\\");
    assert_eq!(unescape_text("a,b"), "a,b");
    assert_eq!(unescape_text_list(""), [""]);
    assert_eq!(unescape_text_list("a,,b\\n"), ["a", "", "b\n"]);
}

#[test]
fn reads_dates_and_the_three_forms_of_date_time() {
    let date = Date::new(2026, 11, 7).unwrap();
    assert_eq!(when(Property::new("DTSTART", "20261107").with_param("VALUE", "DATE")), Ok(When::Date(date)));
    assert_eq!(when(Property::new("DTSTART", "20261107")), Ok(When::Date(date)));
    assert_eq!(when(Property::new("DTSTART", "20261107T093000")), Ok(When::Floating(civil(2026, 11, 7, 9, 30, 0))));
    let utc = Timestamp::from_unix(1_794_043_800, 0).unwrap();
    assert_eq!(when(Property::new("DTSTART", "20261107T093000Z")), Ok(When::Utc(utc)));
    assert_eq!(when(Property::new("DTSTART", "20261107t093000z").with_param("TZID", "Europe/Budapest")), Ok(When::Utc(utc)));
    assert_eq!(
        when(Property::new("DTSTART", " 20261107T093060 ").with_param("TZID", "Europe/Budapest")),
        Ok(When::Zoned(civil(2026, 11, 7, 9, 30, 59), "Europe/Budapest".into()))
    );
    assert!(When::Date(date).is_date());
    assert!(!When::Utc(utc).is_date());
}

#[test]
fn refuses_what_is_no_date() {
    for value in ["", "2026110", "20261131", "2026-1107", "20261107T0930", "20261107X093000", "20261107T2400000", "20261107T09300a", "+0261107"] {
        assert_eq!(when(Property::new("DTEND", value)), Err(IcalError::bad_value("DTEND")), "{value:?}");
    }
    assert_eq!(when(Property::new("DTEND", "20261107T093000").with_param("VALUE", "DATE")), Err(IcalError::bad_value("DTEND")));
}

#[test]
fn writes_what_it_reads() {
    let values = [
        When::Date(Date::new(2026, 2, 3).unwrap()),
        When::Floating(civil(2026, 2, 3, 4, 5, 6)),
        When::Utc(Timestamp::from_unix(1_794_043_800, 0).unwrap()),
        When::Zoned(civil(2026, 2, 3, 4, 5, 6), "Europe/Budapest".into()),
    ];
    let texts =
        ["DTSTART;VALUE=DATE:20260203", "DTSTART:20260203T040506", "DTSTART:20261107T093000Z", "DTSTART;TZID=Europe/Budapest:20260203T040506"];
    for (value, text) in values.iter().zip(texts) {
        let property = value.to_property("dtstart");
        let line = crate::Component::new("X").with_property(property.clone()).write().unwrap();
        assert!(line.contains(text), "{line}");
        assert_eq!(&When::from_property(&property).unwrap(), value);
    }
}

#[test]
fn reads_and_writes_lists_of_dates() {
    let property = Property::new("RDATE", "20261107T093000/PT1H,20261108T093000/20261108T103000").with_param("VALUE", "PERIOD");
    assert_eq!(
        When::list_from_property(&property).unwrap(),
        [When::Floating(civil(2026, 11, 7, 9, 30, 0)), When::Floating(civil(2026, 11, 8, 9, 30, 0))]
    );
    let dates = Property::new("EXDATE", "20261107,20261108").with_param("VALUE", "DATE");
    let read = When::list_from_property(&dates).unwrap();
    assert_eq!(list_to_property("EXDATE", &read), Some(dates));
    assert_eq!(When::list_from_property(&Property::new("EXDATE", "20261107,x")), Err(IcalError::bad_value("EXDATE")));

    let zoned = [When::Zoned(civil(2026, 1, 1, 9, 0, 0), "A".into()), When::Zoned(civil(2026, 1, 2, 9, 0, 0), "A".into())];
    let written = list_to_property("EXDATE", &zoned).unwrap();
    assert_eq!(written.value(), "20260101T090000,20260102T090000");
    assert_eq!(written.param("TZID"), Some("A"));
    let mixed = [When::Zoned(civil(2026, 1, 1, 9, 0, 0), "A".into()), When::Zoned(civil(2026, 1, 2, 9, 0, 0), "B".into())];
    assert_eq!(list_to_property("EXDATE", &mixed), None);
    assert_eq!(list_to_property("EXDATE", &[]), None);
}

#[test]
fn dates_move_by_days() {
    let date = Date::new(2026, 12, 31).unwrap();
    assert_eq!(date.add_days(1), Date::new(2027, 1, 1));
    assert_eq!(date.add_days(-365), Date::new(2025, 12, 31));
    assert_eq!(date.add_days(i64::MAX), None);
    assert_eq!(Date::new(10_000, 12, 31).unwrap().add_days(1), None);
    assert_eq!(Date::of(civil(2026, 3, 4, 5, 6, 7)), Date::new(2026, 3, 4).unwrap());
    assert_eq!(date.midnight(), civil(2026, 12, 31, 0, 0, 0));
    assert_eq!(Date::new(2026, 2, 29), None);
}

#[test]
fn reads_durations() {
    let cases = [("P15DT5H0M20S", 15, 5 * 3600 + 20), ("+P2W", 14, 0), ("-PT15M", 0, -900), ("p1dt1s", 1, 1), ("PT0S", 0, 0), ("-P1DT1H", -1, -3600)];
    for (text, days, seconds) in cases {
        let span = Span::parse(text).unwrap();
        assert_eq!((span.days(), span.seconds()), (days, seconds), "{text}");
    }
    for text in ["", "P", "PT", "P1", "1D", "P1H", "PT1D", "P1DT", "PT1W", "P1D2", "PT1H1T", "P99999999999999999999D", "P9999999W", "P1X", "PTT1S"] {
        assert_eq!(Span::parse(text), Err(IcalError::bad_value("DURATION")), "{text}");
    }
    assert!(Span::parse("-PT1S").unwrap().is_negative());
    assert!(!Span::parse("PT1S").unwrap().is_negative());
}

#[test]
fn writes_durations_shortest() {
    let cases = [(0, 0, "P0D"), (1, 0, "P1D"), (0, 3600, "PT1H"), (2, 3661, "P2DT1H1M1S"), (0, 60, "PT1M"), (0, -5, "-PT5S"), (-7, 0, "-P7D")];
    for (days, seconds, text) in cases {
        assert_eq!(Span::new(days, seconds).unwrap().to_string(), text);
    }
    assert_eq!(Span::new(1, -1), None);
    assert_eq!(Span::new(i64::MAX, 0), None);
    assert_eq!(Span::default(), Span::new(0, 0).unwrap());
}

#[test]
fn reads_and_writes_utc_offsets() {
    let offset = |value: &str| read_offset(&Property::new("TZOFFSETTO", value));
    assert_eq!(offset("+0100"), Ok(Offset::from_minutes(60).unwrap()));
    assert_eq!(offset("-0530"), Ok(Offset::from_minutes(-330).unwrap()));
    assert_eq!(offset("+013045"), Ok(Offset::from_minutes(90).unwrap()));
    for value in ["", "0100", "+01", "+01000", "+0160", "+01a0", "+0100xx", "*0100"] {
        assert_eq!(offset(value), Err(IcalError::bad_value("TZOFFSETTO")), "{value:?}");
    }
    assert_eq!(write_offset(Offset::from_minutes(-330).unwrap()), "-0530");
    assert_eq!(write_offset(Offset::UTC), "+0000");
}

proptest! {
    #[test]
    fn escaped_text_reads_back(text in "[ -~\téő\n]{0,40}") {
        prop_assert_eq!(unescape_text(&escape_text(&text)), text);
    }

    #[test]
    fn written_durations_read_back(days in -100_000i64..100_000, seconds in 0i64..10_000_000) {
        let span = Span::new(days, if days < 0 { -seconds } else { seconds }).unwrap();
        prop_assert_eq!(Span::parse(&span.to_string()), Ok(span));
    }
}
