// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The client state machine (RFC 5321 section 4.1, STARTTLS RFC 3207, AUTH RFC 4954): bytes in, bytes out, events.

use std::collections::VecDeque;
use std::fmt;

use super::data::{check_domain, check_path, dot_stuffed};
use super::ehlo::Extensions;
use super::enhanced::EnhancedCode;
use super::reply::{Reader, Reply};
use crate::ProtoError;
use crate::imap::sasl::{base64, base64_forms, check_credentials, plain_response};
use crate::imap::text::Secrets;

/// How the connection started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// Implicit TLS (port 465, RFC 8314): encrypted from the first byte.
    Tls,
    /// Not encrypted (port 587): STARTTLS must come before any AUTH or MAIL.
    Plain,
}

/// The session's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Waiting for the server's greeting.
    Greeting,
    /// Greeted (or encrypted after STARTTLS), waiting for EHLO.
    Connected,
    /// EHLO answered: STARTTLS, AUTH and MAIL may follow.
    Ready,
    /// The server agreed to STARTTLS: the caller runs the TLS handshake, then calls [`Client::tls_established`].
    StartingTls,
    /// A mail transaction is open: MAIL was accepted, RCPT and DATA may follow.
    Mail,
    /// QUIT was sent.
    Quitting,
    /// The session is over.
    Closed,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Greeting => "greeting",
            Self::Connected => "connected",
            Self::Ready => "ready",
            Self::StartingTls => "starting TLS",
            Self::Mail => "mail transaction",
            Self::Quitting => "quitting",
            Self::Closed => "closed",
        })
    }
}

/// The command a reply answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    /// EHLO.
    Ehlo,
    /// STARTTLS.
    StartTls,
    /// AUTH PLAIN or AUTH LOGIN.
    Auth,
    /// MAIL FROM.
    Mail,
    /// RCPT TO.
    Rcpt,
    /// DATA and the message after it.
    Data,
    /// RSET.
    Rset,
    /// QUIT.
    Quit,
}

impl fmt::Display for Verb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ehlo => "EHLO",
            Self::StartTls => "STARTTLS",
            Self::Auth => "AUTH",
            Self::Mail => "MAIL",
            Self::Rcpt => "RCPT",
            Self::Data => "DATA",
            Self::Rset => "RSET",
            Self::Quit => "QUIT",
        })
    }
}

/// What a command brought when the server took it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// EHLO: what the server can do.
    Hello(Extensions),
    /// STARTTLS: run the TLS handshake now.
    StartTls,
    /// AUTH: signed in.
    SignedIn,
    /// MAIL: the transaction is open.
    MailAccepted,
    /// RCPT: the server takes the message for this recipient.
    RecipientAccepted,
    /// The message: the server took it; its words (often a queue id), cleaned.
    Sent(String),
    /// RSET: no transaction is open.
    Reset,
    /// QUIT: the session is over.
    Quit,
}

/// The server's refusal of a command: 4yz (for now) or 5yz (for good).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The three-digit reply code.
    pub code: u16,
    /// The enhanced status code (RFC 3463), when the server offers ENHANCEDSTATUSCODES and sent one.
    pub enhanced: Option<EnhancedCode>,
    /// The server's words, cleaned and capped, with the password hidden; empty before TLS.
    pub text: String,
}

impl Refusal {
    /// Whether the refusal is for good (5yz): the same command will not work later.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        self.code >= 500
    }
}

/// What the server answered, in the order it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The server greeted (220).
    Greeted,
    /// A command is done.
    Completed {
        /// The command.
        command: Verb,
        /// What it brought, or the server's refusal.
        outcome: Result<Answer, Refusal>,
    },
}

/// What MAIL FROM says of the message to come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MailOptions {
    /// Its size in bytes: told to a server that offers SIZE (RFC 1870), and checked against its limit.
    pub size: Option<u64>,
    /// It holds 8-bit text: `BODY=8BITMIME` (RFC 6152).
    pub eight_bit: bool,
    /// It has UTF-8 addresses or header fields: `SMTPUTF8` (RFC 6531); taken as set when the sender is not ASCII.
    pub utf8: bool,
}

/// The command waiting for its answer.
struct InFlight {
    verb: Verb,
    /// What goes after each go-ahead (3yz): the AUTH LOGIN steps, the message after DATA.
    waiting: VecDeque<Vec<u8>>,
}

