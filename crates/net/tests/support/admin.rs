// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A small IMAP client that changes a mailbox the way another mail program would, for the tests that need the server
//! to change while a worker runs: it creates and deletes the per-run folder and appends invented messages. It is test
//! code: `net` itself never writes to a server in E1.
//!
//! It signs in with LOGIN over implicit TLS only, and reads answers line by line up to its tag.

use std::error::Error;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_testkit::live::Folders;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

/// The longest answer line the client reads.
const MAX_LINE: usize = 64 * 1024;

/// What went wrong, without the server's words when they could echo a secret.
#[derive(Debug)]
pub struct AdminError(String);

impl fmt::Display for AdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for AdminError {}

fn fail(what: impl Into<String>) -> AdminError {
    AdminError(what.into())
}

/// A signed-in admin connection.
pub struct Admin {
    stream: BufReader<StreamOwned<ClientConnection, TcpStream>>,
    next: u32,
}

impl Admin {
    /// Connects to `address` over TLS for `host` with `config` and signs in as `user`.
    pub fn connect(address: SocketAddr, host: &str, config: Arc<ClientConfig>, user: &str, password: &str) -> Result<Self, AdminError> {
        let socket = TcpStream::connect_timeout(&address, Duration::from_secs(30)).map_err(|error| fail(format!("connect: {error}")))?;
        socket.set_read_timeout(Some(Duration::from_secs(60))).map_err(|error| fail(error.to_string()))?;
        let name = ServerName::try_from(host.to_owned()).map_err(|error| fail(error.to_string()))?;
        let connection = ClientConnection::new(config, name).map_err(|error| fail(format!("tls: {error}")))?;
        let mut admin = Self { stream: BufReader::new(StreamOwned::new(connection, socket)), next: 0 };
        let greeting = admin.line()?;
        if !greeting.starts_with("* OK") {
            return Err(fail("the server did not greet with OK"));
        }
        // The answer is checked, never printed: a server may echo what it was sent.
        admin.command(&format!("LOGIN {} {}", quoted(user)?, quoted(password)?)).map_err(|_| fail("LOGIN refused"))?;
        Ok(admin)
    }

    /// Connects to `host` on port 993, looked up by the system.
    pub fn connect_host(host: &str, config: Arc<ClientConfig>, user: &str, password: &str) -> Result<Self, AdminError> {
        let address = (host, 993).to_socket_addrs().map_err(|error| fail(format!("lookup: {error}")))?.next().ok_or_else(|| fail("no address"))?;
        Self::connect(address, host, config, user, password)
    }

    /// Appends `message` (CRLF line ends) to `mailbox`.
    pub fn append(&mut self, mailbox: &str, message: &str) -> Result<(), AdminError> {
        let tag = self.tag();
        self.send(&format!("{tag} APPEND {} {{{}}}", quoted(mailbox)?, message.len()))?;
        let go = self.line()?;
        if !go.starts_with('+') {
            return Err(fail(format!("APPEND refused: {go}")));
        }
        self.send(message)?;
        self.until(&tag).map(drop)
    }

    /// LOGOUT; the connection ends either way.
    pub fn logout(mut self) {
        let _answer = self.command("LOGOUT");
    }

    /// Sends `command` under a new tag and reads up to its answer; the lines before it.
    fn command(&mut self, command: &str) -> Result<Vec<String>, AdminError> {
        let tag = self.tag();
        self.send(&format!("{tag} {command}"))?;
        self.until(&tag)
    }

    fn tag(&mut self) -> String {
        self.next += 1;
        format!("t{}", self.next)
    }

    fn send(&mut self, line: &str) -> Result<(), AdminError> {
        let stream = self.stream.get_mut();
        stream
            .write_all(line.as_bytes())
            .and_then(|()| stream.write_all(b"\r\n"))
            .and_then(|()| stream.flush())
            .map_err(|error| fail(error.to_string()))
    }

    /// Reads up to the answer tagged `tag`: the lines before it when it is OK.
    fn until(&mut self, tag: &str) -> Result<Vec<String>, AdminError> {
        let mut before = Vec::new();
        loop {
            let line = self.line()?;
            if let Some(rest) = line.strip_prefix(tag).and_then(|rest| rest.strip_prefix(' ')) {
                return if rest.starts_with("OK") { Ok(before) } else { Err(fail(format!("{tag} refused: {rest}"))) };
            }
            before.push(line);
        }
    }

    fn line(&mut self) -> Result<String, AdminError> {
        let mut line = Vec::new();
        let read = (&mut self.stream).take(MAX_LINE as u64).read_until(b'\n', &mut line).map_err(|error| fail(error.to_string()))?;
        if read == 0 {
            return Err(fail("the server closed the connection"));
        }
        Ok(String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_owned())
    }
}

impl Folders for Admin {
    type Error = AdminError;

    fn create(&mut self, name: &str) -> Result<(), AdminError> {
        self.command(&format!("CREATE {}", quoted(name)?)).map(drop)
    }

    fn delete(&mut self, name: &str) -> Result<(), AdminError> {
        self.command(&format!("DELETE {}", quoted(name)?)).map(drop)
    }
}

/// `text` as an IMAP quoted string; refused when it holds a line end or NUL, or is not ASCII (a folder name would
/// need modified UTF-7).
fn quoted(text: &str) -> Result<String, AdminError> {
    if !text.is_ascii() || text.contains(['\r', '\n', '\0']) {
        return Err(fail("a value that cannot be quoted"));
    }
    Ok(format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")))
}
