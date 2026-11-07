// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Calendar dates and wall-clock times at a fixed offset from UTC, proleptic Gregorian.

use crate::Timestamp;

const SECS_PER_DAY: i64 = 86_400;
/// Days from 0000-03-01 to 1970-01-01.
const EPOCH_SHIFT: i64 = 719_468;
/// Days in a 400-year cycle.
const DAYS_PER_ERA: i64 = 146_097;

/// The first and last year a [`CivilDateTime`] can hold: the years [`Timestamp`] covers.
const MIN_YEAR: i32 = 0;
const MAX_YEAR: i32 = 10_000;

/// A fixed distance from UTC, from -99:59 to +99:59 (the widest RFC 5322 can write).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Offset {
    minutes: i16,
}

impl Offset {
    /// UTC itself.
    pub const UTC: Self = Self { minutes: 0 };

    /// The offset `minutes` east of UTC (west when negative), or `None` beyond 99 hours and 59 minutes.
    #[must_use]
    pub const fn from_minutes(minutes: i16) -> Option<Self> {
        if minutes < -5999 || minutes > 5999 {
            return None;
        }
        Some(Self { minutes })
    }

    /// Minutes east of UTC; negative is west.
    #[must_use]
    pub const fn minutes(self) -> i16 {
        self.minutes
    }

    const fn seconds(self) -> i64 {
        // Widening, in range by construction.
        self.minutes as i64 * 60
    }
}

/// A day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

/// A date and a wall-clock time to the second, without a zone: what a clock on the wall shows somewhere.
///
/// Every value is a real date: the constructor refuses 30 February, hour 24 and the like. Years run from 0 to 10000;
/// leap seconds do not exist here (a parser that meets second 60 decides what it means).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDateTime {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl CivilDateTime {
    /// The given date and time, or `None` when it does not exist.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        let valid = (MIN_YEAR..=MAX_YEAR).contains(&year) && day >= 1 && day <= days_in_month(year, month) && hour < 24 && minute < 60 && second < 60;
        valid.then_some(Self { year, month, day, hour, minute, second })
    }

    /// The year, 0 to 10000.
    #[must_use]
    pub const fn year(self) -> i32 {
        self.year
    }

    /// The month, 1 to 12.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    /// The day of the month, from 1.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }

    /// The hour, 0 to 23.
    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// The minute, 0 to 59.
    #[must_use]
    pub const fn minute(self) -> u8 {
        self.minute
    }

    /// The second, 0 to 59.
    #[must_use]
    pub const fn second(self) -> u8 {
        self.second
    }

    /// The day of the week of the date.
    #[must_use]
    pub fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday.
        match days_from_civil(self.year, self.month, self.day).rem_euclid(7) {
            0 => Weekday::Thursday,
            1 => Weekday::Friday,
            2 => Weekday::Saturday,
            3 => Weekday::Sunday,
            4 => Weekday::Monday,
            5 => Weekday::Tuesday,
            _ => Weekday::Wednesday,
        }
    }

    /// The moment this wall-clock time shows at `offset`, or `None` outside the range of [`Timestamp`].
    #[must_use]
    pub fn to_timestamp(self, offset: Offset) -> Option<Timestamp> {
        let days = days_from_civil(self.year, self.month, self.day);
        let time = i64::from(self.hour) * 3600 + i64::from(self.minute) * 60 + i64::from(self.second);
        let local = days.checked_mul(SECS_PER_DAY)?.checked_add(time)?;
        Timestamp::from_unix(local.checked_sub(offset.seconds())?, 0)
    }
}

impl Timestamp {
    /// What a wall clock at `offset` shows at this moment, or `None` when that falls outside years 0 to 10000. The
    /// nanoseconds are dropped.
    #[must_use]
    pub fn to_civil(self, offset: Offset) -> Option<CivilDateTime> {
        let local = self.unix_seconds().checked_add(offset.seconds())?;
        let days = local.div_euclid(SECS_PER_DAY);
        let time = local.rem_euclid(SECS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        let hour = u8::try_from(time / 3600).ok()?;
        let minute = u8::try_from(time % 3600 / 60).ok()?;
        let second = u8::try_from(time % 60).ok()?;
        CivilDateTime::new(i32::try_from(year).ok()?, month, day, hour, minute, second)
    }
}

/// Whether `year` has 29 February.
#[must_use]
pub const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// The number of days in `month` (1 to 12) of `year`; 0 for a month that does not exist.
#[must_use]
pub const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

// The two conversions count in 400-year eras of 146097 days from 0000-03-01, so a leap day is the last day of its
// year. Callers pass a valid date (from `CivilDateTime`) or a day number within a few million days of the epoch,
// so the plain arithmetic below stays far from overflow.

/// Days since 1970-01-01 of a valid date.
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = (i64::from(month) + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_PER_ERA + day_of_era - EPOCH_SHIFT
}

/// The date `days` after 1970-01-01, for `days` within the range of [`Timestamp`] plus a few days.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let days = days + EPOCH_SHIFT;
    let era = days.div_euclid(DAYS_PER_ERA);
    let day_of_era = days - era * DAYS_PER_ERA;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    // `month` is 1 to 12 and `day` 1 to 31 by the arithmetic above.
    (year, u8::try_from(month).unwrap_or(0), u8::try_from(day).unwrap_or(0))
}

#[cfg(test)]
#[path = "civil_tests.rs"]
mod tests;
