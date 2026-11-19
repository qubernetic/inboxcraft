// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The week of a date under each rule, as tables: the year's turn, leap years and the ends of the years held.

use super::{FirstWeek, Week, WeekRule};
use crate::{CivilDateTime, Weekday};

fn date(year: i32, month: u8, day: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, 0, 0, 0).unwrap()
}

fn rule(first_day: Weekday, first_week: FirstWeek) -> WeekRule {
    WeekRule { first_day, first_week }
}

/// `(date, first day of its week, number, week-year)`.
type Row = ((i32, u8, u8), (i32, u8, u8), u8, i32);

fn check(rule: WeekRule, rows: &[Row]) {
    for &((y, m, d), (fy, fm, fd), number, year) in rows {
        // The time of day does not matter.
        let at = CivilDateTime::new(y, m, d, 23, 59, 59).unwrap();
        let expected = Week { first_day: date(fy, fm, fd), number, year };
        assert_eq!(rule.week(at), Some(expected), "{y}-{m:02}-{d:02} under {rule:?}");
        assert_eq!(rule.first_day_of(at), Some(date(fy, fm, fd)), "{y}-{m:02}-{d:02} under {rule:?}");
    }
}

#[test]
fn the_default_is_iso_8601() {
    assert_eq!(WeekRule::default(), rule(Weekday::Monday, FirstWeek::FirstFourDayWeek));
}

#[test]
fn iso_weeks_at_the_turn_of_the_year_with_monday() {
    check(
        WeekRule::default(),
        &[
            ((2020, 12, 27), (2020, 12, 21), 52, 2020),
            ((2020, 12, 28), (2020, 12, 28), 53, 2020),
            ((2020, 12, 31), (2020, 12, 28), 53, 2020),
            ((2021, 1, 1), (2020, 12, 28), 53, 2020),
            ((2021, 1, 3), (2020, 12, 28), 53, 2020),
            ((2021, 1, 4), (2021, 1, 4), 1, 2021),
            ((2021, 1, 10), (2021, 1, 4), 1, 2021),
            ((2026, 12, 27), (2026, 12, 21), 52, 2026),
            ((2026, 12, 31), (2026, 12, 28), 53, 2026),
            ((2027, 1, 1), (2026, 12, 28), 53, 2026),
            ((2027, 1, 3), (2026, 12, 28), 53, 2026),
            ((2027, 1, 4), (2027, 1, 4), 1, 2027),
            ((2032, 12, 26), (2032, 12, 20), 52, 2032),
            ((2032, 12, 31), (2032, 12, 27), 53, 2032),
            ((2033, 1, 1), (2032, 12, 27), 53, 2032),
            ((2033, 1, 2), (2032, 12, 27), 53, 2032),
            ((2033, 1, 3), (2033, 1, 3), 1, 2033),
            // A December day in week 1 of the next year.
            ((2024, 12, 31), (2024, 12, 30), 1, 2025),
            ((2025, 1, 1), (2024, 12, 30), 1, 2025),
        ],
    );
}

#[test]
fn the_same_dates_with_sunday_first() {
    check(
        rule(Weekday::Sunday, FirstWeek::FirstFourDayWeek),
        &[
            ((2020, 12, 26), (2020, 12, 20), 52, 2020),
            ((2020, 12, 27), (2020, 12, 27), 53, 2020),
            ((2021, 1, 1), (2020, 12, 27), 53, 2020),
            ((2021, 1, 2), (2020, 12, 27), 53, 2020),
            ((2021, 1, 3), (2021, 1, 3), 1, 2021),
            ((2021, 1, 10), (2021, 1, 10), 2, 2021),
            ((2026, 12, 26), (2026, 12, 20), 51, 2026),
            ((2026, 12, 27), (2026, 12, 27), 52, 2026),
            ((2027, 1, 1), (2026, 12, 27), 52, 2026),
            ((2027, 1, 3), (2027, 1, 3), 1, 2027),
            ((2032, 12, 26), (2032, 12, 26), 52, 2032),
            ((2033, 1, 1), (2032, 12, 26), 52, 2032),
            ((2033, 1, 2), (2033, 1, 2), 1, 2033),
            ((2033, 1, 9), (2033, 1, 9), 2, 2033),
        ],
    );
}

#[test]
fn the_same_dates_with_saturday_first() {
    check(
        rule(Weekday::Saturday, FirstWeek::FirstFourDayWeek),
        &[
            ((2020, 12, 26), (2020, 12, 26), 52, 2020),
            ((2021, 1, 1), (2020, 12, 26), 52, 2020),
            ((2021, 1, 2), (2021, 1, 2), 1, 2021),
            ((2021, 1, 10), (2021, 1, 9), 2, 2021),
            ((2026, 12, 31), (2026, 12, 26), 52, 2026),
            ((2027, 1, 1), (2026, 12, 26), 52, 2026),
            ((2027, 1, 2), (2027, 1, 2), 1, 2027),
            ((2032, 12, 31), (2032, 12, 25), 52, 2032),
            ((2033, 1, 1), (2033, 1, 1), 1, 2033),
            ((2033, 1, 9), (2033, 1, 8), 2, 2033),
        ],
    );
}

