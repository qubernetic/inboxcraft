// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Content transfer encodings (RFC 2045 section 6): base64 and quoted-printable.
//!
//! Adapted from UCS-Care-Group/mailcraft `crates/mime/src/transfer.rs` (see `PROVENANCE.md`); base64 here is strict.

use std::borrow::Cow;

use crate::MimeError;

/// The most characters of an unknown encoding's name kept in [`TransferEncoding::Unknown`].
pub const MAX_NAME_IN_ERROR: usize = 64;

/// A `Content-Transfer-Encoding`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TransferEncoding {
    /// `7bit`, also what a part without the field is.
    #[default]
    SevenBit,
    /// `8bit`.
    EightBit,
    /// `binary`.
    Binary,
    /// `quoted-printable`.
    QuotedPrintable,
    /// `base64`.
    Base64,
    /// Any other name, lower case, at most [`MAX_NAME_IN_ERROR`] characters, control characters as U+FFFD.
    Unknown(String),
}

impl TransferEncoding {
    /// Reads a `Content-Transfer-Encoding` value; an empty one is `7bit`.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let name = value.split(|c: char| c.is_whitespace() || c == '(').find(|t| !t.is_empty()).unwrap_or_default().to_lowercase();
        match name.as_str() {
            "" | "7bit" => Self::SevenBit,
            "8bit" => Self::EightBit,
            "binary" => Self::Binary,
            "quoted-printable" => Self::QuotedPrintable,
            "base64" => Self::Base64,
            _ => Self::Unknown(name.chars().take(MAX_NAME_IN_ERROR).map(|c| if c.is_control() { '\u{fffd}' } else { c }).collect()),
        }
    }

    /// Undoes the encoding. The identity encodings borrow the body.
    ///
    /// # Errors
    ///
    /// [`MimeError::BadBase64`] for base64 with bytes outside its alphabet (line breaks and spaces aside) or a lone
    /// character at the end of a group; [`MimeError::UnknownEncoding`] for an encoding RFC 2045 does not define.
    /// Quoted-printable never fails: an `=` that starts no escape stays as it is (RFC 2045 section 6.7, note 1).
    pub fn decode<'b>(&self, body: &'b [u8]) -> Result<Cow<'b, [u8]>, MimeError> {
        match self {
            Self::SevenBit | Self::EightBit | Self::Binary => Ok(Cow::Borrowed(body)),
            Self::QuotedPrintable => Ok(Cow::Owned(qp_decode(body))),
            Self::Base64 => base64_decode(body).map(Cow::Owned),
            Self::Unknown(name) => Err(MimeError::UnknownEncoding(name.clone())),
        }
    }
}

/// Decodes base64, skipping white space. Padding ends a group, so padded chunks one after another decode as they
/// should; missing padding is forgiven.
pub(crate) fn base64_decode(input: &[u8]) -> Result<Vec<u8>, MimeError> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let (mut acc, mut n) = (0u32, 0u8);
    for &c in input {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                flush(&mut out, acc, n)?;
                (acc, n) = (0, 0);
                continue;
            }
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return Err(MimeError::BadBase64),
        };
        acc = (acc << 6) | u32::from(v);
        n += 1;
        if n == 4 {
            let [_, a, b, c] = acc.to_be_bytes();
            out.extend_from_slice(&[a, b, c]);
            (acc, n) = (0, 0);
        }
    }
    flush(&mut out, acc, n)?;
    Ok(out)
}

/// The bytes of a group of fewer than four characters.
fn flush(out: &mut Vec<u8>, acc: u32, n: u8) -> Result<(), MimeError> {
    match n {
        0 => {}
        2 => {
            let [_, a, _, _] = (acc << 12).to_be_bytes();
            out.push(a);
        }
        3 => {
            let [_, a, b, _] = (acc << 6).to_be_bytes();
            out.extend_from_slice(&[a, b]);
        }
        _ => return Err(MimeError::BadBase64),
    }
    Ok(())
}

fn hex(b: u8) -> Option<u8> {
    char::from(b).to_digit(16).and_then(|d| u8::try_from(d).ok())
}

/// Decodes quoted-printable: `=XX` is a byte, `=` at the end of a line joins it to the next, white space at the end
/// of a line is padding. An `=` that is neither stays as it is.
pub(crate) fn qp_decode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0usize;
    while let Some(&c) = input.get(i) {
        match c {
            b'=' => {
                if let (Some(h), Some(l)) = (input.get(i + 1).copied().and_then(hex), input.get(i + 2).copied().and_then(hex)) {
                    out.push((h << 4) | l);
                    i += 3;
                    continue;
                }
                let j = skip_blanks(input, i + 1);
                match input.get(j) {
                    Some(b'\r') if input.get(j + 1) == Some(&b'\n') => i = j + 2,
                    Some(b'\r' | b'\n') => i = j + 1,
                    None => i = j,
                    Some(_) => {
                        out.push(b'=');
                        i += 1;
                    }
                }
            }
            b' ' | b'\t' => {
                let j = skip_blanks(input, i);
                if !matches!(input.get(j), Some(b'\r' | b'\n') | None) {
                    out.extend_from_slice(input.get(i..j).unwrap_or_default());
                }
                i = j;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Where the run of spaces and tabs from `i` ends.
fn skip_blanks(input: &[u8], mut i: usize) -> usize {
    while matches!(input.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    i
}

#[cfg(test)]
#[path = "transfer_tests.rs"]
mod tests;
