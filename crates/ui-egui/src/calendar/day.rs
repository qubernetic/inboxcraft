// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A day of the calendar, counted from 1 January 1970, with the names the calendar screen gives it. Weeks start on
//! Monday.

use inboxcraft_time::{CivilDateTime, Offset, Timestamp, Weekday};

use crate::tr;

/// The seconds of a day.
const DAY_SECONDS: i64 = 86_400;

/// A day on the computer's wall clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(i64);

impl Day {
    /// 1 January 1970.
    pub const EPOCH: Self = Self(0);

    /// The day `year`-`month`-`day`, or `None` when there is no such day.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        let midnight = CivilDateTime::new(year, month, day, 0, 0, 0)?.to_timestamp(Offset::UTC)?;
        Some(Self(midnight.unix_seconds().div_euclid(DAY_SECONDS)))
    }

    /// The day `at` falls on at `offset`.
    #[must_use]
    pub fn of(at: Timestamp, offset: Offset) -> Option<Self> {
        let civil = at.to_civil(offset)?;
        Self::new(civil.year(), civil.month(), civil.day())
    }

    /// The day an ISO 8601 text starts with (`2026-11-09`, `2026-11-09T09:30:00+01:00`).
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = |from: usize, to: usize| text.get(from..to).filter(|part| part.bytes().all(|b| b.is_ascii_digit()));
        if text.get(4..5) != Some("-") || text.get(7..8) != Some("-") {
            return None;
        }
        Self::new(digits(0, 4)?.parse().ok()?, digits(5, 7)?.parse().ok()?, digits(8, 10)?.parse().ok()?)
    }

    /// The day `days` later (earlier when negative); this day when that leaves the years the calendar knows.
    #[must_use]
    pub fn plus(self, days: i64) -> Self {
        let later = Self(self.0.saturating_add(days));
        if later.civil().is_some() { later } else { self }
    }

    /// How many days `self` is after `earlier` (negative when before).
    #[must_use]
    pub fn since(self, earlier: Self) -> i64 {
        self.0.saturating_sub(earlier.0)
    }

    /// The year.
    #[must_use]
    pub fn year(self) -> i32 {
        self.civil().map_or(1970, CivilDateTime::year)
    }

    /// The month, 1 to 12.
    #[must_use]
    pub fn month(self) -> u8 {
        self.civil().map_or(1, CivilDateTime::month)
    }

    /// The day of the month, from 1.
    #[must_use]
    pub fn date(self) -> u8 {
        self.civil().map_or(1, CivilDateTime::day)
    }

    /// The day of the week.
    #[must_use]
    pub fn weekday(self) -> Weekday {
        self.civil().map_or(Weekday::Thursday, CivilDateTime::weekday)
    }

    /// The days since Monday: 0 on a Monday, 6 on a Sunday.
    #[must_use]
    pub fn weekday_index(self) -> i64 {
        // 1 January 1970 was a Thursday.
        self.0.saturating_add(3).rem_euclid(7)
    }

    /// The Monday of the day's week.
    #[must_use]
    pub fn monday(self) -> Self {
        self.plus(-self.weekday_index())
    }

    /// The first day of the day's month.
    #[must_use]
    pub fn first_of_month(self) -> Self {
        self.plus(1 - i64::from(self.date()))
    }

    /// The first day of the month `months` after the day's month (before when negative).
    #[must_use]
    pub fn months_later(self, months: i32) -> Self {
        let index = self.year().saturating_mul(12).saturating_add(i32::from(self.month()) - 1).saturating_add(months);
        let (year, month) = (index.div_euclid(12), index.rem_euclid(12) + 1);
        u8::try_from(month).ok().and_then(|month| Self::new(year, month, 1)).unwrap_or(self.first_of_month())
    }

    /// The day's midnight in UTC, as the engine reads a moment: `2026-11-09T00:00:00Z`.
    #[must_use]
    pub fn iso_midnight(self) -> String {
        format!("{:04}-{:02}-{:02}T00:00:00Z", self.year(), self.month(), self.date())
    }

    fn civil(self) -> Option<CivilDateTime> {
        Timestamp::from_unix(self.0.checked_mul(DAY_SECONDS)?, 0)?.to_civil(Offset::UTC)
    }
}

/// The name of `month` (1 to 12).
#[must_use]
pub fn month_name(month: u8) -> &'static str {
    match month {
        1 => tr!("January"),
        2 => tr!("February"),
        3 => tr!("March"),
        4 => tr!("April"),
        5 => tr!("May"),
        6 => tr!("June"),
        7 => tr!("July"),
        8 => tr!("August"),
        9 => tr!("September"),
        10 => tr!("October"),
        11 => tr!("November"),
        _ => tr!("December"),
    }
}

/// The short name of `weekday`: `Mon`.
#[must_use]
pub fn weekday_short(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => tr!("Mon"),
        Weekday::Tuesday => tr!("Tue"),
        Weekday::Wednesday => tr!("Wed"),
        Weekday::Thursday => tr!("Thu"),
        Weekday::Friday => tr!("Fri"),
        Weekday::Saturday => tr!("Sat"),
        Weekday::Sunday => tr!("Sun"),
    }
}

/// The letter of the weekday `index` days after Monday, over a column of the mini month.
#[must_use]
pub fn weekday_letter(index: i64) -> &'static str {
    match index {
        0 => tr!("M"),
        1 | 3 => tr!("T"),
        2 => tr!("W"),
        4 => tr!("F"),
        _ => tr!("S"),
    }
}

/// `day` in full: `Thursday 5 November 2026`.
#[must_use]
pub fn long(day: Day) -> String {
    tr!(
        "{weekday} {date} {month} {year}",
        weekday = crate::mailbox::weekday_text(day.weekday()),
        date = day.date(),
        month = month_name(day.month()),
        year = day.year()
    )
}

/// `day` without its year: `Thursday 5 November`.
#[must_use]
pub fn dated(day: Day) -> String {
    tr!("{weekday} {date} {month}", weekday = crate::mailbox::weekday_text(day.weekday()), date = day.date(), month = month_name(day.month()))
}

/// The days `first` to `last` as a title: `2 – 8 November 2026`, `30 November – 6 December 2026`,
/// `28 December 2026 – 3 January 2027`.
#[must_use]
pub fn span(first: Day, last: Day) -> String {
    let (to_month, year) = (month_name(last.month()), last.year());
    if first.year() != last.year() {
        tr!(
            "{from} {from_month} {from_year} – {to} {to_month} {year}",
            from = first.date(),
            from_month = month_name(first.month()),
            from_year = first.year(),
            to = last.date(),
            to_month = to_month,
            year = year
        )
    } else if first.month() != last.month() {
        tr!(
            "{from} {from_month} – {to} {to_month} {year}",
            from = first.date(),
            from_month = month_name(first.month()),
            to = last.date(),
            to_month = to_month,
            year = year
        )
    } else {
        tr!("{from} – {to} {to_month} {year}", from = first.date(), to = last.date(), to_month = to_month, year = year)
    }
}

#[cfg(test)]
#[path = "day_tests.rs"]
mod tests;
