// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An IMAP connection: the socket driving `proto`'s client, from the greeting through TLS to the sign-in.
//!
//! The order is fixed. Implicit TLS encrypts before the first byte; a STARTTLS account reads the capabilities,
//! refuses a server without STARTTLS ([`ErrorKind::StartTlsMissing`]) and runs the handshake before anything else
//! is sent. A password goes out only over an encrypted connection: `proto` refuses it before, and this connection
//! never asks before. A server that offers a sign-in in clear is not taken up on it.

use std::fmt;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::{ErrorKind, Security, Server};
use inboxcraft_proto::ProtoError;
use inboxcraft_proto::imap::{Answer, Capabilities, Client, Event, Extension, Refusal, State, Tag, Transport};
use inboxcraft_time::Clock;
use rustls::ClientConfig;

use crate::NetError;
use crate::compress::Deflate;
use crate::connect::{Connector, Stream};

/// The size of one read from the socket.
const READ_CHUNK: usize = 64 * 1024;

/// A password, in memory only: it has no `Display`, its `Debug` hides it, and its bytes are overwritten when it is
/// dropped.
pub struct Password(String);

impl Password {
    /// The password `text`.
    #[must_use]
    pub fn new(text: String) -> Self {
        Self(text)
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(hidden)")
    }
}

impl Drop for Password {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
        std::hint::black_box(&bytes);
    }
}

/// How a command ended: its answer, or the server's refusal.
pub(crate) type Outcome = Result<Answer, Refusal>;

/// An open IMAP connection.
pub struct ImapConnection {
    client: Client,
    stream: Stream,
    buffer: Box<[u8]>,
    connector: Connector,
    preauth: bool,
    /// The COMPRESS=DEFLATE state, once the server agreed.
    deflate: Option<Deflate>,
}

impl fmt::Debug for ImapConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImapConnection")
            .field("state", &self.client.state())
            .field("encrypted", &self.client.is_encrypted())
            .field("compressed", &self.deflate.is_some())
            .finish_non_exhaustive()
    }
}

impl ImapConnection {
    /// Connects to `server`, encrypts the connection and reads the greeting and the capabilities; not signed in yet.
    ///
    /// # Errors
    ///
    /// A [`NetError`] with the cause: the host, the connection, a certificate fault, a server without STARTTLS on a
    /// STARTTLS account, or a server that breaks the protocol.
    pub fn open(server: &Server, connector: &Connector) -> Result<Self, NetError> {
        let config = connector.tls().client_config()?;
        let stream = connector.tcp(server)?;
        Self::over(stream, server, connector, &config)
    }

    /// As [`ImapConnection::open`], over the TCP connection `stream` that is open already, encrypted with `config`.
    pub(crate) fn over(mut stream: Stream, server: &Server, connector: &Connector, config: &Arc<ClientConfig>) -> Result<Self, NetError> {
        let transport = match server.security() {
            Security::Tls => {
                stream = connector.encrypt(stream, server.host(), config)?;
                Transport::Tls
            }
            Security::StartTls => Transport::Plain,
        };
        let mut connection = Self {
            client: Client::new(transport),
            stream,
            buffer: vec![0; READ_CHUNK].into_boxed_slice(),
            connector: connector.clone(),
            preauth: false,
            deflate: None,
        };
        connection.preauth = connection.greeting()?;
        if server.security() == Security::StartTls {
            connection.capabilities()?;
            if !connection.client.capabilities().is_some_and(|caps| caps.has("STARTTLS")) {
                return Err(NetError::new(ErrorKind::StartTlsMissing, "the server does not offer STARTTLS"));
            }
            match connection.run(Client::starttls, &mut |_| {})? {
                Ok(Answer::StartTls) => {}
                Ok(_) => return Err(NetError::new(ErrorKind::Protocol, "an answer of another command")),
                Err(refusal) => return Err(NetError::new(ErrorKind::StartTlsMissing, refusal.code.unwrap_or_default())),
            }
            let plain = std::mem::replace(&mut connection.stream, Stream::Closed);
            connection.stream = connector.encrypt(plain, server.host(), config)?;
            let tag = connection.client.tls_established().map_err(|error| NetError::proto(&error))?;
            connection.wait(&tag, &mut |_| {})?.map_err(|refusal| refused(&refusal))?;
        }
        connection.capabilities()?;
        Ok(connection)
    }

