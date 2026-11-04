// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! COMPRESS=DEFLATE (RFC 4978) for the fake servers and the test client: raw deflate (RFC 1951) both ways, each
//! write ended with a sync flush so the peer can read it at once.

use std::io::{self, Read, Write};

use flate2::Compression;
use flate2::write::{DeflateDecoder, DeflateEncoder};

/// The most bytes one read may inflate to before the stream counts as hostile.
const MAX_INFLATED: usize = 1 << 20;

/// How many compressed bytes are inflated between two checks of [`MAX_INFLATED`]: at deflate's best ratio of about
/// 1:1032 one step adds at most about 64 KiB.
const STEP: usize = 64;

/// The deflate state of one end of a connection.
#[derive(Debug)]
pub(crate) struct Deflate {
    encoder: DeflateEncoder<Vec<u8>>,
    decoder: DeflateDecoder<Vec<u8>>,
    /// How much of the decoder's output has been read.
    taken: usize,
}

impl Deflate {
    pub(crate) fn new() -> Self {
        Self { encoder: DeflateEncoder::new(Vec::new(), Compression::default()), decoder: DeflateDecoder::new(Vec::new()), taken: 0 }
    }

    /// Reads compressed bytes from `inner` and gives back what they inflate to.
    pub(crate) fn read(&mut self, inner: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let ready = self.decoder.get_ref().get(self.taken..).unwrap_or_default();
            if !ready.is_empty() {
                let len = ready.len().min(buf.len());
                if let (Some(to), Some(from)) = (buf.get_mut(..len), ready.get(..len)) {
                    to.copy_from_slice(from);
                }
                self.taken += len;
                if self.taken == self.decoder.get_ref().len() {
                    self.decoder.get_mut().clear();
                    self.taken = 0;
                }
                return Ok(len);
            }
            let mut chunk = [0_u8; 4096];
            let read = inner.read(&mut chunk)?;
            if read == 0 {
                return Ok(0);
            }
            for step in chunk.get(..read).unwrap_or_default().chunks(STEP) {
                self.decoder.write_all(step)?;
                self.decoder.flush()?;
                if self.decoder.get_ref().len() > MAX_INFLATED {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "the compressed stream inflates past the cap"));
                }
            }
        }
    }

    /// Compresses `data` and writes it to `inner` with a sync flush.
    pub(crate) fn write(&mut self, inner: &mut impl Write, data: &[u8]) -> io::Result<()> {
        self.encoder.write_all(data)?;
        self.encoder.flush()?;
        inner.write_all(self.encoder.get_ref())?;
        self.encoder.get_mut().clear();
        inner.flush()
    }
}
