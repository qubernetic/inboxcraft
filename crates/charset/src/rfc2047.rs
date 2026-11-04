// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! RFC 2047 encoded words, `=?charset?encoding?text?=`, in header text.

use std::fmt;

use crate::{Charset, CharsetError, Decoded, base64};

/// The longest encoded word [`encode_text`] writes (RFC 2047 section 2). Longer words are read.
pub const MAX_WORD_LEN: usize = 75;

/// The longest charset label, language included, read as part of an encoded word. Text with a longer one is not an
/// encoded word.
const MAX_LABEL_LEN: usize = 64;

/// What [`encode_text`] writes before the text of each word; `?=` follows it.
const Q_PREFIX: &str = "=?UTF-8?Q?";
const B_PREFIX: &str = "=?UTF-8?B?";

/// Room for the encoded text in one word.
const MAX_PAYLOAD: usize = MAX_WORD_LEN - Q_PREFIX.len() - "?=".len();

/// An encoded word that cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WordError {
    /// The text is not one encoded word.
    NotAWord,
    /// The charset of the word cannot be read.
    Charset(CharsetError),
    /// The encoding is neither B nor Q.
    UnknownEncoding(char),
    /// The B text is not base64.
    BadBase64,
    /// The Q text holds a character Q does not allow or an `=` without two hex digits.
    BadQ,
    /// The bytes do not fit the charset.
    Malformed,
}

impl fmt::Display for WordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAWord => f.write_str("not an encoded word"),
            Self::Charset(error) => error.fmt(f),
            Self::UnknownEncoding(encoding) => write!(f, "unknown encoding {encoding:?}, expected B or Q"),
            Self::BadBase64 => f.write_str("broken base64 in an encoded word"),
            Self::BadQ => f.write_str("broken Q encoding in an encoded word"),
            Self::Malformed => f.write_str("an encoded word holds bytes its character set cannot"),
        }
    }
}

impl std::error::Error for WordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Charset(error) => Some(error),
            _ => None,
        }
    }
}

/// Decodes one encoded word, the whole of `word`.
///
/// # Errors
///
/// A [`WordError`] when `word` is not exactly one encoded word or cannot be read.
pub fn decode_word(word: &str) -> Result<String, WordError> {
    let raw = shape(word).filter(|raw| raw.len == word.len()).ok_or(WordError::NotAWord)?;
    let (charset, bytes) = raw.decode()?;
    let decoded = charset.decode_piece(&bytes);
    if decoded.malformed { Err(WordError::Malformed) } else { Ok(decoded.text) }
}

/// Decodes the encoded words in header text (RFC 2047 section 6), leaving the rest as it is.
///
/// White space between two encoded words goes; adjacent words in one charset are joined before they are decoded, so
/// a character a sender split across two words reads whole. Words without white space around them are read too. A
/// word that cannot be read stays as it stands and sets [`Decoded::malformed`], as do bytes its charset cannot hold.
pub fn decode_text(text: &str) -> Decoded {
    let mut out = Words { text: String::with_capacity(text.len()), pending: None, malformed: false };
    // `copied` is where the text not yet copied starts; `pos` where the search for the next word goes on.
    let (mut copied, mut pos) = (0, 0);
    while let Some(start) = text.get(pos..).and_then(|rest| rest.find("=?")).map(|found| pos + found) {
        let Some(raw) = text.get(start..).and_then(shape) else {
            pos = start + 1;
            continue;
        };
        pos = start + raw.len;
        let Ok((charset, bytes)) = raw.decode() else {
            out.malformed = true;
            continue;
        };
        let between = text.get(copied..start).unwrap_or_default();
        if out.pending.is_none() || !between.bytes().all(is_white_space) {
            out.flush();
            out.text.push_str(between);
        }
        out.push(charset, bytes);
        copied = pos;
    }
    out.flush();
    out.text.push_str(text.get(copied..).unwrap_or_default());
    Decoded { text: out.text, malformed: out.malformed }
}

/// Writes header text with the parts that need it as encoded words in UTF-8.
///
/// CR, LF and NUL go first. A run of white-space separated parts that hold anything but printable ASCII, or that
/// look like an encoded word (`=?`, `?=`), becomes encoded words of at most [`MAX_WORD_LEN`] characters, split
/// between characters and separated by a space; the rest stays as it is. Q is used, which a person can still read,
/// unless B is shorter than two thirds of it. Folding the line is left to the header writer.
#[must_use]
pub fn encode_text(text: &str) -> String {
    let text: String = text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (start, end) in runs_to_encode(&text) {
        out.push_str(text.get(copied..start).unwrap_or_default());
        encode_run(text.get(start..end).unwrap_or_default(), &mut out);
        copied = end;
    }
    out.push_str(text.get(copied..).unwrap_or_default());
    out
}

/// Decoded text as it is built, with the bytes of the last words held back until a word in another charset or
/// plain text follows.
struct Words {
    text: String,
    pending: Option<(Charset, Vec<u8>)>,
    malformed: bool,
}

impl Words {
    fn push(&mut self, charset: Charset, bytes: Vec<u8>) {
        match &mut self.pending {
            Some((held, held_bytes)) if *held == charset => held_bytes.extend(bytes),
            _ => {
                self.flush();
                self.pending = Some((charset, bytes));
            }
        }
    }

    fn flush(&mut self) {
        if let Some((charset, bytes)) = self.pending.take() {
            let decoded = charset.decode_piece(&bytes);
            self.malformed |= decoded.malformed;
            self.text.push_str(&decoded.text);
        }
    }
}

/// An encoded word as it stands in the text.
struct RawWord<'a> {
    label: &'a str,
    encoding: char,
    text: &'a str,
    /// The length of the whole word in bytes.
    len: usize,
}

