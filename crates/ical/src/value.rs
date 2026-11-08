// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed property values (RFC 5545 section 3.3): text, dates and times, durations, UTC offsets.

use std::fmt;

use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use crate::{IcalError, Property};

/// The seconds of a day.
pub(crate) const DAY_SECS: i64 = 86_400;

/// `text` as an iCalendar TEXT value: `\`, `;` and `,` escaped, a line break (CRLF, CR or LF) as `\n`, every other
/// control character but the tab left out.
///
/// ```
/// assert_eq!(inboxcraft_ical::escape_text("a;b,c\\d\r\ne"), "a\\;b\\,c\\\\d\\ne");
/// ```
#[must_use]
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\r' => {
                chars.next_if_eq(&'\n');
                out.push_str("\\n");
            }
            '\n' => out.push_str("\\n"),
            '\t' => out.push('\t'),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// A TEXT value read: `\n` and `\N` a line break, `\\`, `\;`, `\,` the character; any other escaped character
/// stands for itself.
///
/// ```
/// assert_eq!(inboxcraft_ical::unescape_text("a\\;b\\,c\\\\d\\Ne"), "a;b,c\\d\ne");
/// ```
#[must_use]
pub fn unescape_text(text: &str) -> String {
    split_text(text, false).concat()
}

/// A list of TEXT values (`CATEGORIES`, `RESOURCES`): split at every comma that is not escaped, each one read.
///
/// ```
/// assert_eq!(inboxcraft_ical::unescape_text_list("a\\,b,c"), ["a,b", "c"]);
/// ```
#[must_use]
pub fn unescape_text_list(text: &str) -> Vec<String> {
    split_text(text, true)
}

fn split_text(text: &str, list: bool) -> Vec<String> {
    let mut values = Vec::new();
    let mut value = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            ',' if list => values.push(std::mem::take(&mut value)),
            '\\' => match chars.next() {
                Some('n' | 'N') => value.push('\n'),
                Some(other) => value.push(other),
                None => value.push('\\'),
            },
            c => value.push(c),
        }
    }
    values.push(value);
    values
}

/// A calendar date, from year 0 to 10000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    midnight: CivilDateTime,
}

impl Date {
    /// The date, or `None` when it does not exist.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        CivilDateTime::new(year, month, day, 0, 0, 0).map(|midnight| Self { midnight })
    }

    /// The year.
    #[must_use]
    pub fn year(self) -> i32 {
        self.midnight.year()
    }

    /// The month, 1 to 12.
    #[must_use]
    pub fn month(self) -> u8 {
        self.midnight.month()
    }

    /// The day of the month, from 1.
    #[must_use]
    pub fn day(self) -> u8 {
        self.midnight.day()
    }

    /// The start of the day on the wall clock.
    #[must_use]
    pub fn midnight(self) -> CivilDateTime {
        self.midnight
    }

    /// The date `days` later (earlier when negative), or `None` outside years 0 to 10000.
    #[must_use]
    pub fn add_days(self, days: i64) -> Option<Self> {
        let moved = civil_from_seconds(civil_seconds(self.midnight).checked_add(days.checked_mul(DAY_SECS)?)?)?;
        Some(Self { midnight: moved })
    }

    /// The date of `civil`.
    #[must_use]
    pub fn of(civil: CivilDateTime) -> Self {
        Self { midnight: day_start(civil) }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}{:02}{:02}", self.year(), self.month(), self.day())
    }
}

/// When something starts, ends or happens: a DATE or one of the three forms of DATE-TIME (RFC 5545 section 3.3.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum When {
    /// A whole day (`VALUE=DATE`): all-day events.
    Date(Date),
    /// A wall-clock time in whatever zone the person is in.
    Floating(CivilDateTime),
    /// A moment, in UTC (the `Z` form).
    Utc(Timestamp),
    /// A wall-clock time in the zone `tzid` names (a `TZID` parameter).
    Zoned(CivilDateTime, String),
}

impl When {
    /// The value of `property`: a DATE when its `VALUE` is `DATE` or the value has no time, otherwise a DATE-TIME,
    /// [`When::Zoned`] when it carries a `TZID` and is not in UTC. A leap second is read as second 59.
    ///
    /// # Errors
    ///
    /// [`IcalError::BadValue`] when the value is neither.
    pub fn from_property(property: &Property) -> Result<Self, IcalError> {
        let tzid = property.param("TZID");
        Self::read(property.value(), property.param("VALUE"), tzid).ok_or_else(|| IcalError::bad_value(property.name()))
    }

    /// Every value of a list property (`EXDATE`, `RDATE`); a PERIOD counts by its start.
    ///
    /// # Errors
    ///
    /// [`IcalError::BadValue`] when one of them cannot be read.
    pub fn list_from_property(property: &Property) -> Result<Vec<Self>, IcalError> {
        let kind = property.param("VALUE");
        let tzid = property.param("TZID");
        let kind = if kind.is_some_and(|kind| kind.eq_ignore_ascii_case("PERIOD")) { None } else { kind };
        property
            .value()
            .split(',')
            .map(|value| {
                let start = value.split('/').next().unwrap_or(value);
                Self::read(start, kind, tzid).ok_or_else(|| IcalError::bad_value(property.name()))
            })
            .collect()
    }

