// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UID FETCH: what is asked for, and FETCH responses (RFC 9051 sections 6.4.5 and 7.5.2).
//!
//! The reading of one response follows UCS-Care-Group/mailcraft `crates/proto/src/imap/mod.rs` (`Item`, see
//! `PROVENANCE.md`).

use inboxcraft_model::Uid;
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use super::mailbox::flag_list;
use super::value::{Value, number};

/// The part of a message a fetch takes, without setting `\Seen` (`BODY.PEEK`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// The whole message (`BODY[]`).
    Full,
    /// The header, up to and with the empty line that ends it (`BODY[HEADER]`).
    Header,
    /// The body without the header (`BODY[TEXT]`).
    Text,
}

impl Section {
    fn peek(self) -> &'static str {
        match self {
            Self::Full => "BODY.PEEK[]",
            Self::Header => "BODY.PEEK[HEADER]",
            Self::Text => "BODY.PEEK[TEXT]",
        }
    }

    /// The section a FETCH response names (`BODY[HEADER]`, also IMAP2's `RFC822.HEADER`).
    fn named(name: &str) -> Option<Self> {
        match name {
            "BODY[]" | "RFC822" => Some(Self::Full),
            "BODY[HEADER]" | "RFC822.HEADER" => Some(Self::Header),
            "BODY[TEXT]" | "RFC822.TEXT" => Some(Self::Text),
            _ => None,
        }
    }
}

/// What a UID FETCH asks for besides the UID, which it always asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FetchAttributes {
    /// `FLAGS`.
    pub flags: bool,
    /// `INTERNALDATE`: when the server got the message.
    pub internal_date: bool,
    /// `RFC822.SIZE`.
    pub size: bool,
    /// A part of the message.
    pub section: Option<Section>,
}

impl FetchAttributes {
    /// The list of data items: `(UID FLAGS BODY.PEEK[])`.
    pub(crate) fn items(self) -> String {
        let mut items = vec!["UID"];
        for (wanted, item) in [(self.flags, "FLAGS"), (self.internal_date, "INTERNALDATE"), (self.size, "RFC822.SIZE")] {
            if wanted {
                items.push(item);
            }
        }
        if let Some(section) = self.section {
            items.push(section.peek());
        }
        format!("({})", items.join(" "))
    }
}

/// What one FETCH response says about a message; only what it says is `Some`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FetchItem {
    /// The message's sequence number.
    pub seq: u32,
    /// Its UID.
    pub uid: Option<Uid>,
    /// Its flags, at most 64.
    pub flags: Option<Vec<String>>,
    /// When the server got it; a date that cannot be read is left out.
    pub internal_date: Option<Timestamp>,
    /// Its size in bytes.
    pub size: Option<u32>,
    /// The part of the message the response carries, and its bytes.
    pub section: Option<(Section, Vec<u8>)>,
}

impl FetchItem {
    /// From the values of `n FETCH (name value name value ...)`, the names in any order; `None` when they are not that.
    pub(crate) fn from_values(values: Vec<Value>) -> Option<Self> {
        let mut values = values.into_iter();
        let seq = values.next()?.number().filter(|seq| *seq != 0)?;
        if !values.next()?.atom()?.eq_ignore_ascii_case("FETCH") {
            return None;
        }
        let Value::List(pairs) = values.next()? else { return None };
        let mut item = Self { seq, ..Self::default() };
        let mut pairs = pairs.into_iter();
        while let Some(name) = pairs.next() {
            // Something that is no name is skipped, in the hope that a name follows.
            let Some(name) = name.atom().map(str::to_ascii_uppercase) else { continue };
            let Some(value) = pairs.next() else { break };
            match (name.as_str(), value) {
                ("UID", value) => item.uid = value.number().and_then(Uid::new),
                ("FLAGS", Value::List(flags)) => item.flags = Some(flag_list(flags)),
                ("INTERNALDATE", Value::Str(date)) => item.internal_date = internal_date(&date),
                ("RFC822.SIZE", value) => item.size = value.number(),
                (name, Value::Str(bytes)) => {
                    if let Some(section) = Section::named(name) {
                        item.section = Some((section, bytes));
                    }
                }
                _ => {}
            }
        }
        Some(item)
    }
}

/// An IMAP `date-time` (RFC 9051): `17-Jul-1996 02:44:25 -0700`, the day padded with a space or not at all.
pub(crate) fn internal_date(text: &[u8]) -> Option<Timestamp> {
    const MONTHS: [&[u8]; 12] = [b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec"];
    let text = std::str::from_utf8(text).ok()?.trim_start();
    let (date, rest) = text.split_once(' ')?;
    let (time, zone) = rest.split_once(' ')?;
    let mut date = date.split('-');
    let day = date.next().filter(|d| (1..=2).contains(&d.len())).and_then(number)?;
    let month = date.next()?;
    let month = MONTHS.iter().position(|m| m.eq_ignore_ascii_case(month.as_bytes()))?;
    let year = date.next().filter(|y| y.len() == 4).and_then(number)?;
    let mut time = time.split(':');
    let mut two = || time.next().filter(|t| t.len() == 2).and_then(number);
    let (hour, minute, second) = (two()?, two()?, two()?);
    let (sign, digits) = match zone.as_bytes().first()? {
        b'+' => (1, zone.get(1..)?),
        b'-' => (-1, zone.get(1..)?),
        _ => return None,
    };
    let zone = Some(digits).filter(|d| d.len() == 4).and_then(number)?;
    let (zone_hours, zone_minutes) = (zone / 100, zone % 100);
    if date.next().is_some() || zone_minutes >= 60 {
        return None;
    }
    let offset = Offset::from_minutes(i16::try_from(zone_hours * 60 + zone_minutes).ok()?.saturating_mul(sign))?;
    let civil = CivilDateTime::new(
        i32::try_from(year).ok()?,
        u8::try_from(month.saturating_add(1)).ok()?,
        u8::try_from(day).ok()?,
        u8::try_from(hour).ok()?,
        u8::try_from(minute).ok()?,
        u8::try_from(second).ok()?,
    )?;
    civil.to_timestamp(offset)
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
