// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Framing: whole responses (a line, and for as long as a line ends in `{n}`, n bytes and another line) out of the
//! bytes the server sent, however they were split up.
//!
//! The caps follow UCS-Care-Group/mailcraft `crates/proto/src/line.rs` and `imap/mod.rs` (see `PROVENANCE.md`);
//! the reader is new, it works on bytes handed in instead of a stream.

use super::value::{Piece, Status, literal_marker};
use crate::ProtoError;

/// Longest text one response may carry outside its literals, line endings not counted.
pub const MAX_LINE: usize = 1 << 20;
/// Largest literal one response may carry, and the most bytes of literals in one response.
pub const MAX_LITERAL: usize = 64 << 20;
/// Most literals one response may carry.
pub const MAX_LITERALS: usize = 64;

/// The bytes read so far and the response being put together.
#[derive(Debug, Default)]
pub(crate) struct Framer {
    buf: Vec<u8>,
    /// Where the bytes not handed out yet start.
    start: usize,
    /// How much of the pending bytes has been searched for a line feed already.
    scanned: usize,
    pieces: Vec<Piece>,
    /// The size of the literal the response waits for.
    literal: Option<usize>,
    text_len: usize,
    literal_len: usize,
}

impl Framer {
    /// Takes in what the server sent next.
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        if self.start > 0 {
            self.buf.drain(..self.start.min(self.buf.len()));
            self.start = 0;
        }
        self.buf.extend_from_slice(bytes);
    }

    /// Whether bytes were pushed that no response has taken yet.
    pub(crate) fn has_pending(&self) -> bool {
        self.start < self.buf.len()
    }

    fn pending(&self) -> &[u8] {
        self.buf.get(self.start..).unwrap_or_default()
    }

    /// The next whole response, `None` while more bytes are needed.
    pub(crate) fn next(&mut self) -> Result<Option<Vec<Piece>>, ProtoError> {
        loop {
            if let Some(size) = self.literal {
                if self.pending().len() < size {
                    return Ok(None);
                }
                // Moved out, not copied: the buffer must not hold the literal a second time.
                self.buf.drain(..self.start.min(self.buf.len()));
                let rest = self.buf.split_off(size.min(self.buf.len()));
                let literal = std::mem::replace(&mut self.buf, rest);
                self.start = 0;
                self.pieces.push(Piece::Literal(literal));
                self.literal = None;
                continue;
            }
            let Some(line) = self.line()? else { return Ok(None) };
            let first = self.pieces.is_empty();
            let marker = if first && !may_carry_literals(&line) { None } else { literal_marker(&line) };
            self.pieces.push(Piece::Text(line));
            let Some((_, size)) = marker else {
                self.text_len = 0;
                self.literal_len = 0;
                return Ok(Some(std::mem::take(&mut self.pieces)));
            };
            let size = usize::try_from(size)
                .ok()
                .filter(|size| self.literal_len.saturating_add(*size) <= MAX_LITERAL)
                .ok_or(ProtoError::LiteralTooLarge { size, limit: MAX_LITERAL })?;
            if self.pieces.len() / 2 >= MAX_LITERALS {
                return Err(ProtoError::TooManyLiterals { limit: MAX_LITERALS });
            }
            self.literal_len = self.literal_len.saturating_add(size);
            self.literal = Some(size);
        }
    }

    /// The next line without its line ending (CRLF, or a bare LF), counted against [`MAX_LINE`].
    fn line(&mut self) -> Result<Option<Vec<u8>>, ProtoError> {
        let pending = self.pending();
        let Some(at) = pending.get(self.scanned..).and_then(|rest| rest.iter().position(|b| *b == b'\n')) else {
            self.scanned = pending.len();
            // The limit is on the text itself; a CR may still come on top.
            if self.text_len.saturating_add(self.scanned) > MAX_LINE.saturating_add(1) {
                return Err(ProtoError::LineTooLong { limit: MAX_LINE });
            }
            return Ok(None);
        };
        let end = self.scanned.saturating_add(at);
        let line = pending.get(..end).unwrap_or_default();
        let line = line.strip_suffix(b"\r").unwrap_or(line).to_vec();
        self.text_len = self.text_len.saturating_add(line.len());
        if self.text_len > MAX_LINE {
            return Err(ProtoError::LineTooLong { limit: MAX_LINE });
        }
        self.start = self.start.saturating_add(end).saturating_add(1);
        self.scanned = 0;
        Ok(Some(line))
    }
}

/// Whether the first line of a response may end in a literal: only untagged data may. In a status response
/// (`* OK`, `A1 NO`) and a continuation request the rest is text for people, where `{5}` is no literal.
fn may_carry_literals(line: &[u8]) -> bool {
    let Some(rest) = line.strip_prefix(b"* ") else { return false };
    let word = rest.split(|b| *b == b' ').next().unwrap_or_default();
    Status::parse(word).is_none()
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
