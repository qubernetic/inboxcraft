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
    let default = WeekRule::default();
    let text = |key: &str| value.get(key).and_then(Value::as_str);
    WeekRule {
        first_day: text("first_day").and_then(day_from_name).unwrap_or(default.first_day),
        first_week: text("first_week").and_then(first_week_from_name).unwrap_or(default.first_week),
    }
}

#[cfg(test)]
#[path = "week_tests.rs"]
mod tests;
