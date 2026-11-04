// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The client state machine (RFC 9051 section 3): bytes in, bytes out, events.

use std::collections::{HashSet, VecDeque};
use std::fmt;

use inboxcraft_model::{Uid, UidSet};

use super::capability::{Capabilities, MAX_CAPABILITY_LEN, is_atom_char};
use super::command::{Command, escaped};
use super::fetch::{FetchAttributes, FetchItem};
use super::frame::Framer;
use super::list::{ListEntry, MAX_MAILBOX_NAME, MAX_MAILBOXES};
use super::mailbox::{MailboxStatus, flag_list};
use super::response::{Response, classify};
use super::sasl::{check_credentials, plain_response};
use super::search::{self, SearchKey, set_text};
use super::text::Secrets;
use super::value::{Piece, Status, StatusLine, Value, number};
use crate::ProtoError;

/// How the connection started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// Implicit TLS (port 993): encrypted from the first byte.
    Tls,
    /// Not encrypted (port 143): STARTTLS must come before any sign-in.
    Plain,
}

/// The session's state (RFC 9051 section 3), with the two waits this client adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Waiting for the server's greeting.
    Greeting,
    /// Greeted, not signed in.
    NotAuthenticated,
    /// Signed in, no mailbox open.
    Authenticated,
    /// A mailbox is open.
    Selected,
    /// The server agreed to STARTTLS: the caller runs the TLS handshake, then calls [`Client::tls_established`].
    StartingTls,
    /// LOGOUT was sent.
    LoggingOut,
    /// The session is over.
    Closed,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Greeting => "greeting",
            Self::NotAuthenticated => "not authenticated",
            Self::Authenticated => "authenticated",
            Self::Selected => "selected",
            Self::StartingTls => "starting TLS",
            Self::LoggingOut => "logging out",
            Self::Closed => "closed",
        })
    }
}

/// The tag of a command: its answer carries it back in [`Event::Completed`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tag(String);

impl Tag {
    /// The tag as sent (`A0001`).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the server answered or told, in the order it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The server greeted; `preauth` when it signed the session in by itself (only over TLS).
    Greeted {
        /// The greeting was PREAUTH.
        preauth: bool,
    },
    /// A command is done.
    Completed {
        /// The command's tag.
        tag: Tag,
        /// What it brought, or the server's refusal.
        outcome: Result<Answer, Refusal>,
    },
    /// A FETCH response: one asked for, or the server telling of a change.
    Fetched(FetchItem),
    /// The open mailbox now holds this many messages.
    Exists(u32),
    /// The message with this sequence number is gone; the ones after it move up by one.
    Expunged(u32),
    /// A text the server says must be shown to the person (`[ALERT]`), cleaned; only over TLS.
    Alert(String),
}

/// What a command brought when it succeeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// CAPABILITY.
    Capabilities(Capabilities),
    /// STARTTLS: run the TLS handshake now, then call [`Client::tls_established`].
    StartTls,
    /// LOGIN or AUTHENTICATE, with the capabilities as the server lists them after the sign-in.
    Authenticated(Capabilities),
    /// LIST: the mailboxes, each once, at most [`MAX_MAILBOXES`].
    Listed(Vec<ListEntry>),
    /// SELECT or EXAMINE.
    Selected(MailboxStatus),
    /// UID FETCH: the messages came as [`Event::Fetched`].
    Fetched,
    /// UID SEARCH: the UIDs found.
    Searched(UidSet),
    /// LOGOUT.
    LoggedOut,
}

/// How the server refused a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusalKind {
    /// `NO`: the server would not.
    No,
    /// `BAD`: the server did not understand.
    Bad,
}

/// A command the server refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// `NO` or `BAD`.
    pub kind: RefusalKind,
    /// The response code, upper-cased (`AUTHENTICATIONFAILED`), when there is one made of atom characters, at most
    /// [`MAX_CAPABILITY_LEN`] long and holding no password.
    pub code: Option<String>,
    /// The server's words, cleaned and capped, with the password hidden; empty before TLS.
    pub text: String,
}

