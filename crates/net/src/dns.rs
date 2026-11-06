// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! DNS for Detect: the MX and SRV records of the owner's mail domain.
//!
//! The messages are `hickory-proto`'s; the exchange is our own, blocking, over `std`: UDP to each name server of the
//! operating system's resolver configuration in turn, and TCP when an answer comes back truncated (RFC 1035 section
//! 4.2, RFC 7766). An answer counts only when its id, its question and its type match the query; anything else on
//! the socket is ignored. Every answer is hostile: records are capped, and each target is checked as a host name.
//!
//! The resolver configuration is `/etc/resolv.conf` (Linux, the BSDs, macOS). Elsewhere there is none yet, and Detect
//! goes on without MX and SRV records.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::BuildHasher;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::num::NonZeroU16;
use std::time::Duration;

use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{Name, RData, RecordType};
use inboxcraft_model::{ErrorKind, Host};
use inboxcraft_time::Timestamp;

use crate::NetError;

/// The most records of one answer kept.
pub const MAX_RECORDS: usize = 16;

/// The most name servers asked (as `resolv.conf` has it).
pub const MAX_SERVERS: usize = 3;

/// The largest UDP answer read; larger ones come over TCP.
const UDP_BUFFER: usize = 4096;

/// How many stray datagrams a query waits through before it gives the server up.
const MAX_STRAYS: usize = 8;

/// The name servers of a `resolv.conf` text: the `nameserver` lines with an IPv4 or IPv6 address, at most
/// [`MAX_SERVERS`], on port 53.
///
/// ```
/// let servers = inboxcraft_net::dns::nameservers("# local\nnameserver 192.0.2.53\nsearch example.org\nnameserver ::1\n");
/// assert_eq!(servers, ["192.0.2.53:53".parse()?, "[::1]:53".parse()?]);
/// # Ok::<(), std::net::AddrParseError>(())
/// ```
#[must_use]
pub fn nameservers(conf: &str) -> Vec<SocketAddr> {
    let mut servers = Vec::new();
    for line in conf.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("nameserver") {
            continue;
        }
        let Some(address) = words.next().and_then(|word| word.parse::<IpAddr>().ok()) else { continue };
        let server = SocketAddr::new(address, 53);
        if !servers.contains(&server) {
            servers.push(server);
        }
        if servers.len() == MAX_SERVERS {
            break;
        }
    }
    servers
}

/// A service a domain names in an SRV record (RFC 2782).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Srv {
    /// The host that offers it.
    pub host: Host,
    /// Its port.
    pub port: NonZeroU16,
    /// Lower is tried first.
    pub priority: u16,
    /// Among the same priority, higher is tried first.
    pub weight: u16,
}

/// The MX hosts of an answer, by preference; a null MX (RFC 7505) and bad names are left out. `None` when the bytes
/// are not a DNS message.
#[must_use]
pub fn read_mx(answer: &[u8]) -> Option<Vec<Host>> {
    Message::from_vec(answer).ok().map(|message| mx_of(&message))
}

/// The SRV records of an answer, by priority and then weight; a target of `.` (the service is not offered) and bad
/// names are left out. `None` when the bytes are not a DNS message.
#[must_use]
pub fn read_srv(answer: &[u8]) -> Option<Vec<Srv>> {
    Message::from_vec(answer).ok().map(|message| srv_of(&message))
}

fn mx_of(message: &Message) -> Vec<Host> {
    let mut found: Vec<(u16, Host)> = message
        .answers
        .iter()
        .filter_map(|record| match &record.data {
            RData::MX(mx) => host(&mx.exchange).map(|host| (mx.preference, host)),
            _ => None,
        })
        .take(MAX_RECORDS)
        .collect();
    found.sort_by_key(|(preference, _)| *preference);
    let mut hosts: Vec<Host> = Vec::new();
    for (_, host) in found {
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    hosts
}

fn srv_of(message: &Message) -> Vec<Srv> {
    let mut found: Vec<Srv> = message
        .answers
        .iter()
        .filter_map(|record| match &record.data {
            RData::SRV(srv) => Some(Srv { host: host(&srv.target)?, port: NonZeroU16::new(srv.port)?, priority: srv.priority, weight: srv.weight }),
            _ => None,
        })
        .take(MAX_RECORDS)
        .collect();
    found.sort_by(|a, b| a.priority.cmp(&b.priority).then(b.weight.cmp(&a.weight)));
    found
}

/// A record's target as a host; the root (`.`) is none.
fn host(name: &Name) -> Option<Host> {
    if name.is_root() {
        return None;
    }
    Host::new(&name.to_ascii()).ok()
}

/// Asks the name servers.
#[derive(Clone)]
pub struct Resolver {
    servers: Vec<SocketAddr>,
    timeout: Duration,
}

impl fmt::Debug for Resolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resolver").field("servers", &self.servers).field("timeout", &self.timeout).finish()
    }
}

impl Resolver {
    /// A resolver asking `servers` in turn, waiting `timeout` for each answer.
    #[must_use]
    pub fn new(servers: Vec<SocketAddr>, timeout: Duration) -> Self {
        Self { servers, timeout }
    }

