// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! RFC 5322 dates (section 3.3), with the obsolete forms of section 4.3.

use std::fmt;
use std::ops::RangeInclusive;
use std::str::FromStr;

use crate::{CivilDateTime, Offset, Timestamp, Weekday};

/// The longest date text read, in bytes: the RFC 5322 line limit. Anything longer is refused unread.
pub const MAX_LEN: usize = 998;

/// How deep comments may nest inside a date.
pub const MAX_COMMENT_DEPTH: usize = 16;

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// The zone of a mail date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    /// A known offset from UTC (`+0100`, `-0600`, `+0000`, or an obsolete name such as `GMT` or `PST`).
    Fixed(Offset),
    /// `-0000`: the time is UTC, but the zone the sender's clock showed is unknown. A military letter, an unknown
    /// zone name and a missing zone read as this too (RFC 5322 section 4.3).
    Unknown,
}

impl Zone {
    /// The offset the local time is at: UTC for [`Zone::Unknown`].
    #[must_use]
    pub const fn offset(self) -> Offset {
        match self {
            Self::Fixed(offset) => offset,
            Self::Unknown => Offset::UTC,
        }
    }
}

/// A date as a mail header carries it: a moment to the second and the zone of the sender's clock.
///
/// Formatting writes RFC 5322 (`Wed, 4 Nov 2026 13:05:09 +0100`); parsing reads it and the obsolete forms besides:
/// comments and white space anywhere, full day and month names, any letter case, two- and three-digit years, no
/// seconds, second 60 (read as 59), zone names and no zone at all. The day of the week is not checked against the
/// date. Text from a message is hostile: parsing never panics, reads at most [`MAX_LEN`] bytes and nests comments at
/// most [`MAX_COMMENT_DEPTH`] deep.
///
/// ```
/// use inboxcraft_time::MailDate;
///
/// let date = MailDate::parse("4 nov 26 13:05 GMT (obsolete)")?;
/// assert_eq!(date.to_string(), "Wed, 4 Nov 2026 13:05:00 +0000");
/// # Ok::<(), inboxcraft_time::DateError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MailDate {
    timestamp: Timestamp,
    zone: Zone,
    local: CivilDateTime,
}

impl MailDate {
    /// The moment `timestamp`, without its nanoseconds, in `zone`; `None` when the local time there falls outside
    /// years 1 to 9999.
    #[must_use]
    pub fn new(timestamp: Timestamp, zone: Zone) -> Option<Self> {
        let timestamp = Timestamp::from_unix(timestamp.unix_seconds(), 0)?;
        let local = timestamp.to_civil(zone.offset())?;
        (1..=9999).contains(&local.year()).then_some(Self { timestamp, zone, local })
    }

    /// The local time `local` in `zone`; `None` outside years 1 to 9999.
    #[must_use]
    pub fn from_local(local: CivilDateTime, zone: Zone) -> Option<Self> {
        if !(1..=9999).contains(&local.year()) {
            return None;
        }
        Some(Self { timestamp: local.to_timestamp(zone.offset())?, zone, local })
    }

    /// Reads an RFC 5322 date, current or obsolete form.
    ///
    /// # Errors
    ///
    /// [`DateError`] says what was wrong and, for a syntax error, where.
    pub fn parse(text: &str) -> Result<Self, DateError> {
        if text.len() > MAX_LEN {
            return Err(DateError::TooLong);
        }
        Parser { text, tokens: tokenize(text.as_bytes())?, next: 0 }.date()
    }

    /// The moment, to the second.
    #[must_use]
    pub const fn timestamp(self) -> Timestamp {
        self.timestamp
    }

    /// The zone of the sender's clock.
    #[must_use]
    pub const fn zone(self) -> Zone {
        self.zone
    }

    /// What the sender's clock showed (UTC for [`Zone::Unknown`]).
    #[must_use]
    pub const fn local(self) -> CivilDateTime {
        self.local
    }
}

impl fmt::Display for MailDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let l = self.local;
        let weekday = abbreviation(&WEEKDAYS, weekday_index(l.weekday()));
        let month = abbreviation(&MONTHS, usize::from(l.month()).saturating_sub(1));
        write!(f, "{weekday}, {} {month} {:04} {:02}:{:02}:{:02} ", l.day(), l.year(), l.hour(), l.minute(), l.second())?;
        let minutes = self.zone.offset().minutes();
        let sign = if minutes < 0 || self.zone == Zone::Unknown { '-' } else { '+' };
        let minutes = minutes.unsigned_abs();
        write!(f, "{sign}{:02}{:02}", minutes / 60, minutes % 60)
    }
}

impl FromStr for MailDate {
    type Err = DateError;

    fn from_str(text: &str) -> Result<Self, DateError> {
        Self::parse(text)
    }
}

