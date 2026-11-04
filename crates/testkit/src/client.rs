// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A small line client that drives a fake server and names what went wrong.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{CertificateError, ClientConnection, StreamOwned};

use crate::line::{Line, LineReader};
use crate::pki::TlsClient;

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

    /// Classifies a socket error; a TLS error inside it is classified by [`ClientError::from_tls`].
    fn from_io(err: &io::Error, far_off: bool) -> Self {
        if let Some(tls) = err.get_ref().and_then(|inner| inner.downcast_ref::<rustls::Error>()) {
            return Self::from_tls(tls, far_off);
        }
        let kind = match err.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ErrorKind::Timeout,
            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe => {
                ErrorKind::Dropped
            }
            _ => ErrorKind::Io,
        };
        Self::new(kind, err.to_string())
    }

    /// Classifies a TLS error. A certificate outside its validity counts as [`ErrorKind::ClockFarOff`] whenever the
    /// client's clock is far off, because then the clock, not the certificate, is the thing to fix first.
    fn from_tls(err: &rustls::Error, far_off: bool) -> Self {
        use CertificateError::{Expired, ExpiredContext, NotValidForName, NotValidForNameContext, NotValidYet, NotValidYetContext, UnknownIssuer};
        let kind = match err {
            rustls::Error::InvalidCertificate(UnknownIssuer) => ErrorKind::UnknownCa,
            rustls::Error::InvalidCertificate(Expired | ExpiredContext { .. } | NotValidYet | NotValidYetContext { .. }) if far_off => {
                ErrorKind::ClockFarOff
            }
            rustls::Error::InvalidCertificate(Expired | ExpiredContext { .. }) => ErrorKind::Expired,
            rustls::Error::InvalidCertificate(NotValidForName | NotValidForNameContext { .. }) => ErrorKind::WrongHost,
            _ => ErrorKind::Tls,
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

/// The client's connection: plain TCP or TLS over it.
#[derive(Debug)]
enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buf),
            Self::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for Stream {
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

/// A line client connected to a fake server.
#[derive(Debug)]
pub struct TestClient {
    stream: Stream,
    reader: LineReader,
    far_off: bool,
}

impl TestClient {
    /// Connects over plain TCP; `timeout` bounds the connect and every read and write.
    ///
    /// # Errors
    ///
    /// A [`ClientError`] of kind [`ErrorKind::Io`] or [`ErrorKind::Timeout`].
    pub fn plain(addr: SocketAddr, timeout: Duration) -> Result<Self, ClientError> {
        let stream = connect(addr, timeout).map_err(|err| ClientError::from_io(&err, false))?;
        Ok(Self { stream: Stream::Plain(stream), reader: LineReader::default(), far_off: false })
    }

    /// Connects with implicit TLS and completes the handshake, so a certificate fault shows up here.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::UnknownCa`], [`ErrorKind::Expired`], [`ErrorKind::WrongHost`] or [`ErrorKind::ClockFarOff`] for
    /// the certificate faults, [`ErrorKind::Tls`] for any other TLS failure (a host that is not a DNS name, a server
    /// that does not speak TLS), and the socket kinds of [`TestClient::plain`].
    pub fn tls(addr: SocketAddr, tls: &TlsClient, timeout: Duration) -> Result<Self, ClientError> {
        let sock = connect(addr, timeout).map_err(|err| ClientError::from_io(&err, tls.clock.is_far_off()))?;
        Self::handshake(sock, tls)
    }

    /// Turns a plain connection into TLS, after the server answered STARTTLS, and completes the handshake.
    ///
    /// # Errors
    ///
    /// The kinds of [`TestClient::tls`]; [`ErrorKind::Tls`] when the connection already is TLS.
    pub fn start_tls(self, tls: &TlsClient) -> Result<Self, ClientError> {
        let Stream::Plain(sock) = self.stream else { return Err(ClientError::new(ErrorKind::Tls, "the connection already is TLS")) };
        Self::handshake(sock, tls)
    }

    fn handshake(mut sock: TcpStream, tls: &TlsClient) -> Result<Self, ClientError> {
        let far_off = tls.clock.is_far_off();
        let name = ServerName::try_from(tls.host.clone()).map_err(|err| ClientError::new(ErrorKind::Tls, err.to_string()))?;
        let mut conn = ClientConnection::new(Arc::clone(&tls.config), name).map_err(|err| ClientError::from_tls(&err, far_off))?;
        while conn.is_handshaking() {
            conn.complete_io(&mut sock).map_err(|err| ClientError::from_io(&err, far_off))?;
        }
        Ok(Self { stream: Stream::Tls(Box::new(StreamOwned::new(conn, sock))), reader: LineReader::default(), far_off })
    }

    /// Reads one line, without its terminator.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::Dropped`] when the server closed the connection, [`ErrorKind::Timeout`] when it did not answer,
    /// [`ErrorKind::Garbage`] for bytes that are not a printable UTF-8 line.
    pub fn read_line(&mut self) -> Result<String, ClientError> {
        let line = self.reader.read_line(&mut self.stream).map_err(|err| ClientError::from_io(&err, self.far_off))?;
        match line {
            Line::Complete(bytes) => printable(bytes),
            Line::Eof => Err(ClientError::new(ErrorKind::Dropped, "the server closed the connection")),
            Line::TooLong => Err(ClientError::new(ErrorKind::Garbage, "the server sent a line that is too long")),
        }
    }

    /// Sends a tagged command line and reads the server's lines up to and including the one with its tag.
    ///
    /// # Errors
    ///
    /// The kinds of [`TestClient::send_line`] and [`TestClient::read_line`].
    pub fn command(&mut self, line: &str) -> Result<Vec<String>, ClientError> {
        let tag = format!("{} ", line.split(' ').next().unwrap_or_default());
        self.send_line(line)?;
        let mut lines = Vec::new();
        loop {
            let got = self.read_line()?;
            let done = got.starts_with(&tag);
            lines.push(got);
            if done {
                return Ok(lines);
            }
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
        let far_off = self.far_off;
        self.stream.write_all(&bytes).and_then(|()| self.stream.flush()).map_err(|err| ClientError::from_io(&err, far_off))
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
