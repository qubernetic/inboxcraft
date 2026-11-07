// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Commands as bytes: words, quoted strings and synchronising literals (RFC 9051 section 4.3).
//!
//! The quoting rules follow UCS-Care-Group/mailcraft `crates/proto/src/imap/mod.rs` (`Command`, see
//! `PROVENANCE.md`); here a command is split into the chunks the server must ask for one by one.

use std::collections::VecDeque;

/// Longest string sent in quotes; a longer one goes as a literal.
pub(crate) const MAX_QUOTED: usize = 1_000;

/// A command being put together, without its tag.
pub(crate) struct Command {
    /// Every chunk but the last ends in a literal's announcement; the next one starts with the literal.
    chunks: Vec<Vec<u8>>,
}

impl Command {
    pub(crate) fn new(verb: &str) -> Self {
        Self { chunks: vec![verb.as_bytes().to_vec()] }
    }

    fn text(&mut self, bytes: &[u8]) {
        if let Some(last) = self.chunks.last_mut() {
            last.extend_from_slice(bytes);
        }
    }

    /// A bare argument: our own words, or text that was checked.
    pub(crate) fn arg(mut self, word: &str) -> Self {
        self.text(b" ");
        self.text(word.as_bytes());
        self
    }

    /// Our own text right after what came before, without a space: `(` and `)` of a list.
    pub(crate) fn raw(mut self, text: &str) -> Self {
        self.text(text.as_bytes());
        self
    }

    /// An argument as a quoted string, or as a literal when quotes cannot carry it (8-bit, a control character, a
    /// long text). The caller keeps NUL out: no IMAP string carries it.
    pub(crate) fn string(mut self, bytes: &[u8]) -> Self {
        if bytes.len() <= MAX_QUOTED && bytes.iter().all(|b| matches!(b, b' '..=b'~')) {
            let mut quoted = Vec::with_capacity(bytes.len().saturating_add(2));
            quoted.push(b'"');
            quoted.extend(escaped(bytes));
            quoted.push(b'"');
            self.text(b" ");
            self.text(&quoted);
            self
        } else {
            self.literal(bytes)
        }
    }

    /// An argument as a synchronising literal, whatever it holds: the server's go-ahead comes before it.
    pub(crate) fn literal(mut self, bytes: &[u8]) -> Self {
        self.text(format!(" {{{}}}\r\n", bytes.len()).as_bytes());
        self.chunks.push(bytes.to_vec());
        self
    }

    /// The bytes to write now, and the chunks to write after each go-ahead (`+`) of the server.
    pub(crate) fn finish(self, tag: &str) -> (Vec<u8>, VecDeque<Vec<u8>>) {
        let mut chunks: VecDeque<Vec<u8>> = self.chunks.into();
        if let Some(last) = chunks.back_mut() {
            last.extend_from_slice(b"\r\n");
        }
        let mut first = format!("{tag} ").into_bytes();
        first.extend(chunks.pop_front().unwrap_or_default());
        (first, chunks)
    }
}

/// What goes between the quotes of a quoted string: `"` and `\` with a `\` before them.
pub(crate) fn escaped(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for b in bytes {
        if matches!(b, b'"' | b'\\') {
            out.push(b'\\');
        }
        out.push(*b);
    }
    out
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