/// An SMTP submission client without I/O.
///
/// Feed it what the server sent with [`receive`](Self::receive), write what [`take_output`](Self::take_output) gives
/// to the server, and read what happened from [`next_event`](Self::next_event). One command at a time, no
/// PIPELINING.
///
/// AUTH and MAIL are refused with [`ProtoError::NotEncrypted`] until the connection is encrypted, and EHLO is sent
/// again after STARTTLS. Everything the server sends is hostile: lines and replies are capped, and a fatal
/// [`ProtoError`] closes the session.
pub struct Client {
    state: State,
    tls: bool,
    reader: Reader,
    output: Vec<u8>,
    events: VecDeque<Event>,
    /// What EHLO sent, to send it again after STARTTLS.
    domain: String,
    extensions: Option<Extensions>,
    signed_in: bool,
    in_flight: Option<InFlight>,
    secrets: Secrets,
    /// The open transaction's options, as sent.
    transaction: MailOptions,
    /// Recipients the server took in the open transaction.
    recipients: usize,
}

/// Neither the output (it may hold a password or a message) nor anything secret is shown.
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("state", &self.state)
            .field("tls", &self.tls)
            .field("signed_in", &self.signed_in)
            .field("in_flight", &self.in_flight.as_ref().map(|flight| flight.verb))
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client for a connection that has just opened, waiting for the greeting.
    #[must_use]
    pub fn new(transport: Transport) -> Self {
        Self {
            state: State::Greeting,
            tls: transport == Transport::Tls,
            reader: Reader::default(),
            output: Vec::new(),
            events: VecDeque::new(),
            domain: String::new(),
            extensions: None,
            signed_in: false,
            in_flight: None,
            secrets: Secrets::default(),
            transaction: MailOptions::default(),
            recipients: 0,
        }
    }

    /// The session's state.
    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// Whether the connection is encrypted.
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.tls
    }

    /// Whether AUTH succeeded.
    #[must_use]
    pub fn is_signed_in(&self) -> bool {
        self.signed_in
    }

    /// What the server said it can do, `None` until EHLO is answered (and again after STARTTLS, until then).
    #[must_use]
    pub fn extensions(&self) -> Option<&Extensions> {
        self.extensions.as_ref()
    }

    /// The bytes to write to the server, taken out.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }

    /// The next event, taken out.
    pub fn next_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Takes in bytes the server sent, in pieces of any size.
    ///
    /// # Errors
    ///
    /// A fatal [`ProtoError`] when the server broke the protocol or a limit, or ended the session (421); the session
    /// is then [`Closed`](State::Closed) and every later call answers [`ProtoError::Closed`].
    pub fn receive(&mut self, bytes: &[u8]) -> Result<(), ProtoError> {
        match self.state {
            State::Closed => return Err(ProtoError::Closed),
            State::StartingTls if !bytes.is_empty() => return Err(self.fail(ProtoError::PlaintextAfterStartTls)),
            _ => {}
        }
        self.reader.push(bytes);
        while !matches!(self.state, State::StartingTls | State::Closed) {
            let reply = match self.reader.next() {
                Ok(Some(reply)) => reply,
                Ok(None) => break,
                Err(error) => return Err(self.fail(error)),
            };
            if let Err(error) = self.handle(&reply) {
                return Err(self.fail(error));
            }
        }
        Ok(())
    }

    /// EHLO with the client's domain or address literal (`[192.0.2.1]`), after the greeting.
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for a domain that is not letters, digits, `-`, `.` or an address literal, or a
    /// state error.
    pub fn ehlo(&mut self, domain: &str) -> Result<(), ProtoError> {
        self.ready("EHLO", &[State::Connected])?;
        check_domain(domain)?;
        domain.clone_into(&mut self.domain);
        self.start(Verb::Ehlo, format!("EHLO {domain}").into_bytes(), VecDeque::new());
        Ok(())
    }

    /// STARTTLS (RFC 3207), on a connection that is not encrypted and whose server offers it.
    ///
    /// # Errors
    ///
    /// [`ProtoError::AlreadyEncrypted`], [`ProtoError::NotOffered`], or a state error.
    pub fn starttls(&mut self) -> Result<(), ProtoError> {
        self.ready("STARTTLS", &[State::Ready])?;
        if self.tls {
            return Err(ProtoError::AlreadyEncrypted);
        }
        if !self.offers("STARTTLS") {
            return Err(ProtoError::NotOffered("STARTTLS"));
        }
        self.start(Verb::StartTls, b"STARTTLS".to_vec(), VecDeque::new());
        Ok(())
    }

    /// After the TLS handshake that followed STARTTLS: the session goes on encrypted, forgets what the server listed
    /// before, and sends EHLO again.
    ///
    /// # Errors
    ///
    /// [`ProtoError::WrongSmtpState`] unless STARTTLS was just agreed, or [`ProtoError::Closed`].
    pub fn tls_established(&mut self) -> Result<(), ProtoError> {
        match self.state {
            State::StartingTls => {}
            State::Closed => return Err(ProtoError::Closed),
            state => return Err(ProtoError::WrongSmtpState { command: "STARTTLS", state }),
        }
        self.tls = true;
        self.state = State::Connected;
        self.extensions = None;
        self.reader = Reader::default();
        let line = format!("EHLO {}", self.domain).into_bytes();
        self.start(Verb::Ehlo, line, VecDeque::new());
        Ok(())
    }

    /// AUTH PLAIN (RFC 4616) with the response on the command line, over an encrypted connection to a server that
    /// offers it.
    ///
    /// # Errors
    ///
    /// [`ProtoError::NotEncrypted`] before TLS, [`ProtoError::AlreadySignedIn`], [`ProtoError::NotOffered`],
    /// [`ProtoError::BadArgument`] for an empty user name or password or one with a NUL, or a state error.
    pub fn auth_plain(&mut self, user: &str, password: &str) -> Result<(), ProtoError> {
        self.sign_in_ready("PLAIN")?;
        let response = plain_response(user, password)?;
        self.hide_credentials(user, password)?;
        self.start(Verb::Auth, format!("AUTH PLAIN {response}").into_bytes(), VecDeque::new());
        Ok(())
    }

    /// AUTH LOGIN: the user name, then the password, each in base64 after a go-ahead; for servers without PLAIN.
    ///
    /// # Errors
    ///
    /// As [`auth_plain`](Self::auth_plain).
    pub fn auth_login(&mut self, user: &str, password: &str) -> Result<(), ProtoError> {
        self.sign_in_ready("LOGIN")?;
        self.hide_credentials(user, password)?;
        let steps = [user, password].into_iter().map(|step| format!("{}\r\n", base64(step.as_bytes())).into_bytes()).collect();
        self.start(Verb::Auth, b"AUTH LOGIN".to_vec(), steps);
        Ok(())
    }

    /// MAIL FROM, over an encrypted connection: opens a transaction. An empty `from` is the null reverse-path.
    ///
    /// # Errors
    ///
    /// [`ProtoError::NotEncrypted`] before TLS, [`ProtoError::NotOffered`] for 8-bit or UTF-8 the server does not
    /// take, [`ProtoError::TooLarge`] over the server's SIZE, [`ProtoError::BadArgument`] for an address that cannot
    /// be sent, or a state error.
    pub fn mail(&mut self, from: &str, options: MailOptions) -> Result<(), ProtoError> {
        self.encrypted()?;
        self.ready("MAIL", &[State::Ready])?;
        let utf8 = options.utf8 || !from.is_ascii();
        check_path(from, utf8)?;
        if utf8 && !self.offers("SMTPUTF8") {
            return Err(ProtoError::NotOffered("SMTPUTF8"));
        }
        if options.eight_bit && !self.offers("8BITMIME") {
            return Err(ProtoError::NotOffered("8BITMIME"));
        }
        let mut line = format!("MAIL FROM:<{from}>");
        if let Some(size) = options.size.filter(|_| self.offers("SIZE")) {
            self.check_size(size)?;
            line.push_str(&format!(" SIZE={size}"));
        }
        if options.eight_bit {
            line.push_str(" BODY=8BITMIME");
        }
        if utf8 {
            line.push_str(" SMTPUTF8");
        }
        self.transaction = MailOptions { utf8, ..options };
        self.start(Verb::Mail, line.into_bytes(), VecDeque::new());
        Ok(())
    }

    /// RCPT TO, in an open transaction.
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for an empty address, one that cannot be sent, or one that is not ASCII in a
    /// transaction without SMTPUTF8; or a state error.
    pub fn rcpt(&mut self, to: &str) -> Result<(), ProtoError> {
        self.ready("RCPT", &[State::Mail])?;
        if to.is_empty() {
            return Err(ProtoError::BadArgument("empty address"));
        }
        check_path(to, self.transaction.utf8)?;
        self.start(Verb::Rcpt, format!("RCPT TO:<{to}>").into_bytes(), VecDeque::new());
        Ok(())
    }

    /// DATA, then `message` (header and body, RFC 5322) after the server's go-ahead, with its lines ended in CRLF
    /// and dot-stuffed. Once the server answered the message, the transaction is over.
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] before a recipient was accepted, for a message with a NUL, or with 8-bit bytes in
    /// a transaction without `BODY=8BITMIME` or `SMTPUTF8`; [`ProtoError::TooLarge`] over the server's SIZE; or a
    /// state error.
    pub fn data(&mut self, message: &[u8]) -> Result<(), ProtoError> {
        self.ready("DATA", &[State::Mail])?;
        if self.recipients == 0 {
            return Err(ProtoError::BadArgument("message without an accepted recipient"));
        }
        if message.contains(&0) {
            return Err(ProtoError::BadArgument("message with a NUL"));
        }
        if !message.is_ascii() && !self.transaction.eight_bit && !self.transaction.utf8 {
            return Err(ProtoError::BadArgument("8-bit message without BODY=8BITMIME"));
        }
        self.check_size(u64::try_from(message.len()).unwrap_or(u64::MAX))?;
        self.start(Verb::Data, b"DATA".to_vec(), VecDeque::from([dot_stuffed(message)]));
        Ok(())
    }

    /// RSET: ends the open transaction, if any.
    ///
    /// # Errors
    ///
    /// A state error.
    pub fn rset(&mut self) -> Result<(), ProtoError> {
        self.ready("RSET", &[State::Ready, State::Mail])?;
        self.start(Verb::Rset, b"RSET".to_vec(), VecDeque::new());
        Ok(())
    }

    /// QUIT: whatever the server answers, the session is over after it.
    ///
    /// # Errors
    ///
    /// A state error.
    pub fn quit(&mut self) -> Result<(), ProtoError> {
        self.ready("QUIT", &[State::Connected, State::Ready, State::Mail])?;
        self.start(Verb::Quit, b"QUIT".to_vec(), VecDeque::new());
        self.state = State::Quitting;
        Ok(())
    }

    /// Whether a command may start in the session's state.
    fn ready(&self, command: &'static str, states: &[State]) -> Result<(), ProtoError> {
        if self.state == State::Closed {
            return Err(ProtoError::Closed);
        }
        if self.in_flight.is_some() {
            return Err(ProtoError::Busy);
        }
        if !states.contains(&self.state) {
            return Err(ProtoError::WrongSmtpState { command, state: self.state });
        }
        Ok(())
    }

    /// No password and no message over a connection that is not encrypted, whatever the state.
    fn encrypted(&self) -> Result<(), ProtoError> {
        match self.state {
            State::Closed => Err(ProtoError::Closed),
            _ if !self.tls => Err(ProtoError::NotEncrypted),
            _ => Ok(()),
        }
    }

    /// Whether AUTH with `mechanism` may start.
    fn sign_in_ready(&self, mechanism: &'static str) -> Result<(), ProtoError> {
        self.encrypted()?;
        self.ready("AUTH", &[State::Ready])?;
        if self.signed_in {
            return Err(ProtoError::AlreadySignedIn);
        }
        if !self.extensions.as_ref().is_some_and(|extensions| extensions.offers_auth(mechanism)) {
            return Err(ProtoError::NotOffered(if mechanism == "PLAIN" { "AUTH PLAIN" } else { "AUTH LOGIN" }));
        }
        Ok(())
    }

    /// Checks the credentials and adds the password to what is never shown, as sent and in every base64 form a
    /// server may write it back in.
    fn hide_credentials(&mut self, user: &str, password: &str) -> Result<(), ProtoError> {
        check_credentials(user, password)?;
        self.secrets.add(password);
        for form in base64_forms(user, password)? {
            self.secrets.add(&form);
        }
        Ok(())
    }

    fn offers(&self, keyword: &str) -> bool {
        self.extensions.as_ref().is_some_and(|extensions| extensions.has(keyword))
    }

    fn check_size(&self, size: u64) -> Result<(), ProtoError> {
        match self.extensions.as_ref().and_then(Extensions::size_limit) {
            Some(limit) if size > limit => Err(ProtoError::TooLarge { size, limit }),
            _ => Ok(()),
        }
    }

    fn start(&mut self, verb: Verb, line: Vec<u8>, waiting: VecDeque<Vec<u8>>) {
        self.output.extend_from_slice(&line);
        self.output.extend_from_slice(b"\r\n");
        self.in_flight = Some(InFlight { verb, waiting });
    }

    /// Ends the session after a fatal error: nothing more is sent or read.
    fn fail(&mut self, error: ProtoError) -> ProtoError {
        self.state = State::Closed;
        self.in_flight = None;
        self.output.clear();
        error
    }

    fn handle(&mut self, reply: &Reply) -> Result<(), ProtoError> {
        if self.state == State::Greeting {
            return self.greeting(reply);
        }
        let Some(mut flight) = self.in_flight.take() else {
            return Err(if reply.code == 421 { ProtoError::Bye(self.plain_words(reply)) } else { ProtoError::Unexpected("reply") });
        };
        if flight.verb == Verb::Quit {
            self.state = State::Closed;
            if reply.is_intermediate() {
                return Err(ProtoError::Unexpected("go-ahead"));
            }
            let outcome = if reply.is_positive() { Ok(Answer::Quit) } else { Err(self.refusal(reply)) };
            self.events.push_back(Event::Completed { command: Verb::Quit, outcome });
            return Ok(());
        }
        if reply.code == 421 {
            return Err(ProtoError::Bye(self.plain_words(reply)));
        }
        if reply.is_intermediate() {
            let next = match flight.verb {
                Verb::Data if reply.code == 354 => flight.waiting.pop_front(),
                Verb::Auth => flight.waiting.pop_front(),
                _ => None,
            };
            let Some(next) = next else { return Err(ProtoError::Unexpected("go-ahead")) };
            self.output.extend_from_slice(&next);
            self.in_flight = Some(flight);
            return Ok(());
        }
        let message_sent = flight.verb == Verb::Data && flight.waiting.is_empty();
        if flight.verb == Verb::Data && !message_sent && reply.is_positive() {
            return Err(ProtoError::Unexpected("reply to DATA"));
        }
        let outcome = if reply.is_positive() {
            Ok(self.done(flight.verb, reply)?)
        } else {
            if message_sent {
                self.end_transaction();
            }
            Err(self.refusal(reply))
        };
        self.events.push_back(Event::Completed { command: flight.verb, outcome });
        Ok(())
    }

    fn greeting(&mut self, reply: &Reply) -> Result<(), ProtoError> {
        match reply.code {
            220 => {
                self.state = State::Connected;
                self.events.push_back(Event::Greeted);
                Ok(())
            }
            _ if reply.is_transient() || reply.is_permanent() => Err(ProtoError::Bye(self.plain_words(reply))),
            _ => Err(ProtoError::Malformed("greeting")),
        }
    }

    /// What a positive reply to `verb` changes, and the answer for the caller.
    fn done(&mut self, verb: Verb, reply: &Reply) -> Result<Answer, ProtoError> {
        Ok(match verb {
            Verb::Ehlo => {
                let extensions = Extensions::from_lines(reply.lines.iter().skip(1).map(String::as_str));
                self.extensions = Some(extensions.clone());
                self.state = State::Ready;
                Answer::Hello(extensions)
            }
            Verb::StartTls => {
                // Bytes after the go-ahead came before encryption: anyone on the way could have put them there.
                if self.reader.has_pending() {
                    return Err(ProtoError::PlaintextAfterStartTls);
                }
                self.state = State::StartingTls;
                Answer::StartTls
            }
            Verb::Auth => {
                self.signed_in = true;
                Answer::SignedIn
            }
            Verb::Mail => {
                self.state = State::Mail;
                self.recipients = 0;
                Answer::MailAccepted
            }
            Verb::Rcpt => {
                self.recipients = self.recipients.saturating_add(1);
                Answer::RecipientAccepted
            }
            Verb::Data => {
                self.end_transaction();
                Answer::Sent(self.text(reply).1)
            }
            Verb::Rset | Verb::Quit => {
                self.end_transaction();
                Answer::Reset
            }
        })
    }

    fn end_transaction(&mut self) {
        self.state = State::Ready;
        self.recipients = 0;
        self.transaction = MailOptions::default();
    }

    fn refusal(&self, reply: &Reply) -> Refusal {
        let (enhanced, text) = self.text(reply);
        Refusal { code: reply.code, enhanced, text }
    }

    /// A reply's enhanced status code, from a server that offers them, and its words for people: each line without
    /// its enhanced code, joined, cleaned and with the secrets hidden; empty before TLS, where anyone on the way could
    /// have written them and the person would take them for the server's.
    fn text(&self, reply: &Reply) -> (Option<EnhancedCode>, String) {
        let class = u8::try_from(reply.code / 100).unwrap_or_default();
        let enhanced = self.offers("ENHANCEDSTATUSCODES");
        let mut first = None;
        let mut parts = Vec::with_capacity(reply.lines.len());
        for line in &reply.lines {
            let (code, rest) = if enhanced { EnhancedCode::split(line, class) } else { (None, line.as_str()) };
            first = first.or(code);
            parts.push(rest);
        }
        (first, self.words(&parts.join(" ")))
    }

    /// A reply's words as they came, enhanced code and all, for an error that ends the session.
    fn plain_words(&self, reply: &Reply) -> String {
        self.words(&reply.lines.join(" "))
    }

    fn words(&self, text: &str) -> String {
        if self.tls { self.secrets.tidy(text) } else { String::new() }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
