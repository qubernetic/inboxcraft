// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The WHATWG character sets, through `encoding_rs`.

use std::fmt;

use encoding_rs::Encoding;

/// The most characters of a hostile label an error keeps.
pub const MAX_LABEL_IN_ERROR: usize = 64;

/// One of the character sets of the WHATWG Encoding Standard, the set browsers and mail programs read.
///
/// Labels are matched the WHATWG way: without case, around white space, `iso-8859-1` and `us-ascii` as
/// `windows-1252`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Charset(&'static Encoding);

impl Charset {
    /// UTF-8.
    pub const UTF_8: Self = Self(encoding_rs::UTF_8);

    /// The charset a label names, as a `charset=` parameter or an encoded word gives it.
    ///
    /// # Errors
    ///
    /// [`CharsetError::UnknownLabel`] when the WHATWG list has no such label (`utf-7`, `x-unknown`),
    /// [`CharsetError::Unsupported`] when it maps the label to its replacement encoding (`iso-2022-kr`,
    /// `hz-gb-2312`): text in those cannot be read.
    pub fn for_label(label: &str) -> Result<Self, CharsetError> {
        let label = label.trim_matches(|c: char| c.is_ascii_whitespace());
        match Encoding::for_label(label.as_bytes()) {
            Some(encoding) if encoding == encoding_rs::REPLACEMENT => Err(CharsetError::Unsupported(clean_label(label))),
            Some(encoding) => Ok(Self(encoding)),
            None => Err(CharsetError::UnknownLabel(clean_label(label))),
        }
    }

    /// The WHATWG name of the charset (`UTF-8`, `windows-1252`, `Shift_JIS`).
    #[must_use]
    pub fn name(self) -> &'static str {
        self.0.name()
    }

    /// Decodes bytes in this charset to text. A byte order mark of this charset at the start goes; bytes the
    /// charset cannot hold become U+FFFD and set [`Decoded::malformed`].
    pub fn decode(self, bytes: &[u8]) -> Decoded {
        let (text, malformed) = self.0.decode_with_bom_removal(bytes);
        Decoded { text: text.into_owned(), malformed }
    }
}

impl fmt::Debug for Charset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Charset").field(&self.name()).finish()
    }
}

impl fmt::Display for Charset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Decoded text.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct Decoded {
    /// The text, with U+FFFD where the input was broken.
    pub text: String,
    /// Whether the input was broken somewhere.
    pub malformed: bool,
}

/// Decodes bytes in the charset a label names to text.
///
/// # Errors
///
/// As [`Charset::for_label`].
pub fn decode(label: &str, bytes: &[u8]) -> Result<Decoded, CharsetError> {
    Charset::for_label(label).map(|charset| charset.decode(bytes))
}

/// A charset label that cannot be read.
///
/// The label is kept for the message, at most [`MAX_LABEL_IN_ERROR`] characters of it, control characters as U+FFFD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CharsetError {
    /// The WHATWG list has no such label.
    UnknownLabel(String),
    /// The WHATWG list names the label but cannot decode it.
    Unsupported(String),
}

impl fmt::Display for CharsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLabel(label) => write!(f, "unknown character set {label:?}"),
            Self::Unsupported(label) => write!(f, "character set {label:?} is not supported"),
        }
    }
}

impl std::error::Error for CharsetError {}

fn clean_label(label: &str) -> String {
    label.chars().take(MAX_LABEL_IN_ERROR).map(|c| if c.is_control() { '\u{fffd}' } else { c }).collect()
}

#[cfg(test)]
#[path = "charset_tests.rs"]
mod tests;