#[test]
fn the_week_of_1_january_for_each_day_it_falls_on() {
    // 1 January 2017 is a Sunday, 2018 a Monday, 2019 a Tuesday, 2020 a Wednesday, 2026 a Thursday, 2021 a Friday,
    // 2022 a Saturday.
    check(
        rule(Weekday::Sunday, FirstWeek::WeekOf1January),
        &[
            ((2017, 1, 1), (2017, 1, 1), 1, 2017),
            ((2016, 12, 31), (2016, 12, 25), 53, 2016),
            ((2018, 1, 1), (2017, 12, 31), 1, 2018),
            ((2017, 12, 31), (2017, 12, 31), 1, 2018),
            ((2019, 1, 1), (2018, 12, 30), 1, 2019),
            ((2020, 1, 1), (2019, 12, 29), 1, 2020),
            ((2026, 1, 1), (2025, 12, 28), 1, 2026),
            ((2025, 12, 31), (2025, 12, 28), 1, 2026),
            ((2021, 1, 1), (2020, 12, 27), 1, 2021),
            ((2020, 12, 31), (2020, 12, 27), 1, 2021),
            ((2022, 1, 1), (2021, 12, 26), 1, 2022),
            ((2022, 12, 31), (2022, 12, 25), 53, 2022),
            ((2023, 1, 1), (2023, 1, 1), 1, 2023),
        ],
    );
    check(
        rule(Weekday::Monday, FirstWeek::WeekOf1January),
        &[
            ((2017, 1, 1), (2016, 12, 26), 1, 2017),
            ((2018, 1, 1), (2018, 1, 1), 1, 2018),
            ((2017, 12, 31), (2017, 12, 25), 53, 2017),
            ((2019, 1, 1), (2018, 12, 31), 1, 2019),
            ((2018, 12, 31), (2018, 12, 31), 1, 2019),
            ((2020, 1, 1), (2019, 12, 30), 1, 2020),
            ((2026, 1, 1), (2025, 12, 29), 1, 2026),
            ((2021, 1, 1), (2020, 12, 28), 1, 2021),
            ((2022, 1, 1), (2021, 12, 27), 1, 2022),
            ((2023, 1, 1), (2022, 12, 26), 1, 2023),
        ],
    );
}

#[test]
fn leap_years() {
    check(
        WeekRule::default(),
        &[((2024, 2, 29), (2024, 2, 26), 9, 2024), ((2000, 2, 29), (2000, 2, 28), 9, 2000), ((2100, 3, 1), (2100, 3, 1), 9, 2100)],
    );
    check(
        rule(Weekday::Sunday, FirstWeek::WeekOf1January),
        &[
            ((2024, 2, 29), (2024, 2, 25), 9, 2024),
            ((2000, 2, 29), (2000, 2, 27), 10, 2000),
            ((2100, 3, 1), (2100, 2, 28), 10, 2100),
            ((2000, 12, 31), (2000, 12, 31), 1, 2001),
        ],
    );
}

#[test]
fn at_the_ends_of_the_years_held_out_of_range_is_none_never_a_moved_date() {
    // 1 January of year 0 is a Saturday: its Monday week starts in year -1, which is not held.
    assert_eq!(WeekRule::default().week(date(0, 1, 1)), None);
    assert_eq!(WeekRule::default().first_day_of(date(0, 1, 2)), None);
    check(WeekRule::default(), &[((0, 1, 3), (0, 1, 3), 1, 0)]);
    check(rule(Weekday::Saturday, FirstWeek::FirstFourDayWeek), &[((0, 1, 1), (0, 1, 1), 1, 0)]);
    // 31 December 10000 is a Sunday.
    check(WeekRule::default(), &[((10_000, 12, 31), (10_000, 12, 25), 52, 10_000)]);
    // With Sunday and the week of 1 January it opens week 1 of 10001: the number is still right.
    check(rule(Weekday::Sunday, FirstWeek::WeekOf1January), &[((10_000, 12, 31), (10_000, 12, 31), 1, 10_001)]);
}

#[test]
fn every_rule_on_every_day_of_a_span_counts_weeks_one_by_one() {
    for first_day in Weekday::ALL {
        for first_week in [FirstWeek::FirstFourDayWeek, FirstWeek::WeekOf1January] {
            let rule = rule(first_day, first_week);
            let mut day = date(2015, 12, 1);
            let mut last: Option<Week> = None;
            while day < date(2034, 2, 1) {
                let week = rule.week(day).unwrap();
                assert_eq!(week.first_day.weekday(), first_day, "{day:?}");
                assert!(week.first_day <= day && day < week.first_day.plus_days(7).unwrap(), "{day:?}");
                assert!((1..=53).contains(&week.number), "{day:?}");
                if let Some(last) = last.filter(|last| last.first_day != week.first_day) {
                    let next = if week.year == last.year { last.number + 1 } else { 1 };
                    assert_eq!(week.number, next, "{day:?} under {rule:?}");
                }
                last = Some(week);
                day = day.plus_days(1).unwrap();
            }
        }
    }
}

#[test]
fn plus_days_steps_over_months_and_years_and_stops_at_the_ends() {
    assert_eq!(date(2026, 12, 31).plus_days(1), Some(date(2027, 1, 1)));
    assert_eq!(date(2024, 2, 28).plus_days(1), Some(date(2024, 2, 29)));
    assert_eq!(date(2024, 3, 1).plus_days(-1), Some(date(2024, 2, 29)));
    let nine = CivilDateTime::new(2026, 11, 19, 9, 30, 15).unwrap();
    assert_eq!(nine.plus_days(7), CivilDateTime::new(2026, 11, 26, 9, 30, 15));
    assert_eq!(date(10_000, 12, 31).plus_days(1), None);
    assert_eq!(date(0, 1, 1).plus_days(-1), None);
    // 1 January 2026 is day 20454: the third step would overflow the day arithmetic without the bound.
    for days in [i64::MAX, i64::MIN, i64::MAX - 120_454, i64::MIN + 1_000_000, 4_000_001, -4_000_001] {
        assert_eq!(date(2026, 1, 1).plus_days(days), None, "{days}");
    }
}

#[test]
fn the_days_of_the_week_count_from_monday() {
    assert_eq!(Weekday::ALL.map(Weekday::days_from_monday), [0, 1, 2, 3, 4, 5, 6]);
}
