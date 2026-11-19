// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Opening a connection: the address, TCP with timeouts, and TLS over it, at once or after STARTTLS.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use inboxcraft_model::{ErrorKind, Host, Server};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

use crate::{Deadline, NetError, TlsSettings};

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

/// The most ports [`Connector::with_addresses_on`] keeps addresses for.
const MAX_PORTS: usize = 16;

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

/// What finds the addresses of a host name in place of the system: a test's stand-in, so that no test looks a real
/// name up.
pub trait Resolve: fmt::Debug + Send + Sync {
    /// The addresses of `host` for `port`.
    ///
    /// # Errors
    ///
    /// The cause, when the name has none.
    fn resolve(&self, host: &Host, port: u16) -> io::Result<Vec<SocketAddr>>;
}

/// The most addresses of one host tried.
const MAX_ADDRESSES: usize = 16;

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
    /// Given addresses for the servers on a port, before `dial`.
    on_port: Vec<(u16, Vec<SocketAddr>)>,
    dialer: Arc<dyn Dialer>,
    /// Looks host names up in place of the system.
    resolver: Option<Arc<dyn Resolve>>,
    /// The end of everything this connector does, for a probe: none for the sync, IDLE and send connections.
    deadline: Option<Deadline>,
    /// The most addresses of one host tried.
    most_addresses: usize,
}

impl Connector {
    /// A connector that looks host names up with the system's resolver.
    #[must_use]
    pub fn new(tls: TlsSettings) -> Self {
        Self {
            tls,
            timeouts: Timeouts::default(),
            dial: Dial::Lookup,
            on_port: Vec::new(),
            dialer: Arc::new(SystemDialer),
            resolver: None,
            deadline: None,
            most_addresses: MAX_ADDRESSES,
        }
    }

    /// Ends everything it does at `deadline`: the name lookup (the system's on a thread of its own, waited for at
    /// most until then), each connection attempt, and every read and write on the socket under TLS ([`Timed`]), so
    /// the handshake and every answer end there however slowly a server sends or reads. For Detect's probe only: it
    /// is crate-private, and [`crate::probe`] is its one caller, so the sync, IDLE, send and check connections never
    /// have a deadline.
    #[must_use]
    pub(crate) fn with_deadline(self, deadline: Deadline) -> Self {
        Self { deadline: Some(deadline), ..self }
    }

    /// Tries at most `most` addresses of a host (at least one, at most 16).
    #[must_use]
    pub fn with_most_addresses(self, most: usize) -> Self {
        Self { most_addresses: most.clamp(1, MAX_ADDRESSES), ..self }
    }

    /// Looks host names up with `resolver` in place of the system's (the given addresses still come first). For the
    /// tests, whose fake servers listen on 127.0.0.1 under made-up names.
    #[must_use]
    pub fn with_resolver(self, resolver: Arc<dyn Resolve>) -> Self {
        Self { resolver: Some(resolver), ..self }
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

    /// Connects to `addresses` for a server on `port` (the IMAP and the submission server of an account are told
    /// apart by it), before [`Connector::with_addresses`] and the lookup. For the fake servers of the tests. A later
    /// call for the same port replaces the earlier one; past 16 ports the call is ignored.
    #[must_use]
    pub fn with_addresses_on(mut self, port: u16, addresses: Vec<SocketAddr>) -> Self {
        self.on_port.retain(|(held, _)| *held != port);
        if self.on_port.len() < MAX_PORTS {
            self.on_port.push((port, addresses));
        }
        self
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
            if let Some(deadline) = &self.deadline
                && let Err(error) = deadline.wait(self.timeouts.connect)
            {
                return Err(error);
            }
            match self.dial(address) {
                Ok(stream) => return Ok(stream),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    /// The addresses of `server`: given, or looked up by the system; at least one.
    pub(crate) fn resolve(&self, server: &Server) -> Result<Vec<SocketAddr>, NetError> {
        let port = server.port().get();
        let given = self.on_port.iter().find(|(held, _)| *held == port).map(|(_, addresses)| addresses);
        let addresses: Vec<SocketAddr> = match (given, &self.dial) {
            (Some(addresses), _) | (None, Dial::To(addresses)) => addresses.clone(),
            (None, Dial::Lookup) => match &self.resolver {
                Some(resolver) => resolver
                    .resolve(server.host(), port)
                    .map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string()))?
                    .into_iter()
                    .take(self.most_addresses)
                    .collect(),
                None => match &self.deadline {
                    Some(deadline) => {
                        lookup_by(Arc::new(SystemLookup), server.host(), port, deadline)?.into_iter().take(self.most_addresses).collect()
                    }
                    None => (server.host().as_str(), port)
                        .to_socket_addrs()
                        .map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string()))?
                        .take(self.most_addresses)
                        .collect(),
                },
            },
        };
        if addresses.is_empty() {
            return Err(NetError::new(ErrorKind::HostNotFound, format!("no address for {}", server.host())));
        }
        Ok(addresses)
    }

    /// A TCP connection to `address` with the timeouts set, not encrypted yet.
    pub(crate) fn dial(&self, address: SocketAddr) -> Result<Stream, NetError> {
        let now = self.tls.clock().now();
        let connect = match &self.deadline {
            Some(deadline) => deadline.wait(self.timeouts.connect)?,
            None => self.timeouts.connect,
        };
        let io = self.timeouts.io;
        let socket = self.dialer.dial(address, connect).map_err(|error| NetError::io(&error, now))?;
        socket.set_read_timeout(Some(io)).map_err(|error| NetError::io(&error, now))?;
        socket.set_write_timeout(Some(io)).map_err(|error| NetError::io(&error, now))?;
        socket.set_nodelay(true).map_err(|error| NetError::io(&error, now))?;
        Ok(Stream::Plain(Timed::new(socket, self.deadline.clone(), io)))
    }

    /// `stream` encrypted for `host`: the TLS handshake runs here, so a certificate fault comes out of this call.
    pub(crate) fn encrypt(&self, stream: Stream, host: &Host, config: &Arc<ClientConfig>) -> Result<Stream, NetError> {
        let now = self.tls.clock().now();
        let Stream::Plain(mut socket) = stream else { return Err(NetError::new(ErrorKind::Tls, "the connection is encrypted already")) };
        let name = ServerName::try_from(host.as_str().to_owned()).map_err(|error| NetError::new(ErrorKind::Tls, error.to_string()))?;
        let mut connection = ClientConnection::new(Arc::clone(config), name).map_err(|error| NetError::tls(&error, now))?;
        // The socket ends every read and write at the probe's deadline ([`Timed`]), inside rustls' own loops too.
        while connection.is_handshaking() {
            connection.complete_io(&mut socket).map_err(|error| NetError::io(&error, now))?;
        }
        Ok(Stream::Tls(Box::new(StreamOwned::new(connection, socket))))
    }
}

