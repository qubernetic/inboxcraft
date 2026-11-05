// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The writing side of the encodings: base64 and quoted-printable bodies (RFC 2045 section 6), folded header fields
//! (RFC 5322 section 2.2.3), RFC 2047 encoded words and RFC 2231 parameter pieces.

use crate::{MAX_FIELD_BYTES, MimeError};

/// The longest line a header field is folded to where it can be (RFC 5322 section 2.1.1).
const FOLD_AT: usize = 78;

/// The longest line of a base64 or quoted-printable body, line break not counted (RFC 2045 section 6.7 and 6.8).
const BODY_LINE: usize = 76;

/// The most bytes of text in one `=?UTF-8?B?...?=` word: 60 base64 characters, 72 with the frame.
const B_WORD_BYTES: usize = 45;

/// The longest RFC 2231 piece, in characters.
const PIECE_LEN: usize = 60;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The letter of a 6-bit value.
fn letter(value: u32) -> u8 {
    ALPHABET.get(usize::try_from(value & 63).unwrap_or_default()).copied().unwrap_or(b'A')
}

/// Base64 without line breaks.
fn base64_plain(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { letter(n >> (18 - 6 * i)) } else { b'=' });
        }
    }
    out
}

/// Base64 in lines of 76 characters joined by CRLF, without a line break at the end.
pub(crate) fn base64_encode(bytes: &[u8]) -> Vec<u8> {
    base64_plain(bytes).chunks(BODY_LINE).collect::<Vec<_>>().join(&b"\r\n"[..])
}

/// Text with every line end (CRLF, LF or a lone CR) as CRLF.
pub(crate) fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\r\n")
}

/// Quoted-printable (RFC 2045 section 6.7): CRLF stays a line break, `=`, control characters, 8-bit bytes and white
/// space at the end of a line become `=XX`, and longer lines get soft breaks so none passes 76 characters.
pub(crate) fn qp_encode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len().saturating_add(bytes.len() / 3));
    let mut rest = bytes;
    loop {
        let (line, next) = match rest.windows(2).position(|pair| pair == b"\r\n") {
            Some(i) => (rest.get(..i).unwrap_or_default(), Some(rest.get(i.saturating_add(2)..).unwrap_or_default())),
            None => (rest, None),
        };
        qp_line(line, &mut out);
        match next {
            Some(tail) => {
                out.extend_from_slice(b"\r\n");
                rest = tail;
            }
            None => return out,
        }
    }
}

/// One line of quoted-printable, with soft breaks.
fn qp_line(line: &[u8], out: &mut Vec<u8>) {
    let mut len = 0usize;
    for (i, &byte) in line.iter().enumerate() {
        let last = i.saturating_add(1) == line.len();
        let plain = (byte.is_ascii_graphic() && byte != b'=') || (matches!(byte, b' ' | b'\t') && !last);
        let token = if plain { vec![byte] } else { format!("={byte:02X}").into_bytes() };
        if len.saturating_add(token.len()) >= BODY_LINE {
            out.extend_from_slice(b"=\r\n");
            len = 0;
        }
        len = len.saturating_add(token.len());
        out.extend_from_slice(&token);
    }
}

/// Text from outside without CR, LF and NUL, which could end a header line or a protocol line, and without the
/// white space around it.
pub(crate) fn strip_breaks(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect::<String>().trim().to_owned()
}

/// A whole header field: `name`, a colon and `value`, folded at its spaces so lines stay within 78 characters where
/// the value allows, ended by CRLF. The value must hold no CR, LF or NUL already; folding only adds line breaks
/// before spaces, so unfolding gives the value back.
///
/// # Errors
///
/// [`MimeError::FieldTooLong`] when the field is longer than the reader takes, [`MAX_FIELD_BYTES`].
pub(crate) fn fold(name: &str, value: &str) -> Result<Vec<u8>, MimeError> {
    if name.len().saturating_add(1).saturating_add(value.len()) > MAX_FIELD_BYTES {
        return Err(MimeError::FieldTooLong { limit: MAX_FIELD_BYTES });
    }
    let mut out = format!("{name}:");
    let mut line = out.len();
    for (i, token) in value.split(' ').enumerate() {
        // The line before the break holds at least the name, so no line is only white space.
        if !token.is_empty() && line.saturating_add(1).saturating_add(token.len()) > FOLD_AT {
            out.push_str("\r\n");
            line = 0;
        }
        if i > 0 || !token.is_empty() {
            out.push(' ');
            out.push_str(token);
            line = line.saturating_add(1).saturating_add(token.len());
        }
    }
    out.push_str("\r\n");
    Ok(out.into_bytes())
}

/// The text as `=?UTF-8?B?...?=` words of at most 75 characters, split between characters (RFC 2047 sections 2 and
/// 5). Written with spaces between them, they read back as the text, the spaces gone.
pub(crate) fn b_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut chunk = String::new();
    for c in text.chars() {
        if chunk.len().saturating_add(c.len_utf8()) > B_WORD_BYTES {
            words.push(b_word(&chunk));
            chunk.clear();
        }
        chunk.push(c);
    }
    if !chunk.is_empty() {
        words.push(b_word(&chunk));
    }
    words
}

fn b_word(text: &str) -> String {
    format!("=?UTF-8?B?{}?=", String::from_utf8_lossy(&base64_plain(text.as_bytes())))
}

/// The text percent-encoded as RFC 2231 values (section 4): every byte that is no `attribute-char` as `%XX`, in
/// pieces of at most 60 characters split between characters.
pub(crate) fn percent_pieces(text: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut piece = String::new();
    for c in text.chars() {
        let mut buf = [0; 4];
        let encoded: String = c
            .encode_utf8(&mut buf)
            .bytes()
            .map(|b| if b.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&b) { char::from(b).to_string() } else { format!("%{b:02X}") })
            .collect();
        if piece.len().saturating_add(encoded.len()) > PIECE_LEN {
            pieces.push(std::mem::take(&mut piece));
        }
        piece.push_str(&encoded);
    }
    if !piece.is_empty() {
        pieces.push(piece);
    }
    pieces
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;