    /// The operating system's name servers (`/etc/resolv.conf`).
    ///
    /// # Errors
    ///
    /// [`ErrorKind::HostNotFound`] when the configuration cannot be read or names no server.
    pub fn system(timeout: Duration) -> Result<Self, NetError> {
        let conf = std::fs::read_to_string("/etc/resolv.conf").map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string()))?;
        let servers = nameservers(&conf);
        if servers.is_empty() {
            return Err(NetError::new(ErrorKind::HostNotFound, "the resolver configuration names no server"));
        }
        Ok(Self::new(servers, timeout))
    }

    /// The MX hosts of `domain`, by preference; none when the domain has none or does not exist.
    ///
    /// # Errors
    ///
    /// As [`Resolver::ask`].
    pub fn mx(&self, domain: &Host) -> Result<Vec<Host>, NetError> {
        Ok(self.ask(domain.as_str(), RecordType::MX)?.map_or_else(Vec::new, |answer| mx_of(&answer)))
    }

    /// The SRV records of `name` (`_imaps._tcp.example.org`), by priority and weight; none when there are none.
    ///
    /// # Errors
    ///
    /// As [`Resolver::ask`].
    pub fn srv(&self, name: &str) -> Result<Vec<Srv>, NetError> {
        Ok(self.ask(name, RecordType::SRV)?.map_or_else(Vec::new, |answer| srv_of(&answer)))
    }

    /// The answer of the first server that gives one for `name` and `kind`; `None` for a name that does not exist.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::HostNotFound`] for a name that is not one, or without a server; otherwise the last server's
    /// failure: [`ErrorKind::TimedOut`], [`ErrorKind::CommandRefused`] (it refused or failed), or
    /// [`ErrorKind::Protocol`].
    fn ask(&self, name: &str, kind: RecordType) -> Result<Option<Message>, NetError> {
        let mut name = Name::from_utf8(name).map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string()))?;
        // As it comes back in the answer's question.
        name.set_fqdn(true);
        let id = query_id();
        let mut query = Message::new(id, MessageType::Query, OpCode::Query);
        query.metadata.recursion_desired = true;
        query.add_query(Query::query(name.clone(), kind));
        let bytes = query.to_vec().map_err(|error| NetError::new(ErrorKind::Protocol, error.to_string()))?;
        let mut last = NetError::new(ErrorKind::HostNotFound, "no name server");
        for server in &self.servers {
            let answer = self
                .udp(*server, &bytes, id, &name, kind)
                .and_then(|answer| if answer.metadata.truncation { self.tcp(*server, &bytes, id, &name, kind) } else { Ok(answer) });
            match answer.and_then(|answer| outcome(answer, *server)) {
                Ok(found) => return Ok(found),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    fn udp(&self, server: SocketAddr, query: &[u8], id: u16, name: &Name, kind: RecordType) -> Result<Message, NetError> {
        let local: SocketAddr = if server.is_ipv4() { SocketAddr::from(([0, 0, 0, 0], 0)) } else { SocketAddr::from(([0_u16; 8], 0)) };
        let socket = UdpSocket::bind(local).map_err(|error| io(&error))?;
        socket.set_read_timeout(Some(self.timeout)).map_err(|error| io(&error))?;
        socket.connect(server).map_err(|error| io(&error))?;
        socket.send(query).map_err(|error| io(&error))?;
        let mut buffer = vec![0; UDP_BUFFER];
        for _ in 0..MAX_STRAYS {
            let read = socket.recv(&mut buffer).map_err(|error| io(&error))?;
            if let Some(answer) = buffer.get(..read).and_then(|bytes| matching(bytes, id, name, kind)) {
                return Ok(answer);
            }
        }
        Err(NetError::new(ErrorKind::Protocol, format!("no answer of {server} matched the query")))
    }

    fn tcp(&self, server: SocketAddr, query: &[u8], id: u16, name: &Name, kind: RecordType) -> Result<Message, NetError> {
        let mut stream = TcpStream::connect_timeout(&server, self.timeout).map_err(|error| io(&error))?;
        stream.set_read_timeout(Some(self.timeout)).map_err(|error| io(&error))?;
        stream.set_write_timeout(Some(self.timeout)).map_err(|error| io(&error))?;
        let length = u16::try_from(query.len()).map_err(|_| NetError::new(ErrorKind::TooLarge, "the query is too long"))?;
        stream.write_all(&[length.to_be_bytes().as_slice(), query].concat()).map_err(|error| io(&error))?;
        let mut prefix = [0_u8; 2];
        stream.read_exact(&mut prefix).map_err(|error| io(&error))?;
        let mut answer = vec![0; usize::from(u16::from_be_bytes(prefix))];
        stream.read_exact(&mut answer).map_err(|error| io(&error))?;
        matching(&answer, id, name, kind).ok_or_else(|| NetError::new(ErrorKind::Protocol, format!("the answer of {server} did not match the query")))
    }
}

/// `bytes` as the answer to the query `id` for `name` and `kind`, or `None` when it is not.
fn matching(bytes: &[u8], id: u16, name: &Name, kind: RecordType) -> Option<Message> {
    let answer = Message::from_vec(bytes).ok()?;
    let question = answer.queries.first()?;
    let fits =
        answer.metadata.id == id && answer.metadata.message_type == MessageType::Response && question.name() == name && question.query_type() == kind;
    fits.then_some(answer)
}

/// What an answer means: records, a name that does not exist (`None`), or the server's failure.
fn outcome(answer: Message, server: SocketAddr) -> Result<Option<Message>, NetError> {
    match answer.metadata.response_code {
        ResponseCode::NoError => Ok(Some(answer)),
        ResponseCode::NXDomain => Ok(None),
        code => Err(NetError::new(ErrorKind::CommandRefused, format!("{server} answered {code}"))),
    }
}

fn io(error: &std::io::Error) -> NetError {
    NetError::io(error, Timestamp::UNIX_EPOCH)
}

/// A query id nobody off the path can guess: from the process's random hash keys and a counter.
fn query_id() -> u16 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let hash = RandomState::new().hash_one(COUNTER.fetch_add(1, Ordering::Relaxed));
    u16::try_from(hash & 0xffff).unwrap_or_default()
}

#[cfg(test)]
#[path = "dns_tests.rs"]
mod tests;
