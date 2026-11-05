// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Opening a connection: the address, TCP with timeouts, and TLS over it, at once or after STARTTLS.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::{ErrorKind, Host, Server};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

use crate::{NetError, TlsSettings};

/// How long to wait for a connection, and for the server on an open one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// For the TCP connection to open, per address.
    pub connect: Duration,
    /// For each read and write on an open connection.
    pub io: Duration,
}

impl Default for Timeouts {
    /// 30 seconds to connect, 120 seconds for each read and write.
    fn default() -> Self {
        Self { connect: Duration::from_secs(30), io: Duration::from_secs(120) }
    }
}

/// Where a host is: looked up by the system, or given.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Dial {
    Lookup,
    To(Vec<SocketAddr>),
}

/// What opens a TCP connection to an address: the operating system, or a test's stand-in that makes a port time out
/// or refuse without waiting for a real network.
pub trait Dialer: fmt::Debug + Send + Sync {
    /// A connection to `address`, waiting at most `timeout`.
    ///
    /// # Errors
    ///
    /// The socket's error: refused, timed out, unreachable.
    fn dial(&self, address: SocketAddr, timeout: Duration) -> io::Result<TcpStream>;
}

/// The operating system's [`Dialer`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemDialer;

impl Dialer for SystemDialer {
    fn dial(&self, address: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        TcpStream::connect_timeout(&address, timeout)
    }
}

/// Opens connections: TCP with [`Timeouts`], then TLS with the [`TlsSettings`].
#[derive(Debug, Clone)]
pub struct Connector {
    tls: TlsSettings,
    timeouts: Timeouts,
    dial: Dial,
    dialer: Arc<dyn Dialer>,
}

impl Connector {
    /// A connector that looks host names up with the system's resolver.
    #[must_use]
    pub fn new(tls: TlsSettings) -> Self {
        Self { tls, timeouts: Timeouts::default(), dial: Dial::Lookup, dialer: Arc::new(SystemDialer) }
    }

    /// With another [`Dialer`].
    #[must_use]
    pub fn with_dialer(self, dialer: Arc<dyn Dialer>) -> Self {
        Self { dialer, ..self }
    }

    /// With other timeouts.
    #[must_use]
    pub fn with_timeouts(self, timeouts: Timeouts) -> Self {
        Self { timeouts, ..self }
    }

    /// Connects to `addresses` instead of looking the host up; the certificate is still checked for the host's
    /// name. For the fake servers of the tests, which listen on 127.0.0.1.
    #[must_use]
    pub fn with_addresses(self, addresses: Vec<SocketAddr>) -> Self {
        Self { dial: Dial::To(addresses), ..self }
    }

    /// The TLS settings.
    #[must_use]
    pub fn tls(&self) -> &TlsSettings {
        &self.tls
    }

    /// The timeouts.
    #[must_use]
    pub fn timeouts(&self) -> Timeouts {
        self.timeouts
    }

    /// A TCP connection to `server`, not encrypted yet.
    pub(crate) fn tcp(&self, server: &Server) -> Result<Stream, NetError> {
        let mut last = NetError::new(ErrorKind::HostNotFound, format!("no address for {}", server.host()));
        for address in self.resolve(server)? {
            match self.dial(address) {
                Ok(stream) => return Ok(stream),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    /// The addresses of `server`: given, or looked up by the system; at least one.
    pub(crate) fn resolve(&self, server: &Server) -> Result<Vec<SocketAddr>, NetError> {
        let addresses: Vec<SocketAddr> = match &self.dial {
            Dial::To(addresses) => addresses.clone(),
            Dial::Lookup => (server.host().as_str(), server.port().get())
                .to_socket_addrs()
                .map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string()))?
                .collect(),
        };
        if addresses.is_empty() {
            return Err(NetError::new(ErrorKind::HostNotFound, format!("no address for {}", server.host())));
        }
        Ok(addresses)
    }

    /// A TCP connection to `address` with the timeouts set, not encrypted yet.
    pub(crate) fn dial(&self, address: SocketAddr) -> Result<Stream, NetError> {
        let now = self.tls.clock().now();
        let socket = self.dialer.dial(address, self.timeouts.connect).map_err(|error| NetError::io(&error, now))?;
        socket.set_read_timeout(Some(self.timeouts.io)).map_err(|error| NetError::io(&error, now))?;
        socket.set_write_timeout(Some(self.timeouts.io)).map_err(|error| NetError::io(&error, now))?;
        socket.set_nodelay(true).map_err(|error| NetError::io(&error, now))?;
        Ok(Stream::Plain(socket))
    }

    /// `stream` encrypted for `host`: the TLS handshake runs here, so a certificate fault comes out of this call.
    pub(crate) fn encrypt(&self, stream: Stream, host: &Host, config: &Arc<ClientConfig>) -> Result<Stream, NetError> {
        let now = self.tls.clock().now();
        let Stream::Plain(mut socket) = stream else { return Err(NetError::new(ErrorKind::Tls, "the connection is encrypted already")) };
        let name = ServerName::try_from(host.as_str().to_owned()).map_err(|error| NetError::new(ErrorKind::Tls, error.to_string()))?;
        let mut connection = ClientConnection::new(Arc::clone(config), name).map_err(|error| NetError::tls(&error, now))?;
        while connection.is_handshaking() {
            connection.complete_io(&mut socket).map_err(|error| NetError::io(&error, now))?;
        }
        Ok(Stream::Tls(Box::new(StreamOwned::new(connection, socket))))
    }
}

impl Stream {
    /// How long a read waits; `None` waits for ever.
    pub(crate) fn set_read_timeout(&self, wait: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Plain(socket) => socket.set_read_timeout(wait),
            Self::Tls(stream) => stream.sock.set_read_timeout(wait),
            Self::Closed => Err(io::ErrorKind::NotConnected.into()),
        }
    }

    /// Ends TLS with a `close_notify`, so the server knows the end was meant.
    pub(crate) fn close(&mut self) {
        if let Self::Tls(stream) = self {
            stream.conn.send_close_notify();
            let _sent = stream.flush();
        }
    }
}

/// A connection: plain TCP, or TLS over it.
#[derive(Debug)]
pub(crate) enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
    /// In the moment between the two, while the handshake runs.
    Closed,
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(socket) => socket.read(buf),
            Self::Tls(stream) => stream.read(buf),
            Self::Closed => Err(io::ErrorKind::NotConnected.into()),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(socket) => socket.write(buf),
            Self::Tls(stream) => stream.write(buf),
            Self::Closed => Err(io::ErrorKind::NotConnected.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(socket) => socket.flush(),
            Self::Tls(stream) => stream.flush(),
            Self::Closed => Err(io::ErrorKind::NotConnected.into()),
        }
    }
}
