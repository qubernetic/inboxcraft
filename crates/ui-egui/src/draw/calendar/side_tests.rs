// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mini month's columns follow the owner's week rule: its first cell and the letters over its columns.

use inboxcraft_time::{FirstWeek, WeekRule, Weekday};

use super::mini_columns;
use crate::calendar::Day;

fn day(year: i32, month: u8, date: u8) -> Day {
    Day::new(year, month, date).unwrap()
}

#[test]
fn the_mini_month_starts_its_columns_on_the_week_s_first_day() {
    // January 2027 starts on a Friday, January 2033 on a Saturday; the first week counts for no column.
    for first_week in [FirstWeek::FirstFourDayWeek, FirstWeek::WeekOf1January] {
        for (month, first_day, cell, letters) in [
            (day(2027, 1, 1), Weekday::Monday, day(2026, 12, 28), "MTWTFSS"),
            (day(2027, 1, 1), Weekday::Sunday, day(2026, 12, 27), "SMTWTFS"),
            (day(2027, 1, 1), Weekday::Saturday, day(2026, 12, 26), "SSMTWTF"),
            (day(2033, 1, 1), Weekday::Monday, day(2032, 12, 27), "MTWTFSS"),
            (day(2033, 1, 1), Weekday::Sunday, day(2032, 12, 26), "SMTWTFS"),
            (day(2033, 1, 1), Weekday::Saturday, day(2033, 1, 1), "SSMTWTF"),
            (day(2032, 12, 1), Weekday::Saturday, day(2032, 11, 27), "SSMTWTF"),
        ] {
            let (first, shown) = mini_columns(month, WeekRule { first_day, first_week });
            assert_eq!(first, cell, "{month:?} {first_day:?}");
            assert_eq!(shown.concat(), letters, "{month:?} {first_day:?}");
        }
    }
}