    fn read(value: &str, kind: Option<&str>, tzid: Option<&str>) -> Option<Self> {
        let value = value.trim();
        if kind.is_some_and(|kind| kind.eq_ignore_ascii_case("DATE")) || value.len() == 8 {
            return read_date(value).map(Self::Date);
        }
        let (civil, utc) = read_date_time(value)?;
        Some(match (utc, tzid) {
            (true, _) => Self::Utc(civil.to_timestamp(Offset::UTC)?),
            (false, Some(tzid)) => Self::Zoned(civil, tzid.to_owned()),
            (false, None) => Self::Floating(civil),
        })
    }

    /// The property `name` holding this value, with `VALUE=DATE` or `TZID` as needed.
    #[must_use]
    pub fn to_property(&self, name: &str) -> Property {
        match self {
            Self::Date(date) => Property::new(name, date.to_string()).with_param("VALUE", "DATE"),
            Self::Floating(civil) => Property::new(name, write_civil(*civil)),
            Self::Utc(at) => Property::new(name, write_utc(*at)),
            Self::Zoned(civil, tzid) => Property::new(name, write_civil(*civil)).with_param("TZID", tzid.clone()),
        }
    }

    /// Whether this is a whole day.
    #[must_use]
    pub fn is_date(&self) -> bool {
        matches!(self, Self::Date(_))
    }
}

/// The property `name` with a list of values (`EXDATE`, `RDATE`) of one kind, or `None` when they are not all of
/// the kind and zone of the first.
#[must_use]
pub fn list_to_property(name: &str, values: &[When]) -> Option<Property> {
    let first = values.first()?.to_property(name);
    let mut texts = Vec::with_capacity(values.len());
    for value in values {
        let property = value.to_property(name);
        if property.params() != first.params() {
            return None;
        }
        texts.push(property.value().to_owned());
    }
    let mut property = Property::new(name, texts.join(","));
    for param in first.params() {
        property = property.with_param_values(param.name(), param.values().to_vec());
    }
    Some(property)
}

fn digits<T: std::str::FromStr>(text: &str, from: usize, to: usize) -> Option<T> {
    let part = text.get(from..to)?;
    if part.bytes().all(|byte| byte.is_ascii_digit()) { part.parse().ok() } else { None }
}

fn read_date(value: &str) -> Option<Date> {
    if value.len() != 8 {
        return None;
    }
    Date::new(digits(value, 0, 4)?, digits(value, 4, 6)?, digits(value, 6, 8)?)
}

/// `YYYYMMDDTHHMMSS` and whether a `Z` follows.
fn read_date_time(value: &str) -> Option<(CivilDateTime, bool)> {
    let (value, utc) = match value.strip_suffix(['Z', 'z']) {
        Some(value) => (value, true),
        None => (value, false),
    };
    if value.len() != 15 || !value.get(8..9)?.eq_ignore_ascii_case("T") {
        return None;
    }
    let date = read_date(value.get(..8)?)?;
    let second: u8 = digits(value, 13, 15)?;
    let civil = CivilDateTime::new(date.year(), date.month(), date.day(), digits(value, 9, 11)?, digits(value, 11, 13)?, second.min(59))?;
    Some((civil, utc))
}

fn write_civil(civil: CivilDateTime) -> String {
    format!("{}T{:02}{:02}{:02}", Date::of(civil), civil.hour(), civil.minute(), civil.second())
}

fn write_utc(at: Timestamp) -> String {
    // Every timestamp is within years 0 to 10000, so it has a UTC wall-clock time.
    at.to_civil(Offset::UTC).map_or_else(String::new, |civil| format!("{}Z", write_civil(civil)))
}

/// A DURATION (RFC 5545 section 3.3.6): whole days, which follow the wall clock across a change to or from summer
/// time, and seconds, which do not. Both have the same sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    days: i64,
    seconds: i64,
}

/// The longest [`Span`] read: 10001 years of days or of seconds, either way.
const MAX_SPAN_DAYS: i64 = 3_660_000;

impl Span {
    /// `days` and `seconds`, or `None` when their signs differ or one is beyond 10001 years.
    #[must_use]
    pub fn new(days: i64, seconds: i64) -> Option<Self> {
        let in_range = days.abs() <= MAX_SPAN_DAYS && seconds.abs() <= MAX_SPAN_DAYS.saturating_mul(DAY_SECS);
        (in_range && (days.signum() * seconds.signum()) >= 0).then_some(Self { days, seconds })
    }

    /// The days, negative before.
    #[must_use]
    pub fn days(self) -> i64 {
        self.days
    }

