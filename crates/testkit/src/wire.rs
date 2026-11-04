// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The server's end of a connection: plain TCP, or TLS over it from the start or after STARTTLS, and either
//! compressed after COMPRESS=DEFLATE.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use rustls::{ServerConfig, ServerConnection, StreamOwned};

use crate::Error;
use crate::deflate::Deflate;
use crate::line::MAX_LINE;

/// The first byte of a TLS handshake record (RFC 8446 section 5.1).
const HANDSHAKE: u8 = 0x16;

/// How many lines a client may send in clear between the STARTTLS answer and its handshake.
const MAX_CLEAR_LINES: usize = 16;

#[derive(Debug)]
enum Tcp {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}

/// One accepted client connection.
#[derive(Debug)]
pub(crate) struct Wire {
    tcp: Tcp,
    deflate: Option<Box<Deflate>>,
}

impl Wire {
    /// A plain connection.
    pub(crate) fn plain(stream: TcpStream) -> Self {
        Self { tcp: Tcp::Plain(stream), deflate: None }
    }

    /// Whether the connection is encrypted.
    pub(crate) fn is_tls(&self) -> bool {
        matches!(self.tcp, Tcp::Tls(_))
    }

    /// Turns a plain connection into the server side of a TLS one; the handshake runs with the next read or write. A
    /// connection that already is TLS stays as it is.
    pub(crate) fn accept_tls(&mut self, config: &Arc<ServerConfig>) -> Result<(), Error> {
        let Tcp::Plain(stream) = &self.tcp else { return Ok(()) };
        let stream = stream.try_clone()?;
        self.tcp = Tcp::Tls(Box::new(StreamOwned::new(ServerConnection::new(Arc::clone(config))?, stream)));
        Ok(())
    }

    /// Reads the lines a client sends in clear after the STARTTLS answer and before its TLS handshake, which stays
    /// unread for rustls: up to [`MAX_CLEAR_LINES`] lines, each cut at [`MAX_LINE`] bytes. They are read a byte at a
    /// time, so not one byte of the handshake is taken with them.
    pub(crate) fn lines_before_handshake(&mut self) -> io::Result<Vec<Vec<u8>>> {
        let mut lines = Vec::new();
        let (Tcp::Plain(stream), None) = (&mut self.tcp, &self.deflate) else { return Ok(lines) };
        while lines.len() < MAX_CLEAR_LINES {
            let mut first = [0_u8; 1];
            if stream.peek(&mut first)? == 0 || first == [HANDSHAKE] {
                break;
            }
            let (line, whole) = raw_line(stream)?;
            lines.push(line);
            if !whole {
                break;
            }
        }
        Ok(lines)
    }

    /// Compresses both directions from now on.
    pub(crate) fn compress(&mut self) {
        self.deflate = Some(Box::new(Deflate::new()));
    }

    /// Ends the session in order: a TLS connection sends its close notification.
    pub(crate) fn close(&mut self) -> io::Result<()> {
        if let Tcp::Tls(stream) = &mut self.tcp {
            stream.conn.send_close_notify();
            stream.flush()?;
        }
        Ok(())
    }
}

/// Reads one line a byte at a time, without its terminator; whether it ended with LF rather than the connection or
/// [`MAX_LINE`].
fn raw_line(stream: &mut TcpStream) -> io::Result<(Vec<u8>, bool)> {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    while line.len() < MAX_LINE {
        if stream.read(&mut byte)? == 0 {
            return Ok((line, false));
        }
        if byte == *b"\n" {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok((line, true));
        }
        line.extend_from_slice(&byte);
    }
    Ok((line, false))
}

impl Read for Tcp {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buf),
            Self::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for Tcp {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(buf),
            Self::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
    }
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.deflate {
            Some(deflate) => deflate.read(&mut self.tcp, buf),
            None => self.tcp.read(buf),
        }
    }
}

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.deflate {
            Some(deflate) => deflate.write(&mut self.tcp, buf).map(|()| buf.len()),
            None => self.tcp.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tcp.flush()
    }
}