/// What a command waits for.
enum Kind {
    /// CAPABILITY; `after_sign_in` when this client sent it after a sign-in that did not list them, which completes
    /// with it.
    Capability {
        after_sign_in: Option<Tag>,
    },
    StartTls,
    /// LOGIN or AUTHENTICATE PLAIN.
    SignIn,
    List {
        entries: Vec<ListEntry>,
        seen: HashSet<String>,
    },
    Select(MailboxStatus),
    Fetch,
    Search(Vec<Uid>),
    Logout {
        previous: State,
    },
}

/// The command waiting for its answer.
struct InFlight {
    tag: Tag,
    kind: Kind,
    /// The rest of the command, a chunk for each go-ahead.
    waiting: VecDeque<Vec<u8>>,
}

/// An IMAP4rev1 and IMAP4rev2 client without I/O.
///
/// Feed it what the server sent with [`receive`](Self::receive), write what [`take_output`](Self::take_output) gives
/// to the server, and read what happened from [`next_event`](Self::next_event). One command at a time.
///
/// LOGIN and AUTHENTICATE are refused with [`ProtoError::NotEncrypted`] until the connection is encrypted, and the
/// capabilities are read again after STARTTLS and after a sign-in. Everything the server sends is hostile: lines,
/// literals and lists are capped, and a fatal [`ProtoError`] closes the session.
pub struct Client {
    state: State,
    tls: bool,
    framer: Framer,
    output: Vec<u8>,
    events: VecDeque<Event>,
    capabilities: Option<Capabilities>,
    /// An untagged CAPABILITY came during the command in flight.
    fresh_capabilities: bool,
    next_tag: u32,
    in_flight: Option<InFlight>,
    secrets: Secrets,
}