    /// Signs in as `user`: AUTHENTICATE PLAIN where the server offers it, LOGIN otherwise. Nothing is sent when the
    /// greeting was PREAUTH.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::LoginRefused`], [`ErrorKind::NoLoginMethod`], a connection error before the password went out,
    /// or [`ErrorKind::LoginUnanswered`] for any failure after it and before the server's answer.
    pub fn login(&mut self, user: &str, password: &Password) -> Result<(), NetError> {
        if self.preauth || self.client.state() != State::NotAuthenticated {
            return Ok(());
        }
        let plain = self.client.capabilities().is_some_and(|caps| caps.has("AUTH=PLAIN"));
        let tag = if plain { self.client.authenticate_plain(user, password.expose()) } else { self.client.login(user, password.expose()) }
            .map_err(|error| NetError::proto(&error))?;
        match self.wait(&tag, &mut |_| {}) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(refusal)) => Err(NetError::new(ErrorKind::LoginRefused, refusal.code.unwrap_or_default())),
            Err(error) => Err(NetError::unanswered_sign_in(&error, self.client.state() == State::Closed)),
        }
    }

    /// Switches QRESYNC on (ENABLE, RFC 7162 section 3.2.3) when the server offers it, after the sign-in; whether it
    /// is on. With it, a folder opened again reports what changed and what went since the state the last sync left
    /// (VANISHED). A server that refuses leaves it off.
    ///
    /// # Errors
    ///
    /// A connection error, or a server that breaks the protocol.
    pub fn enable_qresync(&mut self) -> Result<bool, NetError> {
        let offered = self.client.capabilities().is_some_and(|caps| caps.has("ENABLE") && caps.has("QRESYNC"));
        if offered && !self.qresync() && self.client.state() == State::Authenticated {
            match self.run(|client| client.enable(Extension::QResync), &mut |_| {})? {
                Ok(Answer::Enabled(_)) | Err(_) => {}
                Ok(_) => return Err(other_answer()),
            }
        }
        Ok(self.qresync())
    }

    /// Whether QRESYNC is on.
    #[must_use]
    pub fn qresync(&self) -> bool {
        self.client.is_enabled(Extension::QResync)
    }

    /// Compresses the connection (COMPRESS=DEFLATE, RFC 4978) when the server offers it, after the sign-in; whether it
    /// is compressed. A server that refuses leaves it as it was. Nothing secret is sent after it: the password went
    /// before, so a compressed length cannot tell of it.
    ///
    /// # Errors
    ///
    /// A connection error, a compressed stream that is broken, or a server that breaks the protocol.
    pub fn compress(&mut self) -> Result<bool, NetError> {
        let offered = self.client.capabilities().is_some_and(|caps| caps.has("COMPRESS=DEFLATE"));
        if self.deflate.is_some() || !offered || self.client.state() == State::NotAuthenticated {
            return Ok(self.deflate.is_some());
        }
        match self.run(Client::compress, &mut |_| {})? {
            Ok(Answer::Compressed(rest)) => {
                let mut deflate = Deflate::new();
                let client = &mut self.client;
                deflate.inflate(&rest, &mut |piece| client.receive(piece).map_err(|error| NetError::proto(&error)))?;
                self.deflate = Some(deflate);
                Ok(true)
            }
            Ok(_) => Err(other_answer()),
            Err(_) => Ok(false),
        }
    }

    /// Whether the connection is compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.deflate.is_some()
    }

    /// What the server can do, once read.
    #[must_use]
    pub fn capabilities_known(&self) -> Option<&Capabilities> {
        self.client.capabilities()
    }

    /// Whether the connection is encrypted.
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.client.is_encrypted()
    }

    /// The session's state.
    #[must_use]
    pub fn state(&self) -> State {
        self.client.state()
    }

    /// Sends LOGOUT, waits for the answer and ends TLS with a `close_notify`; the connection is gone either way.
    pub fn logout(mut self) {
        if let Ok(tag) = self.client.logout() {
            let _gone = self.wait(&tag, &mut |_| {});
        }
        self.stream.close();
    }

    /// Whether the server offers IDLE.
    pub(crate) fn offers_idle(&self) -> bool {
        self.client.capabilities().is_some_and(|caps| caps.has("IDLE"))
    }

    /// The clock of the connection's TLS settings.
    pub(crate) fn clock(&self) -> &Arc<dyn Clock> {
        self.connector.tls().clock()
    }

    /// Starts a command with `start` and sends it, without waiting for its answer.
    pub(crate) fn start(&mut self, start: impl FnOnce(&mut Client) -> Result<Tag, ProtoError>) -> Result<Tag, NetError> {
        let tag = start(&mut self.client).map_err(|error| NetError::proto(&error))?;
        self.flush()?;
        Ok(tag)
    }

    /// Sends DONE to end the IDLE in flight; its answer comes with [`wait`](Self::wait).
    pub(crate) fn done(&mut self) -> Result<(), NetError> {
        self.client.done().map_err(|error| NetError::proto(&error))?;
        self.flush()
    }

    /// Hands the events waiting to `on_event`, then reads what the server sends within `wait` once and hands those on
    /// too. Nothing to read in time is no error.
    pub(crate) fn listen(&mut self, wait: Duration, on_event: &mut dyn FnMut(Event)) -> Result<(), NetError> {
        while let Some(event) = self.client.next_event() {
            on_event(event);
        }
        let now = self.clock().now();
        // A zero wait would mean no timeout at all.
        self.stream.set_read_timeout(Some(wait.max(Duration::from_millis(1)))).map_err(|error| NetError::io(&error, now))?;
        let read = self.pump();
        self.stream.set_read_timeout(Some(self.connector.timeouts().io)).map_err(|error| NetError::io(&error, now))?;
        match read {
            Err(error) if error.kind() == ErrorKind::TimedOut => {}
            other => other?,
        }
        while let Some(event) = self.client.next_event() {
            on_event(event);
        }
        Ok(())
    }

    /// Starts a command with `start` and waits for its answer, handing every other event to `on_event`.
    pub(crate) fn run(
        &mut self,
        start: impl FnOnce(&mut Client) -> Result<Tag, ProtoError>,
        on_event: &mut dyn FnMut(Event),
    ) -> Result<Outcome, NetError> {
        let tag = start(&mut self.client).map_err(|error| NetError::proto(&error))?;
        self.wait(&tag, on_event)
    }

    /// As [`run`](Self::run), but a command the client will not start (an argument it cannot send, a folder open
    /// read-only, an extension the server lacks) is [`ErrorKind::CommandRefused`]: nothing went out, so the connection
    /// holds.
    pub(crate) fn ask(
        &mut self,
        start: impl FnOnce(&mut Client) -> Result<Tag, ProtoError>,
        on_event: &mut dyn FnMut(Event),
    ) -> Result<Outcome, NetError> {
        let tag = start(&mut self.client).map_err(|error| match error {
            ProtoError::BadArgument(_) | ProtoError::ReadOnly | ProtoError::NotOffered(_) | ProtoError::NotEnabled(_) => {
                NetError::new(ErrorKind::CommandRefused, error.to_string())
            }
            error => NetError::proto(&error),
        })?;
        self.wait(&tag, on_event)
    }

    /// The capabilities, asked for when the server has not listed them.
    fn capabilities(&mut self) -> Result<(), NetError> {
        if self.client.capabilities().is_none() {
            self.expect(Client::capability, |answer| matches!(answer, Answer::Capabilities(_)))?;
        }
        Ok(())
    }

    /// Runs a command that must succeed with an answer `fits` takes.
    fn expect(&mut self, start: impl FnOnce(&mut Client) -> Result<Tag, ProtoError>, fits: impl Fn(&Answer) -> bool) -> Result<(), NetError> {
        match self.run(start, &mut |_| {})? {
            Ok(answer) if fits(&answer) => Ok(()),
            Ok(_) => Err(other_answer()),
            Err(refusal) => Err(refused(&refusal)),
        }
    }

    /// Reads until the greeting; whether it was PREAUTH.
    fn greeting(&mut self) -> Result<bool, NetError> {
        loop {
            while let Some(event) = self.client.next_event() {
                if let Event::Greeted { preauth } = event {
                    return Ok(preauth);
                }
            }
            self.pump()?;
        }
    }

    /// Writes what the client has for the server and reads until the command `tag` is answered.
    pub(crate) fn wait(&mut self, tag: &Tag, on_event: &mut dyn FnMut(Event)) -> Result<Outcome, NetError> {
        loop {
            self.flush()?;
            while let Some(event) = self.client.next_event() {
                match event {
                    Event::Completed { tag: done, outcome } if done == *tag => return Ok(outcome),
                    other => on_event(other),
                }
            }
            self.pump()?;
        }
    }

    fn flush(&mut self) -> Result<(), NetError> {
        let mut output = self.client.take_output();
        if output.is_empty() {
            return Ok(());
        }
        if let Some(deflate) = self.deflate.as_mut() {
            output = deflate.compress(&output)?;
        }
        let now = self.connector.tls().clock().now();
        self.stream.write_all(&output).and_then(|()| self.stream.flush()).map_err(|error| NetError::io(&error, now))
    }

    /// One read from the socket into the client.
    fn pump(&mut self) -> Result<(), NetError> {
        let now = self.connector.tls().clock().now();
        let read = self.stream.read(&mut self.buffer).map_err(|error| NetError::io(&error, now))?;
        if read == 0 {
            return Err(NetError::new(ErrorKind::ConnectionDropped, "the server closed the connection"));
        }
        let bytes = self.buffer.get(..read).unwrap_or_default();
        let client = &mut self.client;
        match self.deflate.as_mut() {
            Some(deflate) => deflate.inflate(bytes, &mut |piece| client.receive(piece).map_err(|error| NetError::proto(&error))),
            None => client.receive(bytes).map_err(|error| NetError::proto(&error)),
        }
    }
}

fn other_answer() -> NetError {
    NetError::new(ErrorKind::Protocol, "an answer of another command")
}

/// A refused command that had to succeed.
fn refused(refusal: &Refusal) -> NetError {
    NetError::new(ErrorKind::CommandRefused, refusal.code.clone().unwrap_or_default())
}
