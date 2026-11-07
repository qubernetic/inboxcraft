// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A submission (SMTP) connection: the socket driving `proto`'s SMTP client, from the greeting through TLS to the
//! sign-in and the messages.
//!
//! The order is fixed, as for IMAP. Implicit TLS (port 465) encrypts before the first byte; a STARTTLS account (port
//! 587) says EHLO, refuses a server without STARTTLS ([`ErrorKind::StartTlsMissing`]) and runs the handshake before
//! anything else is sent. A password goes out only over an encrypted connection: `proto` refuses it before, and this
//! connection never asks before, so a server that lists AUTH in clear is not taken up on it.
//!
//! EHLO names the client as `[127.0.0.1]`, which tells the server nothing about the computer. A message goes to all
//! of its recipients or to none: a refused recipient ends the transaction with RSET before the message is sent.

use std::fmt;
use std::io::{Read, Write};
use std::sync::Arc;

use inboxcraft_model::{Account, EmailAddress, ErrorKind, Security, Server};
use inboxcraft_proto::ProtoError;
use inboxcraft_proto::smtp::{Answer, Client, Event, MailOptions, Refusal, State, Transport, Verb};
use rustls::ClientConfig;

use crate::connect::{Connector, Stream};
use crate::{Credentials, NetError, Password};

/// The size of one read from the socket.
const READ_CHUNK: usize = 16 * 1024;

/// What EHLO calls the client: an address literal that tells the server nothing about the computer.
const CLIENT_NAME: &str = "[127.0.0.1]";

/// An open submission connection.
pub struct SmtpConnection {
    client: Client,
    stream: Stream,
    buffer: Box<[u8]>,
    connector: Connector,
}

impl fmt::Debug for SmtpConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmtpConnection")
            .field("state", &self.client.state())
            .field("encrypted", &self.client.is_encrypted())
            .field("signed_in", &self.client.is_signed_in())
            .finish_non_exhaustive()
    }
}

impl SmtpConnection {
    /// Connects to `server`, encrypts the connection and says EHLO; not signed in yet.
    ///
    /// # Errors
    ///
    /// A [`NetError`] with the cause: the host, the connection, a certificate fault, a server without STARTTLS on a
    /// STARTTLS account, or a server that breaks the protocol or refuses the session.
    pub fn open(server: &Server, connector: &Connector) -> Result<Self, NetError> {
        let config = connector.tls().client_config()?;
        let stream = connector.tcp(server)?;
        Self::over(stream, server, connector, &config)
    }

    /// As [`SmtpConnection::open`], over the TCP connection `stream` that is open already, encrypted with `config`.
    pub(crate) fn over(mut stream: Stream, server: &Server, connector: &Connector, config: &Arc<ClientConfig>) -> Result<Self, NetError> {
        let transport = match server.security() {
            Security::Tls => {
                stream = connector.encrypt(stream, server.host(), config)?;
                Transport::Tls
            }
            Security::StartTls => Transport::Plain,
        };
        let mut connection =
            Self { client: Client::new(transport), stream, buffer: vec![0; READ_CHUNK].into_boxed_slice(), connector: connector.clone() };
        connection.greeting()?;
        connection.expect(Verb::Ehlo, |client| client.ehlo(CLIENT_NAME))?;
        if server.security() == Security::StartTls {
            if !connection.client.extensions().is_some_and(|extensions| extensions.has("STARTTLS")) {
                return Err(NetError::new(ErrorKind::StartTlsMissing, "the server does not offer STARTTLS"));
            }
            match connection.run(Verb::StartTls, Client::starttls)? {
                Ok(_) => {}
                Err(refusal) => return Err(NetError::new(ErrorKind::StartTlsMissing, format!("STARTTLS refused with {}", refusal.code))),
            }
            let plain = std::mem::replace(&mut connection.stream, Stream::Closed);
            connection.stream = connector.encrypt(plain, server.host(), config)?;
            connection.client.tls_established().map_err(|error| proto(&error))?;
            connection.wait(Verb::Ehlo)?.map_err(|refusal| refused(&refusal))?;
        }
        Ok(connection)
    }

