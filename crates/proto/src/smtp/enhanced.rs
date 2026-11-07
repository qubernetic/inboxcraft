// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Enhanced status codes (RFC 3463, sent by servers that offer ENHANCEDSTATUSCODES, RFC 2034).

use std::fmt;

/// `class.subject.detail`, such as `5.1.1` (bad destination mailbox address): what a reply means, finer than its
/// three-digit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnhancedCode {
    class: u8,
    subject: u16,
    detail: u16,
}

impl EnhancedCode {
    /// 2 (success), 4 (transient failure) or 5 (permanent failure).
    #[must_use]
    pub fn class(self) -> u8 {
        self.class
    }

    /// What the code is about: 1 addressing, 2 mailbox, 3 mail system, 4 network, 5 protocol, 6 content, 7 policy.
    #[must_use]
    pub fn subject(self) -> u16 {
        self.subject
    }

    /// The detail within the subject.
    #[must_use]
    pub fn detail(self) -> u16 {
        self.detail
    }

    /// The code at the front of a reply's text and the text after it, or `None` and the text as it was. `class` is
    /// the first digit of the reply code; a code of another class is not taken.
    pub(crate) fn split(text: &str, class: u8) -> (Option<Self>, &str) {
        let (word, rest) = text.split_once(' ').unwrap_or((text, ""));
        let mut parts = word.split('.');
        let (Some(first), Some(subject), Some(detail), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            return (None, text);
        };
        match (first, number(subject), number(detail)) {
            ("2" | "4" | "5", Some(subject), Some(detail)) if first.as_bytes() == [b'0'.wrapping_add(class)] => {
                (Some(Self { class, subject, detail }), rest.trim_start())
            }
            _ => (None, text),
        }
    }
}

/// One to three digits.
fn number(text: &str) -> Option<u16> {
    if (1..=3).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit()) { text.parse().ok() } else { None }
}

impl fmt::Display for EnhancedCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.class, self.subject, self.detail)
    }
}

#[cfg(test)]
#[path = "enhanced_tests.rs"]
mod tests;
