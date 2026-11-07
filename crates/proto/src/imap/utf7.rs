// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Modified UTF-7, the encoding of mailbox names (RFC 3501 section 5.1.3).
//!
//! Printable ASCII stands for itself, except `&`, which is written `&-`. Everything else is UTF-16 in base64 (with
//! `,` in place of `/`, and no padding) between `&` and `-`.
//!
//! Taken from UCS-Care-Group/mailcraft `crates/proto/src/imap/utf7.rs` (see `PROVENANCE.md`), tests moved to a file
//! of their own.

const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,";

fn digit(six_bits: u32) -> char {
    DIGITS.get((six_bits & 0x3F) as usize).map_or('A', |b| char::from(*b))
}

/// A mailbox name as people read it. Text that is not valid modified UTF-7 is kept as it is.
///
/// ```
/// assert_eq!(inboxcraft_proto::imap::decode_utf7("Entw&APw-rfe"), "Entwürfe");
/// ```
#[must_use]
pub fn decode_utf7(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut rest = name;
    // Every round takes at least the `&` off `rest`.
    while let Some(at) = rest.find('&') {
        out.push_str(rest.get(..at).unwrap_or(""));
        let after = rest.get(at.saturating_add(1)..).unwrap_or("");
        rest = match after.find('-') {
            Some(0) => {
                out.push('&');
                after.get(1..).unwrap_or("")
            }
            Some(end) => match shifted(after.get(..end).unwrap_or("")) {
                Some(text) => {
                    out.push_str(&text);
                    after.get(end.saturating_add(1)..).unwrap_or("")
                }
                // Not base64 after all: an ampersand like any other character.
                None => {
                    out.push('&');
                    after
                }
            },
            None => {
                out.push('&');
                after
            }
        };
    }
    out.push_str(rest);
    out
}

/// The text of one base64 run, or `None` when it is not one.
fn shifted(encoded: &str) -> Option<String> {
    let mut units = Vec::with_capacity(encoded.len() / 2 + 1);
    let (mut bits, mut count) = (0_u32, 0_u32);
    for b in encoded.bytes() {
        let value = u32::try_from(DIGITS.iter().position(|d| *d == b)?).ok()?;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 16 {
            count -= 16;
            units.push((bits >> count) as u16);
            bits &= (1 << count) - 1;
        }
    }
    // What is left over is padding: less than one digit's worth.
    if count >= 6 || units.is_empty() {
        return None;
    }
    String::from_utf16(&units).ok()
}

/// A mailbox name as a server wants it.
///
/// ```
/// assert_eq!(inboxcraft_proto::imap::encode_utf7("Tom & Jerry"), "Tom &- Jerry");
/// ```
#[must_use]
pub fn encode_utf7(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending: Vec<u16> = Vec::new();
    for c in name.chars() {
        if matches!(c, ' '..='~') {
            flush(&mut pending, &mut out);
            out.push(c);
            if c == '&' {
                out.push('-');
            }
        } else {
            let mut units = [0_u16; 2];
            pending.extend_from_slice(c.encode_utf16(&mut units));
        }
    }
    flush(&mut pending, &mut out);
    out
}

/// Write the characters waiting to be encoded as one base64 run.
fn flush(pending: &mut Vec<u16>, out: &mut String) {
    if pending.is_empty() {
        return;
    }
    out.push('&');
    let (mut bits, mut count) = (0_u32, 0_u32);
    for unit in pending.drain(..) {
        bits = (bits << 16) | u32::from(unit);
        count += 16;
        while count >= 6 {
            count -= 6;
            out.push(digit(bits >> count));
        }
        bits &= (1 << count) - 1;
    }
    if count > 0 {
        out.push(digit(bits << (6 - count)));
    }
    out.push('-');
}

#[cfg(test)]
#[path = "utf7_tests.rs"]
mod tests;