/// The most name lookups on threads of their own that may run at once in the process, across Detect's runs: a lookup
/// a probe gave up on goes on until the system's resolver gives up, and these are what is left.
pub(crate) const MAX_LOOKUPS: usize = 8;

/// The lookups on threads of their own running now.
static LOOKUPS: AtomicUsize = AtomicUsize::new(0);

/// One of the [`MAX_LOOKUPS`] places, given back when it is dropped (when the lookup's thread ends).
struct LookupSlot;

impl LookupSlot {
    fn take() -> Option<Self> {
        LOOKUPS
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |running| if running < MAX_LOOKUPS { running.checked_add(1) } else { None })
            .ok()
            .map(|_| Self)
    }
}

impl Drop for LookupSlot {
    fn drop(&mut self) {
        let _ = LOOKUPS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |running| running.checked_sub(1));
    }
}

/// The system's resolver, which has no timeout of ours.
#[derive(Debug)]
struct SystemLookup;

impl Resolve for SystemLookup {
    fn resolve(&self, host: &Host, port: u16) -> io::Result<Vec<SocketAddr>> {
        (host.as_str(), port).to_socket_addrs().map(|addresses| addresses.take(MAX_ADDRESSES).collect())
    }
}

/// `resolver`'s addresses of `host` on `port`, looked up on a thread of its own and waited for at most until
/// `deadline`. A lookup still running then is left to end by itself; while [`MAX_LOOKUPS`] of them run, a new one
/// fails at once as timed out instead of starting.
fn lookup_by(resolver: Arc<dyn Resolve>, host: &Host, port: u16, deadline: &Deadline) -> Result<Vec<SocketAddr>, NetError> {
    let wait = deadline.wait(Duration::MAX)?;
    let Some(slot) = LookupSlot::take() else {
        return Err(NetError::new(ErrorKind::TimedOut, format!("the name {host} was not looked up: {MAX_LOOKUPS} lookups are still running")));
    };
    let (sender, receiver) = mpsc::channel();
    let name = host.clone();
    std::thread::Builder::new()
        .name("inboxcraft-lookup".to_owned())
        .spawn(move || {
            let _slot = slot;
            let found = resolver.resolve(&name, port);
            // Nobody listens once the wait is over.
            let _ = sender.send(found);
        })
        .map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string()))?;
    match receiver.recv_timeout(wait) {
        Ok(found) => found.map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string())),
        Err(_) => Err(NetError::new(ErrorKind::TimedOut, format!("the name {host} was not found in time"))),
    }
}

/// A TCP socket that ends every read and write at a deadline, for Detect's probe. rustls reads from it and writes to
/// it in loops of its own (the handshake, and until a whole record has come), so the deadline has to be here, under
/// TLS: before each read and write the socket's timeout is set to the I/O timeout or the time left, whichever is
/// shorter, and once nothing is left the call fails with [`io::ErrorKind::TimedOut`]. Without a deadline it is the
/// socket as it is, its timeouts untouched.
#[derive(Debug)]
pub(crate) struct Timed {
    socket: TcpStream,
    deadline: Option<Deadline>,
    io: Duration,
}

impl Timed {
    pub(crate) fn new(socket: TcpStream, deadline: Option<Deadline>, io: Duration) -> Self {
        Self { socket, deadline, io }
    }

    /// The wait for the next call, set with `set`; nothing without a deadline.
    fn arm(&self, set: fn(&TcpStream, Option<Duration>) -> io::Result<()>) -> io::Result<()> {
        let Some(deadline) = &self.deadline else { return Ok(()) };
        // `wait` is never zero: a deadline with nothing left is an error.
        let wait = deadline.wait(self.io).map_err(|error| io::Error::new(io::ErrorKind::TimedOut, error.to_string()))?;
        set(&self.socket, Some(wait))
    }
}

impl Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.arm(TcpStream::set_read_timeout)?;
        self.socket.read(buf)
    }
}

impl Write for Timed {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.arm(TcpStream::set_write_timeout)?;
        self.socket.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}

impl Stream {
    /// How long a read waits; `None` waits for ever. A probe's deadline still ends it ([`Timed`]).
    pub(crate) fn set_read_timeout(&self, wait: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Plain(socket) => socket.socket.set_read_timeout(wait),
            Self::Tls(stream) => stream.sock.socket.set_read_timeout(wait),
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
    Plain(Timed),
    Tls(Box<StreamOwned<ClientConnection, Timed>>),
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

#[cfg(test)]
#[path = "connect_tests.rs"]
mod tests;