    /// Signs in as `user`: AUTH PLAIN where the server offers it, AUTH LOGIN otherwise.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::LoginRefused`], [`ErrorKind::NoLoginMethod`], a connection error before the password went out,
    /// or [`ErrorKind::LoginUnanswered`] for any failure after it and before the server's answer.
    pub fn login(&mut self, user: &str, password: &Password) -> Result<(), NetError> {
        if self.client.is_signed_in() {
            return Ok(());
        }
        let offers = |mechanism| self.client.extensions().is_some_and(|extensions| extensions.offers_auth(mechanism));
        let started = if offers("PLAIN") {
            self.client.auth_plain(user, password.expose())
        } else if offers("LOGIN") {
            self.client.auth_login(user, password.expose())
        } else {
            return Err(NetError::new(ErrorKind::NoLoginMethod, "the server offers neither AUTH PLAIN nor AUTH LOGIN"));
        };
        started.map_err(|error| proto(&error))?;
        match self.wait(Verb::Auth) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(refusal)) => Err(NetError::new(ErrorKind::LoginRefused, format!("AUTH refused with {}", refusal.code))),
            Err(error) => Err(NetError::unanswered_sign_in(&error, self.client.state() == State::Closed)),
        }
    }

    /// Sends `message` (RFC 5322, as it goes) from `from` to `recipients`, the envelope's alone: a Bcc recipient is
    /// one of them and in no header. The server's words on success (often a queue id), or its refusal of the
    /// sender, a recipient or the message; after a refusal the connection holds and nothing was sent.
    ///
    /// `BODY=8BITMIME` is said for a message that is not ASCII, SMTPUTF8 for an address that is not.
    ///
    /// # Errors
    ///
    /// A connection error, a message the server's SIZE does not take ([`ErrorKind::TooLarge`]), an address or a
    /// message the client cannot send or the server cannot take ([`ErrorKind::CommandRefused`]: nothing went out), or
    /// a server that breaks the protocol.
    pub fn send(&mut self, from: &EmailAddress, recipients: &[EmailAddress], message: &[u8]) -> Result<Result<String, Refusal>, NetError> {
        if recipients.is_empty() {
            return Err(NetError::new(ErrorKind::CommandRefused, "a message without a recipient"));
        }
        let utf8 = !from.as_str().is_ascii() || recipients.iter().any(|to| !to.as_str().is_ascii());
        let size = u64::try_from(message.len()).ok();
        let options = MailOptions { size, eight_bit: !message.is_ascii(), utf8 };
        if let Err(refusal) = self.ask(Verb::Mail, |client| client.mail(from.as_str(), options))? {
            return Ok(Err(refusal));
        }
        for to in recipients {
            if let Err(refusal) = self.ask(Verb::Rcpt, |client| client.rcpt(to.as_str()))? {
                self.reset()?;
                return Ok(Err(refusal));
            }
        }
        Ok(self.ask(Verb::Data, |client| client.data(message))?.map(|answer| match answer {
            Answer::Sent(words) => words,
            _ => String::new(),
        }))
    }

    /// Whether the connection is encrypted.
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.client.is_encrypted()
    }

    /// Sends QUIT, waits for the answer and ends TLS with a `close_notify`; the connection is gone either way.
    pub fn quit(mut self) {
        if self.client.quit().is_ok() {
            let _gone = self.wait(Verb::Quit);
        }
        self.stream.close();
    }

    /// Ends an open transaction; the connection holds.
    fn reset(&mut self) -> Result<(), NetError> {
        self.ask(Verb::Rset, Client::rset)?.map(drop).map_err(|refusal| refused(&refusal))
    }

    /// Starts a command with `start` and waits for its answer.
    fn run(&mut self, verb: Verb, start: impl FnOnce(&mut Client) -> Result<(), ProtoError>) -> Result<Result<Answer, Refusal>, NetError> {
        start(&mut self.client).map_err(|error| proto(&error))?;
        self.wait(verb)
    }

    /// As [`run`](Self::run), but a command the client will not start (an argument it cannot send, an extension the
    /// server lacks) is [`ErrorKind::CommandRefused`]: nothing went out, so the connection holds.
    fn ask(&mut self, verb: Verb, start: impl FnOnce(&mut Client) -> Result<(), ProtoError>) -> Result<Result<Answer, Refusal>, NetError> {
        start(&mut self.client).map_err(|error| match error {
            ProtoError::BadArgument(_) | ProtoError::NotOffered(_) => NetError::new(ErrorKind::CommandRefused, error.to_string()),
            error => proto(&error),
        })?;
        self.wait(verb)
    }

    /// Runs a command that must succeed.
    fn expect(&mut self, verb: Verb, start: impl FnOnce(&mut Client) -> Result<(), ProtoError>) -> Result<(), NetError> {
        self.run(verb, start)?.map(drop).map_err(|refusal| refused(&refusal))
    }

    /// Reads until the greeting.
    fn greeting(&mut self) -> Result<(), NetError> {
        loop {
            if let Some(Event::Greeted) = self.client.next_event() {
                return Ok(());
            }
            self.pump()?;
        }
    }

    /// Writes what the client has for the server and reads until the command `verb` is answered.
    fn wait(&mut self, verb: Verb) -> Result<Result<Answer, Refusal>, NetError> {
        loop {
            self.flush()?;
            while let Some(event) = self.client.next_event() {
                if let Event::Completed { command, outcome } = event
                    && command == verb
                {
                    return Ok(outcome);
                }
            }
            self.pump()?;
        }
    }

    fn flush(&mut self) -> Result<(), NetError> {
        let output = self.client.take_output();
        if output.is_empty() {
            return Ok(());
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
        self.client.receive(bytes).map_err(|error| proto(&error))
    }
}

