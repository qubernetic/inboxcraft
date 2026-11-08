// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{Offset, Timestamp, Weekday};

use super::{Day, dated, long, month_name, span, weekday_letter, weekday_short};

fn day(year: i32, month: u8, date: u8) -> Day {
    Day::new(year, month, date).unwrap()
}

#[test]
fn a_day_knows_its_date_weekday_week_and_month() {
    let thursday = day(2026, 11, 5);
    assert_eq!((thursday.year(), thursday.month(), thursday.date()), (2026, 11, 5));
    assert_eq!(thursday.weekday(), Weekday::Thursday);
    assert_eq!(thursday.weekday_index(), 3);
    assert_eq!(thursday.monday(), day(2026, 11, 2));
    assert_eq!(day(2026, 11, 8).monday(), day(2026, 11, 2), "Sunday ends the week");
    assert_eq!(thursday.first_of_month(), day(2026, 11, 1));
    assert_eq!(thursday.plus(27), day(2026, 12, 2));
    assert_eq!(day(2026, 12, 2).since(thursday), 27);
    assert_eq!(thursday.months_later(2), day(2027, 1, 1));
    assert_eq!(thursday.months_later(-11), day(2025, 12, 1));
    assert_eq!(thursday.iso_midnight(), "2026-11-05T00:00:00Z");
}

#[test]
fn a_day_comes_from_a_moment_or_an_iso_text_and_refuses_what_is_none() {
    let at = Timestamp::from_unix(1_793_869_200, 0).unwrap();
    assert_eq!(Day::of(at, Offset::UTC), Some(day(2026, 11, 5)));
    assert_eq!(Day::of(at, Offset::from_minutes(16 * 60).unwrap()), Some(day(2026, 11, 6)));
    assert_eq!(Day::parse("2026-11-09"), Some(day(2026, 11, 9)));
    assert_eq!(Day::parse("2026-11-09T09:30:00+01:00"), Some(day(2026, 11, 9)));
    for bad in ["", "2026", "2026-02-30", "2026/11/09", "20261109", "2026-1x-09", "+026-11-09", "２０２６-11-09"] {
        assert_eq!(Day::parse(bad), None, "{bad:?}");
    }
    assert_eq!(Day::new(2026, 13, 1), None);
}

#[test]
fn a_day_stays_inside_the_years_the_calendar_knows() {
    let last = day(10000, 12, 31);
    assert_eq!(last.plus(1), last);
    assert_eq!(last.plus(i64::MAX), last);
    let first = day(0, 1, 1);
    assert_eq!(first.plus(-1), first);
    assert_eq!(first.plus(i64::MIN), first);
    assert_eq!(last.months_later(1), last.first_of_month());
    assert_eq!(first.months_later(i32::MIN), first);
}

#[test]
fn the_names_of_days_months_and_spans() {
    assert_eq!(month_name(1), "January");
    assert_eq!(month_name(12), "December");
    assert_eq!(month_name(99), "December");
    assert_eq!((1..=11).map(month_name).collect::<Vec<_>>().len(), 11);
    assert_eq!(weekday_short(Weekday::Monday), "Mon");
    let letters: String = (0..7).map(weekday_letter).collect();
    assert_eq!(letters, "MTWTFSS");
    for weekday in [Weekday::Tuesday, Weekday::Wednesday, Weekday::Thursday, Weekday::Friday, Weekday::Saturday, Weekday::Sunday] {
        assert_eq!(weekday_short(weekday).len(), 3);
    }
    assert_eq!(long(day(2026, 11, 5)), "Thursday 5 November 2026");
    assert_eq!(dated(day(2026, 11, 7)), "Saturday 7 November");
    assert_eq!(span(day(2026, 11, 2), day(2026, 11, 8)), "2 – 8 November 2026");
    assert_eq!(span(day(2026, 11, 30), day(2026, 12, 6)), "30 November – 6 December 2026");
    assert_eq!(span(day(2026, 12, 28), day(2027, 1, 3)), "28 December 2026 – 3 January 2027");
}
