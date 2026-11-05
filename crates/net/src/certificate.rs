// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What `account.check` shows of a server's certificate: the issuer and the validity, read from its DER (RFC 5280
//! section 4.1).
//!
//! The certificate comes from the server and is hostile. The reader takes only the fields it needs, refuses
//! anything that is not DER as RFC 5280 writes it, and never trusts a length: every read is checked against what is
//! there. Nothing here decides whether a certificate is trusted; `rustls` and the platform's verifier do that.

use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

/// The most characters taken of one name attribute.
const MAX_VALUE_CHARS: usize = 128;

/// The most name attributes read of the issuer.
const MAX_ATTRIBUTES: usize = 16;

const SEQUENCE: u8 = 0x30;
const SET: u8 = 0x31;
const OID: u8 = 0x06;
const UTC_TIME: u8 = 0x17;
const GENERALIZED_TIME: u8 = 0x18;
const VERSION: u8 = 0xa0;

/// The issuer and the validity of a certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    issuer: String,
    not_before: Timestamp,
    not_after: Timestamp,
}

impl Certificate {
    /// The issuer as `CN=…, O=…`: the common name, organisation, organisational unit and country, in the
    /// certificate's order. Empty when the issuer has none of them.
    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The start of the validity.
    #[must_use]
    pub fn not_before(&self) -> Timestamp {
        self.not_before
    }

    /// The end of the validity.
    #[must_use]
    pub fn not_after(&self) -> Timestamp {
        self.not_after
    }
}

/// The issuer and validity of the DER certificate `der`, or `None` when it is not one.
///
/// ```
/// assert_eq!(inboxcraft_net::certificate::read(b"\x30\x00"), None);
/// ```
#[must_use]
pub fn read(der: &[u8]) -> Option<Certificate> {
    let (certificate, _) = Der(der).take(SEQUENCE)?;
    let (tbs, _) = Der(certificate).take(SEQUENCE)?;
    let mut fields = Der(tbs);
    if fields.peek() == Some(VERSION) {
        fields = fields.skip()?;
    }
    // The serial number and the signature algorithm.
    let fields = fields.skip()?.skip()?;
    let (issuer, fields) = fields.take(SEQUENCE)?;
    let (validity, _) = fields.take(SEQUENCE)?;
    let (not_before, validity) = time(Der(validity))?;
    let (not_after, _) = time(validity)?;
    Some(Certificate { issuer: name(issuer)?, not_before, not_after })
}

/// The rest of a DER encoding.
#[derive(Debug, Clone, Copy)]
struct Der<'a>(&'a [u8]);

impl<'a> Der<'a> {
    fn peek(self) -> Option<u8> {
        self.0.first().copied()
    }

    /// The next element: its tag, its contents and what follows it.
    fn next(self) -> Option<(u8, &'a [u8], Self)> {
        let tag = *self.0.first()?;
        // A high tag number (the low five bits all set) is not in a certificate's fields we read.
        if tag & 0x1f == 0x1f {
            return None;
        }
        let first = *self.0.get(1)?;
        let (length, start) = if first < 0x80 {
            (usize::from(first), 2_usize)
        } else {
            let count = usize::from(first & 0x7f);
            // Indefinite lengths are BER, and four bytes are more than any certificate needs.
            if count == 0 || count > 4 {
                return None;
            }
            let bytes = self.0.get(2..2_usize.checked_add(count)?)?;
            let length = bytes.iter().try_fold(0_usize, |length, byte| length.checked_mul(256)?.checked_add(usize::from(*byte)))?;
            (length, 2_usize.checked_add(count)?)
        };
        let end = start.checked_add(length)?;
        Some((tag, self.0.get(start..end)?, Self(self.0.get(end..)?)))
    }

    /// The contents of the next element, which must have `tag`, and what follows it.
    fn take(self, tag: u8) -> Option<(&'a [u8], Self)> {
        let (found, contents, rest) = self.next()?;
        (found == tag).then_some((contents, rest))
    }

