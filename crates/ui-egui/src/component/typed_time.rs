// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A time the person types into the time picker, and the words the picker shows a time in.
//!
//! [`parse`] reads English words in any case, separated by spaces, and always gives a time ahead of now, on the wall
//! clock of the given offset:
//!
//! - a time: `9`, `17:30`, `17.30`, `5pm`, `5:15 am`, `noon`, `midnight` (today while it is ahead, else tomorrow);
//! - a day, with a time or else at 08:00: `today`, `tomorrow` (`tmrw`), a weekday (`thu`, `thurs`, `thursday`: the
//!   next one, today's while its time is ahead), a day and a month (`12 nov`, `nov 12`: the next one) or an ISO date
//!   (`2026-11-20`);
//! - a span from now, to the minute: `in 3 days`, `in 2 hours`, `in 45 min`, `in 1 week`, or `3d`, `2h`, `45m`, `1w`.
//!
//! Anything else, and a time that is not ahead, is nothing. The text is hostile: it is read only up to
//! [`MAX_TYPED`] bytes and six words, every number is checked and nothing is indexed.

use std::time::Duration;

use inboxcraft_time::{CivilDateTime, Offset, Timestamp, Weekday};

use crate::mailbox::{day_number, month_text, weekday_text};
use crate::tr;

/// The longest text read, in bytes; longer text is nothing.
pub const MAX_TYPED: usize = 64;

/// The most words read; more is nothing.
const MAX_WORDS: usize = 6;

/// The hour of a day named without a time: its morning.
const MORNING: u8 = 8;

const DAY: u64 = 86_400;

const WEEKDAYS: [(&str, Weekday); 7] = [
    ("monday", Weekday::Monday),
    ("tuesday", Weekday::Tuesday),
    ("wednesday", Weekday::Wednesday),
    ("thursday", Weekday::Thursday),
    ("friday", Weekday::Friday),
    ("saturday", Weekday::Saturday),
    ("sunday", Weekday::Sunday),
];

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

/// The day a text names.
#[derive(Debug, Clone, Copy)]
enum Day {
    Today,
    Tomorrow,
    Weekday(Weekday),
    /// A day of a month, in whichever year it is next ahead.
    Date {
        month: u8,
        day: u8,
    },
    Iso {
        year: i32,
        month: u8,
        day: u8,
    },
}