impl RawWord<'_> {
    fn decode(&self) -> Result<(Charset, Vec<u8>), WordError> {
        // RFC 2231 section 5: a language may follow the charset after `*`.
        let label = self.label.split('*').next().unwrap_or_default();
        let charset = Charset::for_label(label).map_err(WordError::Charset)?;
        let bytes = match self.encoding {
            'B' | 'b' => base64::decode(self.text.as_bytes()).ok_or(WordError::BadBase64)?,
            'Q' | 'q' => decode_q(self.text)?,
            other => return Err(WordError::UnknownEncoding(other)),
        };
        Ok((charset, bytes))
    }
}

/// Reads the shape of an encoded word at the start of `text`: a label, one letter or digit for the encoding, and
/// text without `?` or white space. Whether the parts can be read is [`RawWord::decode`]'s business.
fn shape(text: &str) -> Option<RawWord<'_>> {
    let rest = text.strip_prefix("=?")?;
    let label_len = rest.bytes().take(MAX_LABEL_LEN + 1).position(|b| b == b'?')?;
    let label = rest.get(..label_len)?;
    if label.is_empty() || !label.bytes().all(|b| b.is_ascii_graphic() && b != b'=') {
        return None;
    }
    let mut chars = rest.get(label_len + 1..)?.chars();
    let encoding = chars.next().filter(char::is_ascii_alphanumeric)?;
    if chars.next() != Some('?') {
        return None;
    }
    let rest = chars.as_str();
    let text_len = rest.find(|c: char| c == '?' || c.is_ascii_whitespace())?;
    if !rest.get(text_len..)?.starts_with("?=") {
        return None;
    }
    let len = text.len() - rest.len() + text_len + "?=".len();
    Some(RawWord { label, encoding, text: rest.get(..text_len)?, len })
}

/// The Q encoding (RFC 2047 section 4.2): `_` for a space, `=XX` for any byte, printable ASCII as it is.
fn decode_q(text: &str) -> Result<Vec<u8>, WordError> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'_' => out.push(b' '),
            b'=' => {
                let high = bytes.next().and_then(hex_value).ok_or(WordError::BadQ)?;
                let low = bytes.next().and_then(hex_value).ok_or(WordError::BadQ)?;
                out.push((high << 4) | low);
            }
            b'!'..=b'~' => out.push(byte),
            _ => return Err(WordError::BadQ),
        }
    }
    Ok(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    char::from(byte).to_digit(16).and_then(|digit| u8::try_from(digit).ok())
}

fn is_white_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

/// The byte ranges of the runs of parts to encode, each from the start of its first part to the end of its last.
///
/// Only a part that stays plain ends a run: white space between two encoded words would be lost on reading.
fn runs_to_encode(text: &str) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut run_open = false;
    for (start, part) in parts(text).filter(|(_, part)| !part.is_empty()) {
        let end = start + part.len();
        if !needs_encoding(part) {
            run_open = false;
        } else if let Some(last) = runs.last_mut().filter(|_| run_open) {
            last.1 = end;
        } else {
            runs.push((start, end));
            run_open = true;
        }
    }
    runs
}

/// The parts of the text between spaces and tabs, with their byte offsets.
fn parts(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split([' ', '\t']).scan(0, |offset, part| {
        let start = *offset;
        *offset = start + part.len() + 1;
        Some((start, part))
    })
}

fn needs_encoding(part: &str) -> bool {
    part.contains("=?") || part.contains("?=") || part.chars().any(|c| !c.is_ascii() || c.is_ascii_control())
}

fn encode_run(run: &str, out: &mut String) {
    let run_q_len: usize = run.bytes().map(q_len).sum();
    let use_b = base64::encoded_len(run.len()).saturating_mul(3) < run_q_len.saturating_mul(2);
    // The text of the word being filled, and its Q length.
    let (mut word, mut word_q_len) = (String::new(), 0);
    for c in run.chars() {
        let mut buf = [0; 4];
        let c_q_len: usize = c.encode_utf8(&mut buf).bytes().map(q_len).sum();
        let payload = if use_b { base64::encoded_len(word.len() + c.len_utf8()) } else { word_q_len + c_q_len };
        if payload > MAX_PAYLOAD && !word.is_empty() {
            push_word(&word, use_b, out);
            out.push(' ');
            (word, word_q_len) = (String::new(), 0);
        }
        word.push(c);
        word_q_len += c_q_len;
    }
    push_word(&word, use_b, out);
}

fn push_word(word: &str, use_b: bool, out: &mut String) {
    if use_b {
        out.push_str(B_PREFIX);
        out.push_str(&base64::encode(word.as_bytes()));
    } else {
        out.push_str(Q_PREFIX);
        word.bytes().for_each(|byte| push_q(byte, out));
    }
    out.push_str("?=");
}

/// Bytes Q writes as they are: those RFC 2047 section 5 allows in a phrase, so the words fit any header.
fn is_q_literal(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'!' | b'*' | b'+' | b'-' | b'/')
}

fn q_len(byte: u8) -> usize {
    if is_q_literal(byte) || byte == b' ' { 1 } else { 3 }
}

fn push_q(byte: u8, out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    if is_q_literal(byte) {
        out.push(char::from(byte));
    } else if byte == b' ' {
        out.push('_');
    } else {
        out.push('=');
        out.push(char::from(HEX.get(usize::from(byte >> 4)).copied().unwrap_or(b'0')));
        out.push(char::from(HEX.get(usize::from(byte & 0x0f)).copied().unwrap_or(b'0')));
    }
}

#[cfg(test)]
#[path = "rfc2047_tests.rs"]
mod tests;
