// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The COMPRESS=DEFLATE stream (RFC 4978): raw DEFLATE (RFC 1951) both ways, inside TLS, from the byte after the
//! server's OK on.
//!
//! What goes out is compressed and ended with a sync flush, so the server can read each command at once. What comes
//! in is inflated in small steps, and each step's output goes on to the IMAP client before the next is inflated:
//! a stream that inflates a thousandfold never holds more than one step's output in memory, and the client's own
//! caps on lines and literals apply to what it inflates to.

use std::io::Write;

use flate2::Compression;
use flate2::write::{DeflateDecoder, DeflateEncoder};
use inboxcraft_model::ErrorKind;

use crate::NetError;

/// How many compressed bytes are inflated in one step: at DEFLATE's best ratio of about 1:1032 a step gives at most
/// about 64 KiB.
const STEP: usize = 64;

/// The DEFLATE state of the client's end of a connection.
pub(crate) struct Deflate {
    encoder: DeflateEncoder<Vec<u8>>,
    decoder: DeflateDecoder<Vec<u8>>,
}

impl Deflate {
    pub(crate) fn new() -> Self {
        Self { encoder: DeflateEncoder::new(Vec::new(), Compression::default()), decoder: DeflateDecoder::new(Vec::new()) }
    }

    /// `data` compressed and flushed, ready for the socket.
    pub(crate) fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>, NetError> {
        self.encoder.write_all(data).and_then(|()| self.encoder.flush()).map_err(|error| broken(&error))?;
        Ok(std::mem::take(self.encoder.get_mut()))
    }

    /// Inflates `data` from the socket step by step, handing each step's output to `take`.
    pub(crate) fn inflate(&mut self, data: &[u8], take: &mut dyn FnMut(&[u8]) -> Result<(), NetError>) -> Result<(), NetError> {
        for step in data.chunks(STEP) {
            self.decoder.write_all(step).and_then(|()| self.decoder.flush()).map_err(|error| broken(&error))?;
            let out = std::mem::take(self.decoder.get_mut());
            if !out.is_empty() {
                take(&out)?;
            }
        }
        Ok(())
    }
}

/// A stream that does not inflate, or a compressor that failed.
fn broken(error: &std::io::Error) -> NetError {
    NetError::new(ErrorKind::Protocol, format!("the compressed stream is broken: {error}"))
}

#[cfg(test)]
#[path = "compress_tests.rs"]
mod tests;