/// Why a date could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DateError {
    /// The text is longer than [`MAX_LEN`] bytes.
    TooLong,
    /// A comment is not closed.
    UnclosedComment,
    /// Comments nest deeper than [`MAX_COMMENT_DEPTH`].
    CommentTooDeep,
    /// Something else stands where a part of the date should.
    Syntax {
        /// The part that was expected.
        expected: &'static str,
        /// The byte offset of what stands there instead.
        at: usize,
    },
    /// The date or time does not exist: 30 February, hour 24, minute 60 or the like.
    NoSuchDate,
    /// The year is 0 or after 9999.
    YearOutOfRange,
}

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => write!(f, "the date is longer than {MAX_LEN} bytes"),
            Self::UnclosedComment => f.write_str("a comment in the date is not closed"),
            Self::CommentTooDeep => write!(f, "comments in the date nest deeper than {MAX_COMMENT_DEPTH}"),
            Self::Syntax { expected, at } => write!(f, "expected {expected} at byte {at}"),
            Self::NoSuchDate => f.write_str("the date or time does not exist"),
            Self::YearOutOfRange => f.write_str("the year is not between 1 and 9999"),
        }
    }
}

impl std::error::Error for DateError {}

fn abbreviation(names: &[&'static str], index: usize) -> &'static str {
    names.get(index).and_then(|name| name.get(..3)).unwrap_or("")
}

fn weekday_index(weekday: Weekday) -> usize {
    match weekday {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    }
}

/// The index of the name `word` is: its first three letters or all of it, in any case.
fn name_index(names: &[&str], word: &str) -> Option<usize> {
    names
        .iter()
        .position(|name| (word.len() == 3 || word.len() == name.len()) && name.get(..word.len()).is_some_and(|n| n.eq_ignore_ascii_case(word)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Digits,
    Letters,
    /// Letters and digits run together.
    Mixed,
    Punct(u8),
    /// A byte that has no place in a date outside a comment.
    Other,
}

#[derive(Debug, Clone, Copy)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
}

/// Splits the text into runs of letters and digits and single punctuation bytes, dropping white space and comments.
fn tokenize(bytes: &[u8]) -> Result<Vec<Token>, DateError> {
    let mut tokens = Vec::new();
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let start = at;
        at += 1;
        let kind = match byte {
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            b'(' => {
                at = skip_comment(bytes, at)?;
                continue;
            }
            b',' | b':' | b'+' | b'-' => Kind::Punct(byte),
            _ if byte.is_ascii_alphanumeric() => {
                while bytes.get(at).is_some_and(u8::is_ascii_alphanumeric) {
                    at += 1;
                }
                let run = bytes.get(start..at).unwrap_or_default();
                if run.iter().all(u8::is_ascii_digit) {
                    Kind::Digits
                } else if run.iter().all(u8::is_ascii_alphabetic) {
                    Kind::Letters
                } else {
                    Kind::Mixed
                }
            }
            _ => Kind::Other,
        };
        tokens.push(Token { kind, start, end: at });
    }
    Ok(tokens)
}

/// The offset just past the comment whose `(` ends before `at`.
fn skip_comment(bytes: &[u8], mut at: usize) -> Result<usize, DateError> {
    let mut depth = 1;
    while depth > 0 {
        match bytes.get(at) {
            None => return Err(DateError::UnclosedComment),
            Some(b'\\') => at += 1,
            Some(b'(') if depth == MAX_COMMENT_DEPTH => return Err(DateError::CommentTooDeep),
            Some(b'(') => depth += 1,
            Some(b')') => depth -= 1,
            Some(_) => {}
        }
        at += 1;
    }
    Ok(at)
}

struct Parser<'a> {
    text: &'a str,
    tokens: Vec<Token>,
    next: usize,
}

impl<'a> Parser<'a> {
    fn date(mut self) -> Result<MailDate, DateError> {
        if self.peek_kind() == Some(Kind::Letters) {
            let word = self.take(Kind::Letters, "the day of the week")?;
            name_index(&WEEKDAYS, word.text).ok_or_else(|| self.syntax_error_at("the day of the week", word.start))?;
            self.punct(b',', "a comma after the day of the week")?;
        }
        let day = self.number(1..=2, "the day of the month")?;
        let month = self.take(Kind::Letters, "the month")?;
        let month = name_index(&MONTHS, month.text).ok_or_else(|| self.syntax_error_at("the month", month.start))?;
        let year = self.year()?;
        let hour = self.number(1..=2, "the hour")?;
        self.punct(b':', "a colon after the hour")?;
        let minute = self.number(1..=2, "the minute")?;
        let second = if self.peek_kind() == Some(Kind::Punct(b':')) {
            self.next += 1;
            self.number(1..=2, "the second")?
        } else {
            0
        };
        let zone = self.zone()?;
        if let Some(token) = self.tokens.get(self.next) {
            return Err(self.syntax_error_at("the end of the date", token.start));
        }
        // Second 60 is a leap second, which `Timestamp` does not count.
        let second = if second == 60 { 59 } else { second };
        let month = u8::try_from(month + 1).map_err(|_| DateError::NoSuchDate)?;
        let local = [day, hour, minute, second].map(|n| u8::try_from(n).unwrap_or(u8::MAX));
        let local = CivilDateTime::new(year, month, local[0], local[1], local[2], local[3]).ok_or(DateError::NoSuchDate)?;
        MailDate::from_local(local, zone).ok_or(DateError::NoSuchDate)
    }

    fn year(&mut self) -> Result<i32, DateError> {
        let token = self.take(Kind::Digits, "the year")?;
        let value = digits_value(token.text);
        let year = match token.text.len() {
            // Obsolete two- and three-digit years (RFC 5322 section 4.3).
            2 if value < 50 => value + 2000,
            2 => value + 1900,
            3 => value + 1900,
            4.. => value,
            _ => return Err(self.syntax_error_at("the year", token.start)),
        };
        if !(1..=9999).contains(&year) {
            return Err(DateError::YearOutOfRange);
        }
        i32::try_from(year).map_err(|_| DateError::YearOutOfRange)
    }

    fn zone(&mut self) -> Result<Zone, DateError> {
        let Some(token) = self.tokens.get(self.next).copied() else {
            return Ok(Zone::Unknown);
        };
        self.next += 1;
        let text = self.text.get(token.start..token.end).unwrap_or_default();
        match token.kind {
            Kind::Punct(sign @ (b'+' | b'-')) => {
                let digits = self.tokens.get(self.next).copied();
                let digits = digits.filter(|d| d.kind == Kind::Digits && d.start == token.end && d.end - d.start == 4);
                let digits = digits.ok_or_else(|| self.syntax_error_at("a zone of four digits after the sign", token.start))?;
                self.next += 1;
                let value = digits_value(self.text.get(digits.start..digits.end).unwrap_or_default());
                if value % 100 > 59 {
                    return Err(DateError::NoSuchDate);
                }
                let minutes = i16::try_from(value / 100 * 60 + value % 100).map_err(|_| DateError::NoSuchDate)?;
                Ok(match (sign, minutes) {
                    (b'-', 0) => Zone::Unknown,
                    (b'-', _) => Zone::Fixed(Offset::from_minutes(-minutes).ok_or(DateError::NoSuchDate)?),
                    _ => Zone::Fixed(Offset::from_minutes(minutes).ok_or(DateError::NoSuchDate)?),
                })
            }
            Kind::Letters => Ok(named_zone(text)),
            _ => Err(self.syntax_error_at("the zone", token.start)),
        }
    }

    fn peek_kind(&self) -> Option<Kind> {
        self.tokens.get(self.next).map(|token| token.kind)
    }

    fn take(&mut self, kind: Kind, expected: &'static str) -> Result<Word<'a>, DateError> {
        let token = self.tokens.get(self.next).copied();
        match token {
            Some(token) if token.kind == kind => {
                self.next += 1;
                Ok(Word { text: self.text.get(token.start..token.end).unwrap_or_default(), start: token.start })
            }
            _ => Err(self.syntax_error(expected)),
        }
    }

    fn number(&mut self, digits: RangeInclusive<usize>, expected: &'static str) -> Result<u32, DateError> {
        let token = self.take(Kind::Digits, expected)?;
        if !digits.contains(&token.text.len()) {
            return Err(self.syntax_error_at(expected, token.start));
        }
        Ok(digits_value(token.text))
    }

    fn punct(&mut self, byte: u8, expected: &'static str) -> Result<(), DateError> {
        self.take(Kind::Punct(byte), expected).map(|_| ())
    }

    /// A syntax error at the next token, or at the end of the text.
    fn syntax_error(&self, expected: &'static str) -> DateError {
        let at = self.tokens.get(self.next).map_or(self.text.len(), |token| token.start);
        DateError::Syntax { expected, at }
    }

    fn syntax_error_at(&self, expected: &'static str, at: usize) -> DateError {
        DateError::Syntax { expected, at: at.min(self.text.len()) }
    }
}

struct Word<'a> {
    text: &'a str,
    start: usize,
}

/// The value of a run of ASCII digits; a run too long for `u32` gives `u32::MAX`.
fn digits_value(digits: &str) -> u32 {
    digits.bytes().try_fold(0_u32, |value, digit| value.checked_mul(10)?.checked_add(u32::from(digit.saturating_sub(b'0')))).unwrap_or(u32::MAX)
}

/// The zone a name stands for: the obsolete names of RFC 5322 section 4.3 and `UTC`; anything else (a military
/// letter, `CET`, a city) is unknown.
fn named_zone(name: &str) -> Zone {
    const NAMES: [(&str, i16); 11] = [
        ("UT", 0),
        ("UTC", 0),
        ("GMT", 0),
        ("EST", -300),
        ("EDT", -240),
        ("CST", -360),
        ("CDT", -300),
        ("MST", -420),
        ("MDT", -360),
        ("PST", -480),
        ("PDT", -420),
    ];
    NAMES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(name))
        .and_then(|&(_, minutes)| Offset::from_minutes(minutes))
        .map_or(Zone::Unknown, Zone::Fixed)
}

#[cfg(test)]
#[path = "rfc5322_tests.rs"]
mod tests;
