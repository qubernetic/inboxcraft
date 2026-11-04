// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the server says it can do (RFC 9051 section 7.2.2).

/// Most capabilities kept from one list.
pub const MAX_CAPABILITIES: usize = 256;
/// Longest capability kept, in bytes.
pub const MAX_CAPABILITY_LEN: usize = 128;

/// The capabilities the server listed, upper-cased, each once and in its order. A word that is no IMAP atom, or
/// longer than [`MAX_CAPABILITY_LEN`], is left out; after [`MAX_CAPABILITIES`] the rest is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities(Vec<String>);

impl Capabilities {
    /// The capabilities in `words`, as a server lists them.
    pub(crate) fn from_words<'a>(words: impl IntoIterator<Item = &'a str>) -> Self {
        let mut kept: Vec<String> = Vec::new();
        for word in words {
            if kept.len() >= MAX_CAPABILITIES {
                break;
            }
            if word.is_empty() || word.len() > MAX_CAPABILITY_LEN || !word.bytes().all(is_atom_char) {
                continue;
            }
            let word = word.to_ascii_uppercase();
            if !kept.contains(&word) {
                kept.push(word);
            }
        }
        Self(kept)
    }

    /// Whether the server listed `capability` (`STARTTLS`, `AUTH=PLAIN`), whatever the letter case.
    #[must_use]
    pub fn has(&self, capability: &str) -> bool {
        self.0.iter().any(|kept| kept.eq_ignore_ascii_case(capability))
    }

    /// The capabilities, upper-cased.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

/// A character of an IMAP atom (RFC 9051 `ATOM-CHAR`).
pub(crate) fn is_atom_char(b: u8) -> bool {
    b.is_ascii_graphic() && !matches!(b, b'(' | b')' | b'{' | b'%' | b'*' | b'"' | b'\\' | b']')
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
