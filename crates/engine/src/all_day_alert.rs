// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The hour an all-day event's alert rings, a setting (`event.all_day_alert.set`): the alarm a new all-day event gets
//! when the owner picks none, and the alert an all-day event without any alarm rings here. Under RFC 5545 a whole day
//! starts at midnight, so the alarm is written as a trigger from that midnight (`-PT15H` rings at 09:00 the day
//! before); this computer rings an all-day event's alarms on its own clock, so the hour holds on the days the clock
//! is put forward or back. Events already written keep their alarms. The front end keeps the choice beside the week
//! rule and hands it back at the start ([`from_value`]); what cannot be read is the default.

use serde_json::{Value, json};

use inboxcraft_ical::Span;

/// When an all-day event's alert rings, on the owner's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AllDayAlert {
    /// At 09:00 the day before.
    #[default]
    DayBefore9,
    /// At 12:00 the day before.
    DayBefore12,
    /// At 18:00 the day before.
    DayBefore18,
    /// At 09:00 on the day itself.
    OnTheDay9,
}

/// The key the choice is kept under, beside the week rule.
pub const KEY: &str = "all_day_alert";

impl AllDayAlert {
    /// Every choice, in the order the setting lists them.
    pub const ALL: [Self; 4] = [Self::DayBefore9, Self::DayBefore12, Self::DayBefore18, Self::OnTheDay9];

    /// The name commands and the kept file use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DayBefore9 => "day_before_9",
            Self::DayBefore12 => "day_before_12",
            Self::DayBefore18 => "day_before_18",
            Self::OnTheDay9 => "on_the_day_9",
        }
    }

    /// The choice `name` names, or `None`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.name() == name)
    }

    /// Seconds from the start of the day to when the alert rings: negative the day before.
    #[must_use]
    pub const fn from_start(self) -> i64 {
        match self {
            Self::DayBefore9 => -15 * 3600,
            Self::DayBefore12 => -12 * 3600,
            Self::DayBefore18 => -6 * 3600,
            Self::OnTheDay9 => 9 * 3600,
        }
    }

    /// The alarm's trigger from the start of the day, as written (`-PT15H` for 09:00 the day before).
    pub(crate) fn span(self) -> Option<Span> {
        Span::new(0, self.from_start())
    }
}

/// The choice as JSON, to keep beside the week rule.
///
/// ```
/// use inboxcraft_engine::all_day_alert::{AllDayAlert, from_value, to_value};
///
/// assert_eq!(from_value(&to_value(AllDayAlert::DayBefore18)), AllDayAlert::DayBefore18);
/// ```
#[must_use]
pub fn to_value(choice: AllDayAlert) -> Value {
    json!({ KEY: choice.name() })
}

/// The choice kept in `value` under [`KEY`], read as outside data: a file without it, or with anything else there,
/// is the default, never an error.
#[must_use]
pub fn from_value(value: &Value) -> AllDayAlert {
    value.get(KEY).and_then(Value::as_str).and_then(AllDayAlert::from_name).unwrap_or_default()
}

#[cfg(test)]
#[path = "all_day_alert_tests.rs"]
mod tests;