    /// What follows the next element.
    fn skip(self) -> Option<Self> {
        self.next().map(|(_, _, rest)| rest)
    }
}

/// A `Name` (a sequence of sets of type and value) as `CN=…, O=…`.
fn name(der: &[u8]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut rest = Der(der);
    while rest.peek().is_some() {
        let (set, after) = rest.take(SET)?;
        rest = after;
        let mut attributes = Der(set);
        while attributes.peek().is_some() {
            let (attribute, after) = attributes.take(SEQUENCE)?;
            attributes = after;
            let (oid, value) = Der(attribute).take(OID)?;
            let (tag, value, _) = value.next()?;
            let label = match oid {
                [0x55, 0x04, 0x03] => "CN",
                [0x55, 0x04, 0x0a] => "O",
                [0x55, 0x04, 0x0b] => "OU",
                [0x55, 0x04, 0x06] => "C",
                _ => continue,
            };
            if parts.len() < MAX_ATTRIBUTES {
                parts.push(format!("{label}={}", text(tag, value)?));
            }
        }
    }
    Some(parts.join(", "))
}

/// A directory string as text, cut at [`MAX_VALUE_CHARS`]; control characters become spaces.
fn text(tag: u8, value: &[u8]) -> Option<String> {
    let decoded: String = match tag {
        // UTF8String, PrintableString, TeletexString (taken as Latin-1 where it is not UTF-8), IA5String.
        0x0c | 0x13 | 0x16 => String::from_utf8_lossy(value).into_owned(),
        0x14 => match std::str::from_utf8(value) {
            Ok(text) => text.to_owned(),
            Err(_) => value.iter().map(|byte| char::from(*byte)).collect(),
        },
        // BMPString: UTF-16, big-endian.
        0x1e => {
            let units = value.chunks(2).map(|pair| match pair {
                [high, low] => u16::from_be_bytes([*high, *low]),
                _ => 0xfffd,
            });
            char::decode_utf16(units).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
        }
        // UniversalString: UTF-32, big-endian.
        0x1c => value
            .chunks(4)
            .map(|quad| match quad {
                [a, b, c, d] => char::from_u32(u32::from_be_bytes([*a, *b, *c, *d])).unwrap_or(char::REPLACEMENT_CHARACTER),
                _ => char::REPLACEMENT_CHARACTER,
            })
            .collect(),
        _ => return None,
    };
    Some(decoded.chars().take(MAX_VALUE_CHARS).map(|c| if c.is_control() { ' ' } else { c }).collect())
}

/// A `Time`: UTCTime (`YYMMDDHHMMSSZ`, years 1950 to 2049) or GeneralizedTime (`YYYYMMDDHHMMSSZ`), both in UTC.
fn time(der: Der<'_>) -> Option<(Timestamp, Der<'_>)> {
    let (tag, contents, rest) = der.next()?;
    let digits = match (tag, contents) {
        (UTC_TIME, [digits @ .., b'Z']) if digits.len() == 12 => digits,
        (GENERALIZED_TIME, [digits @ .., b'Z']) if digits.len() == 14 => digits,
        _ => return None,
    };
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let number = |from: usize| -> Option<u8> {
        let pair = digits.get(from..from.checked_add(2)?)?;
        pair.iter().try_fold(0_u8, |value, digit| value.checked_mul(10)?.checked_add(digit.checked_sub(b'0')?))
    };
    let (year, at) = if tag == UTC_TIME {
        let short = i32::from(number(0)?);
        (if short >= 50 { 1900 + short } else { 2000 + short }, 2)
    } else {
        (i32::from(number(0)?) * 100 + i32::from(number(2)?), 4)
    };
    let civil = CivilDateTime::new(year, number(at)?, number(at + 2)?, number(at + 4)?, number(at + 6)?, number(at + 8)?)?;
    Some((civil.to_timestamp(Offset::UTC)?, rest))
}

#[cfg(test)]
#[path = "certificate_tests.rs"]
mod tests;