    /// The seconds, negative before.
    #[must_use]
    pub fn seconds(self) -> i64 {
        self.seconds
    }

    /// Whether it points into the past.
    #[must_use]
    pub fn is_negative(self) -> bool {
        self.days < 0 || self.seconds < 0
    }

    /// Reads `[+-]P...` with weeks, days, hours, minutes and seconds.
    ///
    /// # Errors
    ///
    /// [`IcalError::BadValue`] (named `DURATION`) when it is no duration.
    pub fn parse(text: &str) -> Result<Self, IcalError> {
        read_span(text.trim()).ok_or_else(|| IcalError::bad_value("DURATION"))
    }
}

fn read_span(text: &str) -> Option<Span> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let rest = rest.strip_prefix(['P', 'p'])?;
    let (mut days, mut seconds, mut in_time, mut number, mut any) = (0i64, 0i64, false, None::<i64>, false);
    let mut timed = false;
    for c in rest.chars() {
        if let Some(digit) = c.to_digit(10) {
            number = Some(number.unwrap_or(0).checked_mul(10)?.checked_add(i64::from(digit))?);
            continue;
        }
        let unit = match (c.to_ascii_uppercase(), in_time) {
            ('T', false) if number.is_none() => {
                in_time = true;
                continue;
            }
            ('W', false) => (7, 0),
            ('D', false) => (1, 0),
            ('H', true) => (0, 3600),
            ('M', true) => (0, 60),
            ('S', true) => (0, 1),
            _ => return None,
        };
        let value = number.take()?;
        days = days.checked_add(value.checked_mul(unit.0)?)?;
        seconds = seconds.checked_add(value.checked_mul(unit.1)?)?;
        any = true;
        timed = in_time;
    }
    if number.is_some() || !any || in_time != timed {
        return None;
    }
    if negative { Span::new(days.checked_neg()?, seconds.checked_neg()?) } else { Span::new(days, seconds) }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_negative() {
            f.write_str("-")?;
        }
        f.write_str("P")?;
        let (days, seconds) = (self.days.unsigned_abs(), self.seconds.unsigned_abs());
        if days > 0 || seconds == 0 {
            write!(f, "{days}D")?;
        }
        if seconds > 0 {
            f.write_str("T")?;
            let (hours, minutes, secs) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
            if hours > 0 {
                write!(f, "{hours}H")?;
            }
            if minutes > 0 {
                write!(f, "{minutes}M")?;
            }
            if secs > 0 {
                write!(f, "{secs}S")?;
            }
        }
        Ok(())
    }
}

/// A UTC-OFFSET value, `+HHMM` or `-HHMM` with optional seconds (dropped), as an [`Offset`].
///
/// # Errors
///
/// [`IcalError::BadValue`] (named `property`) when it is no offset.
pub fn read_offset(property: &Property) -> Result<Offset, IcalError> {
    let value = property.value().trim();
    let bad = || IcalError::bad_value(property.name());
    let sign: i16 = match value.get(..1) {
        Some("+") => 1,
        Some("-") => -1,
        _ => return Err(bad()),
    };
    if value.len() != 5 && value.len() != 7 {
        return Err(bad());
    }
    let hours: i16 = digits(value, 1, 3).ok_or_else(bad)?;
    let minutes: i16 = digits(value, 3, 5).ok_or_else(bad)?;
    if value.len() == 7 {
        digits::<u8>(value, 5, 7).ok_or_else(bad)?;
    }
    if minutes > 59 {
        return Err(bad());
    }
    Offset::from_minutes(sign * (hours * 60 + minutes)).ok_or_else(bad)
}

/// `offset` as a UTC-OFFSET value.
#[must_use]
pub fn write_offset(offset: Offset) -> String {
    let minutes = offset.minutes();
    let sign = if minutes < 0 { '-' } else { '+' };
    let minutes = minutes.unsigned_abs();
    format!("{sign}{:02}{:02}", minutes / 60, minutes % 60)
}

/// Seconds since 1970-01-01T00:00:00 on the wall clock: a count in which every day has 86400 seconds.
pub(crate) fn civil_seconds(civil: CivilDateTime) -> i64 {
    // Every civil time from year 0 to 10000 has a timestamp at UTC.
    civil.to_timestamp(Offset::UTC).map_or(0, Timestamp::unix_seconds)
}

/// The wall-clock time `seconds` after 1970-01-01T00:00:00, or `None` outside years 0 to 10000.
pub(crate) fn civil_from_seconds(seconds: i64) -> Option<CivilDateTime> {
    Timestamp::from_unix(seconds, 0)?.to_civil(Offset::UTC)
}

/// The start of the day of `civil`.
pub(crate) fn day_start(civil: CivilDateTime) -> CivilDateTime {
    CivilDateTime::new(civil.year(), civil.month(), civil.day(), 0, 0, 0).unwrap_or(civil)
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
