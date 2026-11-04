// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A small line client that drives a fake server and names what went wrong.

use std::fmt;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::line::{Line, LineReader};

/// What kind of failure a [`ClientError`] is. Each fault of the fault script gives its own kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The server's chain ends in a CA the trust store does not hold.
    UnknownCa,
    /// The server's certificate has expired.
    Expired,
    /// The server's certificate is for another host name.
    WrongHost,
    /// The certificate is outside its validity at the client's clock, and that clock is far off.
    ClockFarOff,
    /// Any other TLS failure.
    Tls,
    /// The server closed the connection.
    Dropped,
    /// The server did not answer within the timeout.
    Timeout,
    /// The server sent a line that is not printable UTF-8, or one that is too long.
    Garbage,
    /// Any other socket failure, such as a refused connection.
    Io,
}

/// A client failure: its kind and the underlying message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientError {
    kind: ErrorKind,
    detail: String,
}

impl ClientError {
    fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        Self { kind, detail: detail.into() }
    }

    /// What kind of failure this is.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    fn from_io(err: &io::Error) -> Self {
        let kind = match err.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ErrorKind::Timeout,
            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe => {
                ErrorKind::Dropped
            }
            _ => ErrorKind::Io,
        };
        Self::new(kind, err.to_string())
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for ClientError {}

/// A line client connected to a fake server.
#[derive(Debug)]
pub struct TestClient {
    stream: TcpStream,
    reader: LineReader,
}

impl TestClient {
    /// Connects over plain TCP; `timeout` bounds the connect and every read and write.
    ///
    /// # Errors
    ///
    /// A [`ClientError`] of kind [`ErrorKind::Io`] or [`ErrorKind::Timeout`].
    pub fn plain(addr: SocketAddr, timeout: Duration) -> Result<Self, ClientError> {
        let stream = connect(addr, timeout).map_err(|err| ClientError::from_io(&err))?;
        Ok(Self { stream, reader: LineReader::default() })
    }

    /// Reads one line, without its terminator.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::Dropped`] when the server closed the connection, [`ErrorKind::Timeout`] when it did not answer,
    /// [`ErrorKind::Garbage`] for bytes that are not a printable UTF-8 line.
    pub fn read_line(&mut self) -> Result<String, ClientError> {
        let line = self.reader.read_line(&mut self.stream).map_err(|err| ClientError::from_io(&err))?;
        match line {
            Line::Complete(bytes) => printable(bytes),
            Line::Eof => Err(ClientError::new(ErrorKind::Dropped, "the server closed the connection")),
            Line::TooLong => Err(ClientError::new(ErrorKind::Garbage, "the server sent a line that is too long")),
        }
    }

    /// Sends one line; CR, LF and NUL are stripped and CRLF is added.
    ///
    /// # Errors
    ///
    /// A [`ClientError`] from the socket.
    pub fn send_line(&mut self, line: &str) -> Result<(), ClientError> {
        let mut bytes: Vec<u8> = line.bytes().filter(|byte| !matches!(byte, b'\r' | b'\n' | b'\0')).collect();
        bytes.extend_from_slice(b"\r\n");
        self.stream.write_all(&bytes).and_then(|()| self.stream.flush()).map_err(|err| ClientError::from_io(&err))
    }
}

fn connect(addr: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}

fn printable(bytes: Vec<u8>) -> Result<String, ClientError> {
    match String::from_utf8(bytes) {
        Ok(line) if !line.chars().any(|c| c.is_control() && c != '\t') => Ok(line),
        _ => Err(ClientError::new(ErrorKind::Garbage, "the server sent a line that is not printable UTF-8")),
    }
}