/// The time `text` names at `now` on the wall clock of `offset`, always after `now`; `None` when it names none.
#[must_use]
pub fn parse(text: &str, now: Timestamp, offset: Offset) -> Option<Timestamp> {
    if text.len() > MAX_TYPED {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower.split_whitespace().take(MAX_WORDS + 1).collect();
    if words.is_empty() || words.len() > MAX_WORDS {
        return None;
    }
    if let Some(span) = span(&words) {
        let minute = now.unix_seconds().checked_sub(now.unix_seconds().rem_euclid(60))?;
        return Timestamp::from_unix(minute, 0)?.checked_add(span);
    }
    let (day, time) = parts(&words)?;
    resolve(day, time, now, offset)
}

/// `in 3 days` or `3d`: the span it names.
fn span(words: &[&str]) -> Option<Duration> {
    let (count, unit) = match words {
        ["in", count, unit] => (*count, *unit),
        [one] => one.split_at(one.find(|c: char| !c.is_ascii_digit())?),
        _ => return None,
    };
    let count = number(count).filter(|count| *count > 0)?;
    let unit = match unit {
        "m" | "min" | "mins" | "minute" | "minutes" => 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3_600,
        "d" | "day" | "days" => DAY,
        "w" | "wk" | "wks" | "week" | "weeks" => DAY * 7,
        _ => return None,
    };
    count.checked_mul(unit).map(Duration::from_secs)
}

/// One to six ASCII digits as a number.
fn number(text: &str) -> Option<u64> {
    (!text.is_empty() && text.len() <= 6 && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().ok()).flatten()
}

/// An hour and a minute.
type Clock = (u8, u8);

/// The day and the time the words name, each at most once.
fn parts(words: &[&str]) -> Option<(Option<Day>, Option<Clock>)> {
    // `5 pm` is `5pm`.
    let mut tokens: Vec<String> = Vec::with_capacity(words.len());
    for word in words {
        match tokens.last_mut() {
            Some(last) if matches!(*word, "am" | "pm") => last.push_str(word),
            _ => tokens.push((*word).to_owned()),
        }
    }
    let mut day = None;
    let mut time = None;
    let mut month = None;
    let mut bare = Vec::new();
    for token in &tokens {
        let token = token.as_str();
        if let Some(named) = day_word(token) {
            once(&mut day, named)?;
        } else if let Some(named) = month_word(token) {
            once(&mut month, named)?;
        } else if let Some(named) = clock(token) {
            once(&mut time, named)?;
        } else if token.len() <= 2 && number(token).is_some() {
            bare.push(token);
        } else {
            return None;
        }
    }
    let mut bare = bare.into_iter().filter_map(|token| u8::try_from(number(token)?).ok());
    if let Some(month) = month {
        once(&mut day, Day::Date { month, day: bare.next()? })?;
    }
    if let Some(hour) = bare.next() {
        once(&mut time, (hour, 0)).filter(|()| hour < 24)?;
    }
    if bare.next().is_some() {
        return None;
    }
    Some((day, time))
}

/// Puts `value` into an empty `slot`; `None` when it was already filled.
fn once<T>(slot: &mut Option<T>, value: T) -> Option<()> {
    match slot {
        Some(_) => None,
        None => {
            *slot = Some(value);
            Some(())
        }
    }
}

/// `today`, `tomorrow`, a weekday or an ISO date.
fn day_word(token: &str) -> Option<Day> {
    match token {
        "today" => return Some(Day::Today),
        "tomorrow" | "tmrw" => return Some(Day::Tomorrow),
        _ => {}
    }
    if token.len() >= 3
        && let Some((_, weekday)) = WEEKDAYS.iter().find(|(name, _)| name.starts_with(token))
    {
        return Some(Day::Weekday(*weekday));
    }
    let mut fields = token.split('-');
    let (Some(year), Some(month), Some(day), None) = (fields.next(), fields.next(), fields.next(), fields.next()) else { return None };
    if year.len() != 4 || month.len() > 2 || day.len() > 2 {
        return None;
    }
    let small = |text: &str| u8::try_from(number(text)?).ok();
    Some(Day::Iso { year: i32::try_from(number(year)?).ok()?, month: small(month)?, day: small(day)? })
}

/// A month's name or its first three letters or more: its number.
fn month_word(token: &str) -> Option<u8> {
    if token.len() < 3 {
        return None;
    }
    let index = MONTHS.iter().position(|name| name.starts_with(token))?;
    u8::try_from(index).ok()?.checked_add(1)
}

/// `17:30`, `17.30`, `5pm`, `5:15am`, `noon` or `midnight`: the hour and minute. A bare number is not a clock here.
fn clock(token: &str) -> Option<Clock> {
    match token {
        "noon" => return Some((12, 0)),
        "midnight" => return Some((0, 0)),
        _ => {}
    }
    let (core, meridiem) = match token.strip_suffix("am") {
        Some(core) => (core, Some(false)),
        None => match token.strip_suffix("pm") {
            Some(core) => (core, Some(true)),
            None => (token, None),
        },
    };
    let (hour, minute) = match core.split_once([':', '.']) {
        Some((hour, minute)) if minute.len() == 2 => (hour, number(minute)?),
        Some(_) => return None,
        None if meridiem.is_some() => (core, 0),
        None => return None,
    };
    let hour = u8::try_from(number(hour).filter(|_| hour.len() <= 2)?).ok()?;
    let minute = u8::try_from(minute).ok().filter(|minute| *minute < 60)?;
    let hour = match meridiem {
        None if hour < 24 => hour,
        Some(pm) if (1..=12).contains(&hour) => hour % 12 + if pm { 12 } else { 0 },
        _ => return None,
    };
    Some((hour, minute))
}

/// The date `days` after `date`.
fn after(date: CivilDateTime, days: u64) -> Option<CivilDateTime> {
    let noon = CivilDateTime::new(date.year(), date.month(), date.day(), 12, 0, 0)?.to_timestamp(Offset::UTC)?;
    noon.checked_add(Duration::from_secs(days.checked_mul(DAY)?))?.to_civil(Offset::UTC)
}

/// The day and time named, made a time after `now` on the wall clock of `offset`.
fn resolve(day: Option<Day>, time: Option<Clock>, now: Timestamp, offset: Offset) -> Option<Timestamp> {
    let today = now.to_civil(offset)?;
    let (hour, minute) = time.unwrap_or((MORNING, 0));
    let ahead = |year: i32, month: u8, day: u8| CivilDateTime::new(year, month, day, hour, minute, 0)?.to_timestamp(offset).filter(|at| *at > now);
    let on = |date: CivilDateTime| ahead(date.year(), date.month(), date.day());
    match day {
        None if time.is_some() => (0..=1).find_map(|days| on(after(today, days)?)),
        None => None,
        Some(Day::Today) => on(today),
        Some(Day::Tomorrow) => on(after(today, 1)?),
        Some(Day::Weekday(weekday)) => (0..=7).find_map(|days| after(today, days).filter(|date| date.weekday() == weekday).and_then(on)),
        // Eight years cover the longest wait for a 29 February (2096 to 2104).
        Some(Day::Date { month, day }) => (0..=8).find_map(|years| ahead(today.year().checked_add(years)?, month, day)),
        Some(Day::Iso { year, month, day }) => ahead(year, month, day),
    }
}

fn month_name(month: u8) -> &'static str {
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

fn weekday_short(weekday: Weekday) -> &'static str {
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

fn clock_text(civil: CivilDateTime) -> String {
    format!("{:02}:{:02}", civil.hour(), civil.minute())
}

/// `at` in full on the wall clock of `offset`, the year only when it is not `now`'s: "Thursday 12 November, 09:00".
#[must_use]
pub fn long(at: Timestamp, now: Timestamp, offset: Offset) -> String {
    let (Some(civil), Some(today)) = (at.to_civil(offset), now.to_civil(offset)) else { return String::new() };
    let weekday = weekday_text(civil.weekday());
    let month = month_name(civil.month());
    let time = clock_text(civil);
    if civil.year() == today.year() {
        tr!("{weekday} {day} {month}, {time}", weekday = weekday, day = civil.day(), month = month, time = time)
    } else {
        tr!("{weekday} {day} {month} {year}, {time}", weekday = weekday, day = civil.day(), month = month, year = civil.year(), time = time)
    }
}

/// `at` short beside a preset: the time today, the weekday and time within six days, else the weekday and the date,
/// with the year when it is not `now`'s.
#[must_use]
pub fn short(at: Timestamp, now: Timestamp, offset: Offset) -> String {
    let (Some(civil), Some(today)) = (at.to_civil(offset), now.to_civil(offset)) else { return String::new() };
    let days = day_number(civil).zip(day_number(today)).map_or(i64::MAX, |(day, today)| day.saturating_sub(today));
    let weekday = weekday_short(civil.weekday());
    let month = month_text(civil.month());
    match days {
        0 => clock_text(civil),
        1..=6 => tr!("{weekday} {time}", weekday = weekday, time = clock_text(civil)),
        _ if civil.year() == today.year() => tr!("{weekday} {day} {month}", weekday = weekday, day = civil.day(), month = month),
        _ => tr!("{weekday} {day} {month} {year}", weekday = weekday, day = civil.day(), month = month, year = civil.year()),
    }
}

#[cfg(test)]
#[path = "typed_time_tests.rs"]
mod tests;