/// Neither the output (it may hold a password) nor anything secret is shown.
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("state", &self.state)
            .field("tls", &self.tls)
            .field("in_flight", &self.in_flight.as_ref().map(|flight| &flight.tag))
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
            framer: Framer::default(),
            output: Vec::new(),
            events: VecDeque::new(),
            capabilities: None,
            fresh_capabilities: false,
            next_tag: 1,
            in_flight: None,
            secrets: Secrets::default(),
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

    /// What the server said it can do, `None` until it has (and again after STARTTLS and a sign-in, until read).
    #[must_use]
    pub fn capabilities(&self) -> Option<&Capabilities> {
        self.capabilities.as_ref()
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
    /// A fatal [`ProtoError`] when the server broke the protocol or a limit; the session is then
    /// [`Closed`](State::Closed) and every later call answers [`ProtoError::Closed`].
    pub fn receive(&mut self, bytes: &[u8]) -> Result<(), ProtoError> {
        match self.state {
            State::Closed => return Err(ProtoError::Closed),
            State::StartingTls if !bytes.is_empty() => return Err(self.fail(ProtoError::PlaintextAfterStartTls)),
            _ => {}
        }
        self.framer.push(bytes);
        while !matches!(self.state, State::StartingTls | State::Closed) {
            let pieces = match self.framer.next() {
                Ok(Some(pieces)) => pieces,
                Ok(None) => break,
                Err(error) => return Err(self.fail(error)),
            };
            if let Err(error) = self.handle(pieces) {
                return Err(self.fail(error));
            }
        }
        Ok(())
    }

    /// CAPABILITY.
    ///
    /// # Errors
    ///
    /// [`ProtoError::Busy`], [`ProtoError::WrongState`] before the greeting, or [`ProtoError::Closed`].
    pub fn capability(&mut self) -> Result<Tag, ProtoError> {
        self.ready("CAPABILITY", &[State::NotAuthenticated, State::Authenticated, State::Selected])?;
        Ok(self.start(Command::new("CAPABILITY"), Kind::Capability { after_sign_in: None }))
    }

    /// STARTTLS, on a connection that is not encrypted and whose server offers it.
    ///
    /// # Errors
    ///
    /// [`ProtoError::AlreadyEncrypted`], [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`], or a state
    /// error.
    pub fn starttls(&mut self) -> Result<Tag, ProtoError> {
        self.ready("STARTTLS", &[State::NotAuthenticated])?;
        if self.tls {
            return Err(ProtoError::AlreadyEncrypted);
        }
        let capabilities = self.capabilities.as_ref().ok_or(ProtoError::CapabilitiesUnknown)?;
        if !capabilities.has("STARTTLS") {
            return Err(ProtoError::NotOffered("STARTTLS"));
        }
        Ok(self.start(Command::new("STARTTLS"), Kind::StartTls))
    }

    /// After the TLS handshake that followed STARTTLS: the session goes on encrypted, forgets the capabilities it
    /// was told before, and asks for them again with a CAPABILITY whose tag this is.
    ///
    /// # Errors
    ///
    /// [`ProtoError::WrongState`] unless STARTTLS was just agreed, or [`ProtoError::Closed`].
    pub fn tls_established(&mut self) -> Result<Tag, ProtoError> {
        match self.state {
            State::StartingTls => {}
            State::Closed => return Err(ProtoError::Closed),
            state => return Err(ProtoError::WrongState { command: "STARTTLS", state }),
        }
        self.tls = true;
        self.state = State::NotAuthenticated;
        self.capabilities = None;
        self.framer = Framer::default();
        Ok(self.start(Command::new("CAPABILITY"), Kind::Capability { after_sign_in: None }))
    }

    /// LOGIN, over an encrypted connection to a server that does not say `LOGINDISABLED`.
    ///
    /// # Errors
    ///
    /// [`ProtoError::NotEncrypted`] before TLS, [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`],
    /// [`ProtoError::BadArgument`] for an empty user name or password or one with a NUL, or a state error.
    pub fn login(&mut self, user: &str, password: &str) -> Result<Tag, ProtoError> {
        self.ready("LOGIN", &[State::NotAuthenticated])?;
        if self.sign_in_capabilities()?.has("LOGINDISABLED") {
            return Err(ProtoError::NotOffered("LOGIN"));
        }
        check_credentials(user, password)?;
        self.secrets.add(password);
        // The form inside the quotes, when it differs: a server may write the argument back as it came.
        self.secrets.add(&String::from_utf8_lossy(&escaped(password.as_bytes())));
        let command = Command::new("LOGIN").string(user.as_bytes()).string(password.as_bytes());
        Ok(self.start(command, Kind::SignIn))
    }

    /// AUTHENTICATE PLAIN (RFC 4616), over an encrypted connection to a server that offers `AUTH=PLAIN`; with
    /// `SASL-IR` (RFC 4959) the response goes with the command, otherwise after the server's go-ahead.
    ///
    /// # Errors
    ///
    /// As [`login`](Self::login), and [`ProtoError::NotOffered`] without `AUTH=PLAIN`.
    pub fn authenticate_plain(&mut self, user: &str, password: &str) -> Result<Tag, ProtoError> {
        self.ready("AUTHENTICATE", &[State::NotAuthenticated])?;
        let capabilities = self.sign_in_capabilities()?;
        if !capabilities.has("AUTH=PLAIN") {
            return Err(ProtoError::NotOffered("AUTH=PLAIN"));
        }
        let inline = capabilities.has("SASL-IR");
        let response = plain_response(user, password)?;
        self.secrets.add(password);
        self.secrets.add(&response);
        let command = Command::new("AUTHENTICATE PLAIN");
        if inline {
            return Ok(self.start(command.arg(&response), Kind::SignIn));
        }
        let tag = self.start(command, Kind::SignIn);
        if let Some(flight) = self.in_flight.as_mut() {
            flight.waiting.push_back(format!("{response}\r\n").into_bytes());
        }
        Ok(tag)
    }

    /// LIST `""` `"*"`: every mailbox, with `RETURN (SPECIAL-USE)` when the server offers LIST-EXTENDED and
    /// SPECIAL-USE (RFC 5258, RFC 6154).
    ///
    /// # Errors
    ///
    /// [`ProtoError::Busy`], a state error, or [`ProtoError::Closed`].
    pub fn list(&mut self) -> Result<Tag, ProtoError> {
        self.ready("LIST", &[State::Authenticated, State::Selected])?;
        let offers = |capability| self.capabilities.as_ref().is_some_and(|capabilities| capabilities.has(capability));
        let mut command = Command::new("LIST").arg("\"\"").arg("\"*\"");
        if offers("LIST-EXTENDED") && offers("SPECIAL-USE") {
            command = command.arg("RETURN (SPECIAL-USE)");
        }
        Ok(self.start(command, Kind::List { entries: Vec::new(), seen: HashSet::new() }))
    }

    /// SELECT: opens a mailbox to read and change. `mailbox` is the name as the server wrote it ([`ListEntry::raw`]).
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for an empty or overlong name or one with a control character, a state error.
    pub fn select(&mut self, mailbox: &str) -> Result<Tag, ProtoError> {
        self.open("SELECT", mailbox, false)
    }

    /// EXAMINE: opens a mailbox read-only. As [`select`](Self::select).
    ///
    /// # Errors
    ///
    /// As [`select`](Self::select).
    pub fn examine(&mut self, mailbox: &str) -> Result<Tag, ProtoError> {
        self.open("EXAMINE", mailbox, true)
    }

    fn open(&mut self, verb: &'static str, mailbox: &str, read_only: bool) -> Result<Tag, ProtoError> {
        self.ready(verb, &[State::Authenticated, State::Selected])?;
        if mailbox.is_empty() || mailbox.len() > MAX_MAILBOX_NAME || mailbox.chars().any(char::is_control) {
            return Err(ProtoError::BadArgument("mailbox name"));
        }
        // Whatever the answer, the mailbox open before is closed (RFC 9051 section 6.3.2).
        self.state = State::Authenticated;
        let command = Command::new(verb).string(mailbox.as_bytes());
        Ok(self.start(command, Kind::Select(MailboxStatus { read_only, ..MailboxStatus::default() })))
    }

    /// UID FETCH of the messages in `set`, with their UID and `attributes`; they come as [`Event::Fetched`].
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for an empty set or one of more than [`crate::imap::MAX_SET_RANGES`] ranges, a
    /// state error.
    pub fn uid_fetch(&mut self, set: &UidSet, attributes: FetchAttributes) -> Result<Tag, ProtoError> {
        self.ready("UID FETCH", &[State::Selected])?;
        let command = Command::new("UID FETCH").arg(&set_text(set)?).arg(&attributes.items());
        Ok(self.start(command, Kind::Fetch))
    }

    /// UID SEARCH: the UIDs of the messages that match `key`.
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for a key that cannot be sent (see [`SearchKey`]), a state error.
    pub fn uid_search(&mut self, key: &SearchKey) -> Result<Tag, ProtoError> {
        self.ready("UID SEARCH", &[State::Selected])?;
        let command = search::command(key)?;
        Ok(self.start(command, Kind::Search(Vec::new())))
    }

    /// LOGOUT.
    ///
    /// # Errors
    ///
    /// [`ProtoError::Busy`], a state error, or [`ProtoError::Closed`].
    pub fn logout(&mut self) -> Result<Tag, ProtoError> {
        self.ready("LOGOUT", &[State::NotAuthenticated, State::Authenticated, State::Selected])?;
        let previous = self.state;
        self.state = State::LoggingOut;
        Ok(self.start(Command::new("LOGOUT"), Kind::Logout { previous }))
    }

    /// The capabilities a sign-in is checked against: only over TLS, and only once the server has listed them.
    fn sign_in_capabilities(&self) -> Result<&Capabilities, ProtoError> {
        if !self.tls {
            return Err(ProtoError::NotEncrypted);
        }
        self.capabilities.as_ref().ok_or(ProtoError::CapabilitiesUnknown)
    }

    fn ready(&self, command: &'static str, allowed: &[State]) -> Result<(), ProtoError> {
        if self.state == State::Closed {
            return Err(ProtoError::Closed);
        }
        if self.in_flight.is_some() {
            return Err(ProtoError::Busy);
        }
        if !allowed.contains(&self.state) {
            return Err(ProtoError::WrongState { command, state: self.state });
        }
        Ok(())
    }

    fn start(&mut self, command: Command, kind: Kind) -> Tag {
        let tag = Tag(format!("A{:04}", self.next_tag));
        self.next_tag = self.next_tag.wrapping_add(1);
        let (first, waiting) = command.finish(tag.as_str());
        self.output.extend_from_slice(&first);
        self.fresh_capabilities = false;
        self.in_flight = Some(InFlight { tag: tag.clone(), kind, waiting });
        tag
    }

    /// Ends the session after a fatal error: nothing more is sent or read.
    fn fail(&mut self, error: ProtoError) -> ProtoError {
        self.state = State::Closed;
        self.in_flight = None;
        self.output.clear();
        error
    }

    fn handle(&mut self, pieces: Vec<Piece>) -> Result<(), ProtoError> {
        let response = classify(pieces)?;
        if self.state == State::Greeting {
            return match response {
                Response::Status(line) => self.greeting(&line),
                _ => Err(ProtoError::Unexpected("response before the greeting")),
            };
        }
        match response {
            Response::Continue => self.go_ahead(),
            Response::Tagged { tag, line } => self.tagged(&tag, &line),
            Response::Status(line) => self.untagged_status(&line),
            Response::Data(values) => self.data(values),
        }
    }

    fn greeting(&mut self, line: &StatusLine) -> Result<(), ProtoError> {
        match line.status {
            Status::Ok => self.state = State::NotAuthenticated,
            Status::Preauth if self.tls => self.state = State::Authenticated,
            Status::Preauth => return Err(ProtoError::PreauthWithoutTls),
            Status::Bye => return Err(ProtoError::Bye(self.words(&line.text))),
            Status::No | Status::Bad => return Err(ProtoError::Malformed("greeting")),
        }
        self.note_code(line);
        self.events.push_back(Event::Greeted { preauth: line.status == Status::Preauth });
        Ok(())
    }

    fn untagged_status(&mut self, line: &StatusLine) -> Result<(), ProtoError> {
        match line.status {
            Status::Bye if self.state == State::LoggingOut => Ok(()),
            Status::Bye => Err(ProtoError::Bye(self.words(&line.text))),
            Status::Preauth => Err(ProtoError::Unexpected("PREAUTH")),
            Status::Ok | Status::No | Status::Bad => {
                self.note_code(line);
                if let Some(InFlight { kind: Kind::Select(status), .. }) = self.in_flight.as_mut() {
                    status.note_code(line);
                }
                Ok(())
            }
        }
    }

    /// Server text for people, cleaned and with the secrets hidden; empty before TLS, where anyone on the way could
    /// have written it and the person would take it for the server's.
    fn words(&self, text: &str) -> String {
        if self.tls { self.secrets.tidy(text) } else { String::new() }
    }

    /// A response code for the caller: a word of atom characters at most [`MAX_CAPABILITY_LEN`] long that holds no
    /// secret, or `None`.
    fn code(&self, code: &str) -> Option<String> {
        let clean = !code.is_empty() && code.len() <= MAX_CAPABILITY_LEN && code.bytes().all(is_atom_char);
        (clean && !self.secrets.shows(code)).then(|| code.to_owned())
    }

    /// What a status line's response code says about the session.
    fn note_code(&mut self, line: &StatusLine) {
        match line.code.as_str() {
            "CAPABILITY" => {
                self.capabilities = Some(Capabilities::from_words(line.arg.split_ascii_whitespace()));
                self.fresh_capabilities = true;
            }
            // In the clear anyone on the way could have written it, and the person would take it for the server's.
            "ALERT" if self.tls => self.events.push_back(Event::Alert(self.secrets.tidy(&line.text))),
            _ => {}
        }
    }

    fn go_ahead(&mut self) -> Result<(), ProtoError> {
        let chunk = self.in_flight.as_mut().and_then(|flight| flight.waiting.pop_front());
        let chunk = chunk.ok_or(ProtoError::Unexpected("continuation request"))?;
        self.output.extend_from_slice(&chunk);
        Ok(())
    }

    fn data(&mut self, values: Vec<Value>) -> Result<(), ProtoError> {
        let first = values.first().and_then(Value::atom).map(str::to_ascii_uppercase).unwrap_or_default();
        let flight = self.in_flight.as_mut().map(|flight| &mut flight.kind);
        match (first.as_str(), flight) {
            ("CAPABILITY", _) => {
                self.capabilities = Some(Capabilities::from_words(values.iter().skip(1).filter_map(Value::atom)));
                self.fresh_capabilities = true;
            }
            ("LIST", Some(Kind::List { entries, seen })) => {
                if entries.len() < MAX_MAILBOXES
                    && let Some(entry) = ListEntry::from_values(values)
                    && seen.insert(entry.raw.clone())
                {
                    entries.push(entry);
                }
            }
            ("FLAGS", Some(Kind::Select(status))) => {
                if let Some(Value::List(flags)) = values.into_iter().nth(1) {
                    status.flags = flag_list(flags);
                }
            }
            ("SEARCH", Some(Kind::Search(found))) => {
                found.extend(values.iter().skip(1).filter_map(Value::number).filter_map(Uid::new));
            }
            (word, flight) => {
                if let Some(count) = number(word) {
                    let open = self.state == State::Selected || matches!(flight, Some(Kind::Select(_) | Kind::Logout { previous: State::Selected }));
                    message_data(count, values, flight, open, &mut self.events)?;
                }
            }
        }
        Ok(())
    }

    fn tagged(&mut self, tag: &str, line: &StatusLine) -> Result<(), ProtoError> {
        let flight = self.in_flight.take().filter(|flight| flight.tag.as_str() == tag).ok_or(ProtoError::Unexpected("tagged response"))?;
        if line.status == Status::Ok && !flight.waiting.is_empty() {
            // The command cannot have succeeded without the rest of it.
            return Err(ProtoError::Unexpected("tagged response"));
        }
        // Capabilities on the answer to STARTTLS were sent in the clear: they are read again over TLS.
        if !matches!(flight.kind, Kind::StartTls) {
            self.note_code(line);
        }
        let fresh = std::mem::take(&mut self.fresh_capabilities);
        let refusal = match line.status {
            Status::Ok => None,
            Status::No => Some(RefusalKind::No),
            _ => Some(RefusalKind::Bad),
        };
        let refusal = refusal.map(|kind| Refusal { kind, code: self.code(&line.code), text: self.words(&line.text) });
        let (tag, kind) = match flight.kind {
            Kind::Capability { after_sign_in: Some(sign_in) } => {
                let capabilities = if refusal.is_none() { self.capabilities.get_or_insert_default().clone() } else { Capabilities::default() };
                self.events.push_back(Event::Completed { tag: sign_in, outcome: Ok(Answer::Authenticated(capabilities)) });
                return Ok(());
            }
            kind => (flight.tag, kind),
        };
        let answer = match (kind, refusal) {
            (Kind::Logout { previous }, Some(refusal)) => {
                self.state = previous;
                Err(refusal)
            }
            (Kind::Select(_), Some(refusal)) => {
                self.state = State::Authenticated;
                Err(refusal)
            }
            (_, Some(refusal)) => Err(refusal),
            (kind, None) => match self.succeeded(kind, &tag, line, fresh)? {
                Some(answer) => Ok(answer),
                None => return Ok(()),
            },
        };
        self.events.push_back(Event::Completed { tag, outcome: answer });
        Ok(())
    }

    /// The answer of a command the server completed with OK; `None` while a CAPABILITY must follow first.
    fn succeeded(&mut self, kind: Kind, tag: &Tag, line: &StatusLine, fresh: bool) -> Result<Option<Answer>, ProtoError> {
        let answer = match kind {
            Kind::Capability { .. } => {
                let capabilities = self.capabilities.get_or_insert_default().clone();
                Answer::Capabilities(capabilities)
            }
            Kind::StartTls => {
                // Bytes after the go-ahead came before encryption: anyone on the way could have put them there.
                if self.framer.has_pending() {
                    return Err(ProtoError::PlaintextAfterStartTls);
                }
                self.state = State::StartingTls;
                Answer::StartTls
            }
            Kind::SignIn => {
                self.state = State::Authenticated;
                if !fresh {
                    // The capabilities change with the sign-in (RFC 9051 section 6.2.3): read them again.
                    self.capabilities = None;
                    self.start(Command::new("CAPABILITY"), Kind::Capability { after_sign_in: Some(tag.clone()) });
                    return Ok(None);
                }
                Answer::Authenticated(self.capabilities.clone().unwrap_or_default())
            }
            Kind::List { entries, .. } => Answer::Listed(entries),
            Kind::Select(mut status) => {
                match line.code.as_str() {
                    "READ-ONLY" => status.read_only = true,
                    "READ-WRITE" => status.read_only = false,
                    _ => {}
                }
                self.state = State::Selected;
                Answer::Selected(status)
            }
            Kind::Fetch => Answer::Fetched,
            Kind::Search(found) => Answer::Searched(found.into_iter().collect()),
            Kind::Logout { .. } => {
                self.state = State::Closed;
                Answer::LoggedOut
            }
        };
        Ok(Some(answer))
    }
}

