// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading CRLF (or bare LF) terminated lines with a cap on their length.

use std::io::{self, Read};

/// The longest line, without its terminator, that the server and the client read.
pub(crate) const MAX_LINE: usize = 8192;

/// What one [`LineReader::read_line`] call found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Line {
    /// A whole line without its terminator.
    Complete(Vec<u8>),
    /// The peer closed the connection.
    Eof,
    /// More than [`MAX_LINE`] bytes came without a terminator.
    TooLong,
}

/// Buffers what was read past the end of the last line.
#[derive(Debug, Default)]
pub(crate) struct LineReader {
    buf: Vec<u8>,
}

impl LineReader {
    /// Reads the next line from `src`.
    pub(crate) fn read_line(&mut self, src: &mut impl Read) -> io::Result<Line> {
        loop {
            if let Some(end) = self.buf.iter().position(|&byte| byte == b'\n') {
                let mut line: Vec<u8> = self.buf.drain(..=end).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Ok(if line.len() > MAX_LINE { Line::TooLong } else { Line::Complete(line) });
            }
            if self.buf.len() > MAX_LINE {
                self.buf.clear();
                return Ok(Line::TooLong);
            }
            let mut chunk = [0_u8; 4096];
            let read = src.read(&mut chunk)?;
            if read == 0 {
                return Ok(Line::Eof);
            }
            self.buf.extend_from_slice(chunk.get(..read).unwrap_or_default());
        }
    }

    /// Reads exactly `len` bytes, such as an IMAP literal; `None` when the peer closed the connection first.
    pub(crate) fn read_bytes(&mut self, src: &mut impl Read, len: usize) -> io::Result<Option<Vec<u8>>> {
        while self.buf.len() < len {
            let mut chunk = [0_u8; 4096];
            let read = src.read(&mut chunk)?;
            if read == 0 {
                return Ok(None);
            }
            self.buf.extend_from_slice(chunk.get(..read).unwrap_or_default());
        }
        Ok(Some(self.buf.drain(..len).collect()))
    }

    /// Forgets what was read past the last line: bytes a client sent in clear after STARTTLS.
    pub(crate) fn discard(&mut self) {
        self.buf.clear();
    }
}
