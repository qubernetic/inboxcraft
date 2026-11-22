// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! How the owner's weeks are counted: the first day of the week and the first week of the year, a
//! [`WeekRule`] (Monday and the first four-day week, ISO 8601, by default). `week.settings` (`safe`) shows it and
//! `week.set` (`owner`, a setting) changes it; the front end keeps it on disk ([`to_value`]) and hands it back at the
//! start ([`crate::Session::set_week_rule`]). The calendar's week and "next week" of the time presets count by it.

use inboxcraft_time::{FirstWeek, WeekRule, Weekday};
use serde_json::{Value, json};

/// The names of the days of the week in commands, Monday first.
pub const DAYS: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

/// The names of the first weeks of the year in commands.
pub const FIRST_WEEKS: [&str; 2] = ["first_four_day_week", "week_of_1_january"];

/// The name of `day` in commands: `monday`.
#[must_use]
pub fn day_name(day: Weekday) -> &'static str {
    let index = usize::try_from(day.days_from_monday()).unwrap_or_default();
    DAYS.get(index).copied().unwrap_or("monday")
}

/// The day `name` names, or `None`.
#[must_use]
pub fn day_from_name(name: &str) -> Option<Weekday> {
    Weekday::ALL.into_iter().find(|day| day_name(*day) == name)
}

/// The name of `first` in commands: `first_four_day_week` or `week_of_1_january`.
#[must_use]
pub const fn first_week_name(first: FirstWeek) -> &'static str {
    match first {
        FirstWeek::FirstFourDayWeek => "first_four_day_week",
        FirstWeek::WeekOf1January => "week_of_1_january",
    }
}

/// The first week `name` names, or `None`.
#[must_use]
pub fn first_week_from_name(name: &str) -> Option<FirstWeek> {
    [FirstWeek::FirstFourDayWeek, FirstWeek::WeekOf1January].into_iter().find(|first| first_week_name(*first) == name)
}

/// The rule as JSON, to keep.
///
/// ```
/// use inboxcraft_engine::week::{from_value, to_value};
/// use inboxcraft_time::{FirstWeek, WeekRule, Weekday};
///
/// let rule = WeekRule { first_day: Weekday::Sunday, first_week: FirstWeek::WeekOf1January };
/// assert_eq!(from_value(&to_value(rule)), rule);
/// ```
#[must_use]
pub fn to_value(rule: WeekRule) -> Value {
    json!({ "first_day": day_name(rule.first_day), "first_week": first_week_name(rule.first_week) })
}

/// The rule kept in `value` ([`to_value`]); a part that cannot be read is left at its default, never an error.
#[must_use]
pub fn from_value(value: &Value) -> WeekRule {
    from_value_or(value, WeekRule::default())
}

/// The rule `value` holds, read as outside data as [`from_value`] reads it, with `default` (the locale's, see
/// [`for_locale`]) for what cannot be read.
#[must_use]
pub fn from_value_or(value: &Value, default: WeekRule) -> WeekRule {
    let text = |key: &str| value.get(key).and_then(Value::as_str);
    WeekRule {
        first_day: text("first_day").and_then(day_from_name).unwrap_or(default.first_day),
        first_week: text("first_week").and_then(first_week_from_name).unwrap_or(default.first_week),
    }
}

/// The regions whose weeks start on Sunday and count week 1 as the week that holds 1 January; every other region, and
/// a locale without one, counts from Monday by ISO 8601. A short list: a region not on it is counted the ISO way.
const SUNDAY_REGIONS: [&str; 14] = ["BR", "CA", "HK", "IL", "IN", "JP", "KR", "MO", "MX", "PH", "SA", "TW", "US", "ZA"];

/// The rule a locale such as `en_US.UTF-8` or `en-CA` suggests, by its region; read as outside data: anything but an
/// ASCII region of two capital letters after an ASCII language of two or three small letters is no region, and the
/// default.
#[must_use]
pub fn for_locale(tag: &str) -> WeekRule {
    let tag = tag.split(['.', '@']).next().unwrap_or_default();
    let mut parts = tag.split(['_', '-']);
    let language = parts.next().filter(|language| (2..=3).contains(&language.len()) && language.bytes().all(|byte| byte.is_ascii_lowercase()));
    let region = parts.next().filter(|region| region.len() == 2 && region.bytes().all(|byte| byte.is_ascii_uppercase()));
    let region = language.and(region);
    if region.is_some_and(|region| SUNDAY_REGIONS.contains(&region)) {
        return WeekRule { first_day: inboxcraft_time::Weekday::Sunday, first_week: FirstWeek::WeekOf1January };
    }
    WeekRule::default()
}

#[cfg(test)]
#[path = "week_tests.rs"]
mod tests;