/// `* n EXISTS`, `* n RECENT`, `* n EXPUNGE`, `* n FETCH (...)`. EXISTS, EXPUNGE and FETCH belong to a mailbox
/// that is `open` or being opened (RFC 9051 section 7.4); anywhere else, before TLS too, they end the session.
fn message_data(count: u32, values: Vec<Value>, flight: Option<&mut Kind>, open: bool, events: &mut VecDeque<Event>) -> Result<(), ProtoError> {
    let word = values.get(1).and_then(Value::atom).map(str::to_ascii_uppercase).unwrap_or_default();
    match (word.as_str(), flight) {
        ("EXISTS", _) if !open => return Err(ProtoError::Unexpected("EXISTS response")),
        ("EXPUNGE", _) if !open => return Err(ProtoError::Unexpected("EXPUNGE response")),
        ("FETCH", _) if !open => return Err(ProtoError::Unexpected("FETCH response")),
        ("EXISTS", Some(Kind::Select(status))) => status.exists = count,
        ("EXISTS", _) => events.push_back(Event::Exists(count)),
        ("RECENT", Some(Kind::Select(status))) => status.recent = count,
        ("EXPUNGE", _) if count != 0 => events.push_back(Event::Expunged(count)),
        ("EXPUNGE", _) => return Err(ProtoError::Malformed("EXPUNGE response")),
        ("FETCH", _) => {
            let item = FetchItem::from_values(values).ok_or(ProtoError::Malformed("FETCH response"))?;
            events.push_back(Event::Fetched(item));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
