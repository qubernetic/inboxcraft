// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! iCalendar (RFC 5545): reading and writing calendar objects, time zones and recurrence.
//!
//! Layer 2 (see `docs/architecture.md`); it uses `time` for dates, offsets and zones. No I/O, the same on native and
//! wasm.
//!
//! - [`parse`] reads text into a tree of [`Component`]s with their [`Property`]s and [`Param`]s, lines unfolded,
//!   names in capitals; [`Component::write`] writes it back with CRLF and lines folded at 75 octets, CR, LF and NUL
//!   left out of values and parameter values quoted or escaped (RFC 6868), so nothing written can start a line.
//! - Values: [`When`] (a date, a floating time, UTC, or a time in a zone a `TZID` names), [`Span`] (a duration),
//!   UTC offsets, and TEXT escapes ([`escape_text`], [`unescape_text`]).
//! - [`Rule`] reads and writes an `RRULE`; [`Rule::iter`] expands it on the wall clock, every part of RFC 5545
//!   section 3.3.10 included (`BYSETPOS`, `BYWEEKNO`, `WKST`, ordinal weekdays, days a month does not have
//!   skipped).
//! - [`TimeZone`] is a `VTIMEZONE` as `time`'s [`inboxcraft_time::Offsets`]; [`Zones`] finds them by `TZID` and
//!   turns a [`When`] into a moment.
//! - [`Calendar`] reads a `VCALENDAR` into [`Event`]s (people, alarms, rules) and zones, keeps what it does not read
//!   to write it back, and [`Calendar::occurrences`] lists the occurrences of every event in a span of time:
//!   `RDATE`, `EXDATE` and changed instances (`RECURRENCE-ID`) applied, summer time followed.
//!
//! A calendar is hostile input. Nothing panics on any bytes; what cannot be read is an [`IcalError`]: input over
//! [`MAX_BYTES`], over [`MAX_ITEMS`] properties and components, nesting deeper than [`MAX_DEPTH`], a rule part out
//! of range or a list over [`MAX_RULE_VALUES`]. An event or zone that cannot be read is kept as it came, not an
//! error. Expanding is bounded too: [`MAX_PERIODS`] empty periods in a row, [`MAX_PER_PERIOD`] candidates in a
//! period, [`MAX_STEPS`] occurrences walked and [`MAX_OCCURRENCES`] given, and [`Expansion::truncated`] says when a
//! limit left something out.
//!
//! ```
//! use inboxcraft_ical::{Calendar, IcalError};
//! use inboxcraft_time::{Offset, Offsets, Timestamp};
//!
//! let text = "BEGIN:VCALENDAR\r\n\
//!     VERSION:2.0\r\n\
//!     PRODID:-//Example//Example//EN\r\n\
//!     BEGIN:VTIMEZONE\r\n\
//!     TZID:Europe/Budapest\r\n\
//!     BEGIN:DAYLIGHT\r\n\
//!     DTSTART:19700329T020000\r\n\
//!     TZOFFSETFROM:+0100\r\n\
//!     TZOFFSETTO:+0200\r\n\
//!     RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\n\
//!     END:DAYLIGHT\r\n\
//!     BEGIN:STANDARD\r\n\
//!     DTSTART:19701025T030000\r\n\
//!     TZOFFSETFROM:+0200\r\n\
//!     TZOFFSETTO:+0100\r\n\
//!     RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\n\
//!     END:STANDARD\r\n\
//!     END:VTIMEZONE\r\n\
//!     BEGIN:VEVENT\r\n\
//!     UID:standup@example.org\r\n\
//!     DTSTART;TZID=Europe/Budapest:20260323T093000\r\n\
//!     DURATION:PT15M\r\n\
//!     SUMMARY:Stand-up\\, short\r\n\
//!     RRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=3\r\n\
//!     END:VEVENT\r\n\
//!     END:VCALENDAR\r\n";
//! let calendar = Calendar::parse(text.as_bytes())?;
//! assert_eq!(calendar.events[0].summary.as_deref(), Some("Stand-up, short"));
//!
//! // Three Mondays, the last one in summer time: 9:30 in Budapest each time.
//! let (from, to) = (Timestamp::from_unix(1_774_224_000, 0).unwrap(), Timestamp::from_unix(1_776_000_000, 0).unwrap());
//! let expansion = calendar.occurrences(from, to, &Offset::UTC);
//! let budapest = calendar.zones.get("Europe/Budapest").unwrap();
//! for occurrence in &expansion.occurrences {
//!     let wall = budapest.civil(occurrence.start).unwrap();
//!     assert_eq!((wall.hour(), wall.minute()), (9, 30));
//! }
//! assert_eq!(expansion.occurrences.len(), 3);
//! assert_eq!(Calendar::parse(calendar.write()?.as_bytes())?, calendar);
//! # Ok::<(), IcalError>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod calendar;
mod content;
mod error;
mod event;
mod rule;
mod value;
mod zone;

pub use calendar::{Calendar, Expansion, MAX_OCCURRENCES, MAX_STEPS, Occurrence};
pub use content::{Component, MAX_BYTES, MAX_DEPTH, MAX_ITEMS, Param, Property, parse};
pub use error::{IcalError, MAX_NAME_IN_ERROR};
pub use event::{Alarm, Attendee, Class, Event, PartStat, Status, Trigger};
pub use rule::{End, Frequency, MAX_PER_PERIOD, MAX_PERIODS, MAX_RULE_VALUES, Recurrence, Rule, WeekdayNum};
pub use value::{Date, Span, When, escape_text, list_to_property, read_offset, unescape_text, unescape_text_list, write_offset};
pub use zone::{MAX_ONSETS, TimeZone, Zones};

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_ical::name(), "inboxcraft-ical");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
