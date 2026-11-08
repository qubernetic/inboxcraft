// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Server replies (RFC 5321 section 4.2): a three-digit code and text, on one line or several.

use crate::ProtoError;

/// Longest reply line taken, without its line end. RFC 5321 allows 512 octets; servers write longer ones.
pub const MAX_REPLY_LINE: usize = 4 * 1024;

/// Most lines one reply may have.
pub const MAX_REPLY_LINES: usize = 128;

/// A whole reply: its code and the text of each line, as the server wrote it (not yet fit to show).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Reply {
    pub(crate) code: u16,
    pub(crate) lines: Vec<String>,
}

impl Reply {
    /// 2yz: the command is done.
    pub(crate) fn is_positive(&self) -> bool {
        self.code / 100 == 2
    }

    /// 3yz: the server waits for more (the message after DATA, the next step of AUTH).
    pub(crate) fn is_intermediate(&self) -> bool {
        self.code / 100 == 3
    }

    /// 4yz: refused for now; the same command may work later.
    pub(crate) fn is_transient(&self) -> bool {
        self.code / 100 == 4
    }

    /// 5yz: refused for good.
    pub(crate) fn is_permanent(&self) -> bool {
        self.code / 100 == 5
    }
}

/// Cuts the bytes the server sends into replies.
#[derive(Default)]
pub(crate) struct Reader {
    buffer: Vec<u8>,
    code: Option<u16>,
    lines: Vec<String>,
}

impl Reader {
    /// Takes in bytes, in pieces of any size.
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Whether bytes were taken in that are not part of a reply handed out yet.
    pub(crate) fn has_pending(&self) -> bool {
        !self.buffer.is_empty() || !self.lines.is_empty()
    }

    /// The next whole reply, `None` until one has arrived.
    ///
    /// # Errors
    ///
    /// [`ProtoError::LineTooLong`], [`ProtoError::TooManyLines`] or [`ProtoError::Malformed`]; all are fatal.
    pub(crate) fn next(&mut self) -> Result<Option<Reply>, ProtoError> {
        loop {
            let Some(end) = self.buffer.iter().position(|b| *b == b'\n') else {
                if self.buffer.len() > MAX_REPLY_LINE {
                    return Err(ProtoError::LineTooLong { limit: MAX_REPLY_LINE });
                }
                return Ok(None);
            };
            let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.len() > MAX_REPLY_LINE {
                return Err(ProtoError::LineTooLong { limit: MAX_REPLY_LINE });
            }
            let (code, last, text) = split(&line)?;
            if self.code.is_some_and(|first| first != code) {
                return Err(ProtoError::Malformed("reply code"));
            }
            self.code = Some(code);
            self.lines.push(String::from_utf8_lossy(text).into_owned());
            if last {
                self.code = None;
                return Ok(Some(Reply { code, lines: std::mem::take(&mut self.lines) }));
            }
            if self.lines.len() >= MAX_REPLY_LINES {
                return Err(ProtoError::TooManyLines { limit: MAX_REPLY_LINES });
            }
        }
    }
}

/// A line's code, whether it is the last line of its reply, and its text.
fn split(line: &[u8]) -> Result<(u16, bool, &[u8]), ProtoError> {
    let malformed = ProtoError::Malformed("reply code");
    let (Some(&[first, second, third]), rest) = (line.get(..3), line.get(3..).unwrap_or_default()) else {
        return Err(malformed);
    };
    if !(b'2'..=b'5').contains(&first) || !(b'0'..=b'5').contains(&second) || !third.is_ascii_digit() {
        return Err(malformed);
    }
    let code = u16::from(first - b'0') * 100 + u16::from(second - b'0') * 10 + u16::from(third - b'0');
    match rest.split_first() {
        None => Ok((code, true, rest)),
        Some((b' ', text)) => Ok((code, true, text)),
        Some((b'-', text)) => Ok((code, false, text)),
        Some(_) => Err(malformed),
    }
}

#[cfg(test)]
#[path = "reply_tests.rs"]
mod tests;
