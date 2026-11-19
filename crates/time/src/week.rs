// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weeks as a region counts them: the day a week starts on and which week is the year's first.
//!
//! With Monday and [`FirstWeek::FirstFourDayWeek`] (the default) this is ISO 8601: week 1 holds the year's first
//! Thursday, and the last days of December or the first of January can belong to the week-year next door.

use crate::{CivilDateTime, Weekday};

impl Weekday {
    /// Every day of the week, Monday first.
    pub const ALL: [Self; 7] = [Self::Monday, Self::Tuesday, Self::Wednesday, Self::Thursday, Self::Friday, Self::Saturday, Self::Sunday];

    /// Days after Monday: 0 for Monday to 6 for Sunday.
    #[must_use]
    pub const fn days_from_monday(self) -> i64 {
        match self {
            Self::Monday => 0,
            Self::Tuesday => 1,
            Self::Wednesday => 2,
            Self::Thursday => 3,
            Self::Friday => 4,
            Self::Saturday => 5,
            Self::Sunday => 6,
        }
    }
}

/// Which week is week 1 of a year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FirstWeek {
    /// The first week with at least four days in the year, the one that holds 4 January (ISO 8601 with Monday).
    #[default]
    FirstFourDayWeek,
    /// The week that holds 1 January.
    WeekOf1January,
}

/// How weeks are counted: the day they start on and the year's first week. The default is ISO 8601.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeekRule {
    /// The day a week starts on.
    pub first_day: Weekday,
    /// Which week is week 1.
    pub first_week: FirstWeek,
}

impl Default for WeekRule {
    fn default() -> Self {
        Self { first_day: Weekday::Monday, first_week: FirstWeek::FirstFourDayWeek }
    }
}

/// The week a date is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Week {
    /// The week's first day, at 00:00.
    pub first_day: CivilDateTime,
    /// The week's number in its week-year, 1 to 53.
    pub number: u8,
    /// The year the week is counted in: the date's own year, or the one before or after it at the year's turn.
    pub year: i32,
}

impl WeekRule {
    /// The week `date` is in, or `None` when the week's first day falls before year 0 (or after year 10000): out of
    /// range is an answer of its own, never a date moved to fit. The time of day of `date` does not matter.
    ///
    /// ```
    /// use inboxcraft_time::{CivilDateTime, WeekRule};
    ///
    /// // 1 January 2021 is a Friday: ISO 8601 counts it in week 53 of 2020.
    /// let week = WeekRule::default().week(CivilDateTime::new(2021, 1, 1, 12, 0, 0).unwrap()).unwrap();
    /// assert_eq!((week.number, week.year), (53, 2020));
    /// assert_eq!(week.first_day, CivilDateTime::new(2020, 12, 28, 0, 0, 0).unwrap());
    /// ```
    #[must_use]
    pub fn week(self, date: CivilDateTime) -> Option<Week> {
        let day = date.day_number();
        let start = self.start_of(day);
        let year = date.year();
        let (week_year, first) = [year.checked_add(1)?, year, year.checked_sub(1)?]
            .into_iter()
            .map(|year| (year, self.week_one(year)))
            .find(|(_, first)| start >= *first)?;
        let number = u8::try_from(start.checked_sub(first)?.div_euclid(7).checked_add(1)?).ok()?;
        Some(Week { first_day: CivilDateTime::from_day_number(start)?, number, year: week_year })
    }

    /// The first day of the week `date` is in, at 00:00; `None` before year 0.
    #[must_use]
    pub fn first_day_of(self, date: CivilDateTime) -> Option<CivilDateTime> {
        CivilDateTime::from_day_number(self.start_of(date.day_number()))
    }

    /// The day number the week holding day number `day` starts on.
    fn start_of(self, day: i64) -> i64 {
        // 1970-01-01 (day 0) was a Thursday, three days after a Monday.
        let from_monday = day.saturating_add(3).rem_euclid(7);
        day.saturating_sub((from_monday - self.first_day.days_from_monday()).rem_euclid(7))
    }

    /// The day number week 1 of `year` starts on; any year, as plain arithmetic.
    fn week_one(self, year: i32) -> i64 {
        let first_january = crate::civil::day_number_of(year, 1, 1);
        match self.first_week {
            FirstWeek::FirstFourDayWeek => self.start_of(first_january.saturating_add(3)),
            FirstWeek::WeekOf1January => self.start_of(first_january),
        }
    }
}

#[cfg(test)]
#[path = "week_tests.rs"]
mod tests;