/// Sends `message` from `account` to `recipients` on a submission connection of its own, signed in with the
/// account's password, asked for first: none means no connection at all. The server's words, or its refusal.
pub(crate) fn submit(
    account: &Account,
    connector: &Connector,
    credentials: &dyn Credentials,
    from: &EmailAddress,
    recipients: &[EmailAddress],
    message: &[u8],
) -> Result<Result<String, Refusal>, NetError> {
    let server = account.submission().ok_or_else(|| NetError::new(ErrorKind::NoSubmissionServer, "no submission server set"))?;
    let password = credentials.password(account).ok_or_else(|| NetError::new(ErrorKind::CredentialMissing, "no password stored"))?;
    let mut connection = SmtpConnection::open(server, connector)?;
    connection.login(account.username(), &password)?;
    drop(password);
    let sent = connection.send(from, recipients, message);
    connection.quit();
    sent
}

/// A refusal as the owner is shown it: the reply code, the enhanced code where the server gave one, and its words.
pub(crate) fn reply(refusal: &Refusal) -> String {
    let mut parts = vec![refusal.code.to_string()];
    parts.extend(refusal.enhanced.map(|code| code.to_string()));
    parts.extend(Some(refusal.text.clone()).filter(|text| !text.is_empty()));
    parts.join(" ")
}

/// A fatal error of the SMTP session, or a command `proto` would not send.
fn proto(error: &ProtoError) -> NetError {
    let kind = match error {
        ProtoError::LineTooLong { .. } | ProtoError::TooManyLines { .. } | ProtoError::TooLarge { .. } => ErrorKind::TooLarge,
        ProtoError::NotOffered("STARTTLS") => ErrorKind::StartTlsMissing,
        ProtoError::NotOffered("AUTH PLAIN" | "AUTH LOGIN") => ErrorKind::NoLoginMethod,
        ProtoError::Bye(_) | ProtoError::Closed => ErrorKind::ConnectionDropped,
        _ => ErrorKind::Protocol,
    };
    NetError::new(kind, error.to_string())
}

/// A refused command that had to succeed.
fn refused(refusal: &Refusal) -> NetError {
    NetError::new(ErrorKind::CommandRefused, format!("refused with {}", refusal.code))
}

#[cfg(test)]
#[path = "smtp_tests.rs"]
mod tests;
