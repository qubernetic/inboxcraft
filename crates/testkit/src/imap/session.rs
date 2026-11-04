// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One client's session with the fake: the states of RFC 3501 section 3, the greeting, the capabilities, sign-in
//! and STARTTLS.

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rustls::ServerConfig;

use crate::imap::notify::Notify;
use crate::imap::reader::{self, Command, MAX_LITERAL, MAX_LITERALS, Refused, Token};
use crate::imap::update::{Change, Update};
use crate::imap::write::{clean, line, no, ok};
use crate::imap::{Action, Capability, FakeImap, When, base64};
use crate::line::{Line, LineReader};
use crate::server::{Ending, Transcript, pause};
use crate::{Error, Fault};

/// How many `BAD` answers end the session.
const MAX_BAD: u32 = 20;

/// Why [`Session::run`] returned.
pub(crate) enum Effect {
    /// The session is over.
    End(Ending),
    /// The server answered STARTTLS: the connection turns into TLS with this configuration, then the session goes on.
    StartTls(Arc<ServerConfig>),
}

/// The session's state (RFC 3501 section 3).
pub(super) enum State {
    NotAuthenticated,
    Authenticated,
    Selected(Selected),
}

/// The mailbox open in the selected state.
pub(super) struct Selected {
    /// Its place in [`FakeImap`]'s mailboxes.
    pub(super) index: usize,
    pub(super) read_only: bool,
    /// The UIDs the client knows, in the order of their message sequence numbers.
    pub(super) view: Vec<u32>,
}

/// [`State`] without what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    NotAuthenticated,
    Authenticated,
    Selected,
}

/// What the server does after writing an answer.
enum Next {
    Go,
    Logout,
    StartTls(Arc<ServerConfig>),
    /// The `+ idling` is sent: play the idle actions, then wait for DONE.
    Idle(String),
    /// Send an empty challenge and read the AUTHENTICATE PLAIN response.
    Challenge(String),
}

/// What the client sent.
enum Input {
    Command(Command),
    Refused(Refused),
    Closed,
    /// Nothing more can be read in step with the client: the server says goodbye and ends.
    Lost(&'static str),
}

pub(crate) struct Session {
    pub(super) fake: FakeImap,
    pub(super) state: State,
    /// CONDSTORE is enabled (RFC 7162 section 3.1): by ENABLE, a SELECT parameter, or a MODSEQ in a FETCH.
    pub(super) condstore: bool,
    /// QRESYNC is enabled (RFC 7162 section 3.2), always by ENABLE.
    pub(super) qresync: bool,
    /// What the client of the open mailbox has not been told yet.
    pub(super) pending: Vec<Update>,
    /// What the last NOTIFY SET asked for.
    pub(super) notify: Option<Notify>,
    /// The client idles: changes are told at once.
    pub(super) idling: bool,
    tls: bool,
    greeted: bool,
    bad: u32,
    reader: LineReader,
    transcript: Transcript,
}

impl Session {
    pub(crate) fn new(fake: FakeImap, tls: bool) -> Self {
        let state = if fake.preauth { State::Authenticated } else { State::NotAuthenticated };
        Self {
            fake,
            state,
            condstore: false,
            qresync: false,
            pending: Vec::new(),
            notify: None,
            idling: false,
            tls,
            greeted: false,
            bad: 0,
            reader: LineReader::default(),
            transcript: Transcript::default(),
        }
    }

    pub(crate) fn transcript(&self) -> Transcript {
        self.transcript.clone()
    }

    /// Serves the client until the session ends or the connection has to change.
    pub(crate) fn run(&mut self, io: &mut (impl Read + Write), stop: &AtomicBool) -> Result<Effect, Error> {
        if !self.greeted {
            self.greeted = true;
            if let Some(end) = self.play(io, &When::Greeting, stop)? {
                return Ok(Effect::End(end));
            }
            let kind = if self.phase() == Phase::Authenticated { "PREAUTH" } else { "OK" };
            send(io, format!("* {kind} [CAPABILITY {}] InboxCraft fake IMAP ready\r\n", self.capabilities()).as_bytes())?;
        }
        loop {
            let mut out = Vec::new();
            let command = match self.read_command(io)? {
                Input::Command(command) => command,
                Input::Refused(refused) => {
                    self.bad(&mut out, refused.tag.as_deref(), refused.reason);
                    send(io, &out)?;
                    if self.too_bad(io)? {
                        return Ok(Effect::End(Ending::Finished));
                    }
                    continue;
                }
                Input::Closed => return Ok(Effect::End(Ending::Dropped)),
                Input::Lost(reason) => {
                    send(io, format!("* BYE {reason}\r\n").as_bytes())?;
                    return Err(Error::LineTooLong);
                }
            };
            let name = command.name.clone();
            if let Some(end) = self.play(io, &When::Command(name.clone()), stop)? {
                return Ok(Effect::End(end));
            }
            let next = self.answer(command, &mut out);
            send(io, &out)?;
            let next = match next {
                Next::Challenge(tag) => {
                    send(io, b"+ \r\n")?;
                    let Line::Complete(response) = self.reader.read_line(io)? else { return Ok(Effect::End(Ending::Dropped)) };
                    self.transcript.push(String::from_utf8_lossy(&response).into_owned(), self.tls);
                    let mut out = Vec::new();
                    self.plain(&tag, &response, &mut out);
                    send(io, &out)?;
                    Next::Go
                }
                Next::Idle(tag) => {
                    self.idling = true;
                    if let Some(end) = self.play(io, &When::Idle, stop)? {
                        return Ok(Effect::End(end));
                    }
                    let done = self.reader.read_line(io)?;
                    self.idling = false;
                    let Line::Complete(done) = done else { return Ok(Effect::End(Ending::Dropped)) };
                    self.transcript.push(String::from_utf8_lossy(&done).into_owned(), self.tls);
                    let mut out = Vec::new();
                    if done.eq_ignore_ascii_case(b"DONE") {
                        ok(&mut out, &tag, "IDLE terminated");
                    } else {
                        self.bad(&mut out, Some(&tag), "Expected DONE");
                    }
                    send(io, &out)?;
                    Next::Go
                }
                next => next,
            };
            if self.too_bad(io)? {
                return Ok(Effect::End(Ending::Finished));
            }
            if let Next::Logout = next {
                return Ok(Effect::End(Ending::Finished));
            }
            if let Some(end) = self.play(io, &When::Answered(name), stop)? {
                return Ok(Effect::End(end));
            }
            if let Next::StartTls(config) = next {
                self.tls = true;
                self.reader.discard();
                return Ok(Effect::StartTls(config));
            }
        }
    }

    /// Says goodbye after too many `BAD` answers; returns whether it did.
    fn too_bad(&self, io: &mut impl Write) -> io::Result<bool> {
        if self.bad < MAX_BAD {
            return Ok(false);
        }
        send(io, b"* BYE Too many bad commands\r\n")?;
        Ok(true)
    }

    /// Plays the actions waiting for `when`; returns how the session ended when one of them ended it.
    fn play(&mut self, io: &mut impl Write, when: &When, stop: &AtomicBool) -> Result<Option<Ending>, Error> {
        for action in self.fake.rules.take(when) {
            match action {
                Action::Fault(Fault::Drop) => return Ok(Some(Ending::Dropped)),
                Action::Fault(Fault::Delay(wait)) => {
                    if pause(wait, stop) {
                        return Ok(Some(Ending::Dropped));
                    }
                }
                Action::Fault(Fault::Garbage(bytes)) => send(io, &bytes)?,
                Action::Send(line) => send(io, format!("{}\r\n", clean(&line)).as_bytes())?,
                Action::Bye(text) => {
                    send(io, format!("* BYE {}\r\n", clean(&text)).as_bytes())?;
                    return Ok(Some(Ending::Finished));
                }
                Action::Deliver { mailbox, message } => self.tell(io, &mailbox, Change::Deliver(message))?,
                Action::Expunge { mailbox, uid } => self.tell(io, &mailbox, Change::Expunge(uid))?,
                Action::Flags { mailbox, uid, flags } => self.tell(io, &mailbox, Change::Flags(uid, flags))?,
                Action::UidValidity { mailbox, value } => self.tell(io, &mailbox, Change::UidValidity(value))?,
            }
        }
        Ok(None)
    }

    /// Makes a scripted change and sends what the client hears of it at once.
    fn tell(&mut self, io: &mut impl Write, mailbox: &str, change: Change) -> io::Result<()> {
        let mut out = Vec::new();
        self.change(mailbox, change, &mut out);
        send(io, &out)
    }

    /// Reads one command with its literals, sending the go-ahead for each synchronising one.
    fn read_command(&mut self, io: &mut (impl Read + Write)) -> Result<Input, Error> {
        let mut lines: Vec<Vec<u8>> = Vec::new();
        let mut literals = Vec::new();
        loop {
            let line = match self.reader.read_line(io)? {
                Line::Complete(line) => line,
                Line::Eof => return Ok(Input::Closed),
                Line::TooLong => return Ok(Input::Lost("Line too long")),
            };
            self.transcript.push(String::from_utf8_lossy(&line).into_owned(), self.tls);
            let Some((len, plus)) = reader::literal_at_end(&line) else {
                lines.push(line);
                break;
            };
            if len > MAX_LITERAL || literals.len() >= MAX_LITERALS || (plus && !self.fake.offers(Capability::LiteralPlus)) {
                if plus {
                    return Ok(Input::Lost("The literal is refused"));
                }
                let tag = reader::tag_of(lines.first().unwrap_or(&line));
                return Ok(Input::Refused(Refused { tag, reason: "The literal is refused" }));
            }
            if !plus {
                send(io, b"+ Ready for the literal\r\n")?;
            }
            let Some(literal) = self.reader.read_bytes(io, len)? else { return Ok(Input::Closed) };
            self.transcript.push(String::from_utf8_lossy(&literal).into_owned(), self.tls);
            lines.push(line);
            literals.push(literal);
        }
        Ok(match reader::read(&lines, &literals) {
            Ok(command) => Input::Command(command),
            Err(refused) => Input::Refused(refused),
        })
    }

    /// Answers one command into `out`.
    fn answer(&mut self, command: Command, out: &mut Vec<u8>) -> Next {
        let Command { tag, name, args } = command;
        self.flush(out);
        match (name.as_str(), self.phase()) {
            ("CAPABILITY", _) => {
                line(out, &format!("* CAPABILITY {}", self.capabilities()));
                ok(out, &tag, "CAPABILITY completed");
            }
            ("NOOP", _) => ok(out, &tag, "NOOP completed"),
            ("LOGOUT", _) => {
                line(out, "* BYE Logging out");
                ok(out, &tag, "LOGOUT completed");
                return Next::Logout;
            }
            ("STARTTLS", Phase::NotAuthenticated) => {
                let Some(config) = self.starttls() else { return self.unavailable(out, &tag, &name) };
                ok(out, &tag, "Begin TLS negotiation now");
                return Next::StartTls(config);
            }
            ("LOGIN", Phase::NotAuthenticated) => self.login(&tag, &args, out),
            ("AUTHENTICATE", Phase::NotAuthenticated) => return self.authenticate(&tag, &args, out),
            ("ENABLE", Phase::Authenticated) if self.fake.offers(Capability::Enable) => self.enable(&tag, &args, out),
            ("IDLE", Phase::Authenticated | Phase::Selected) if self.fake.offers(Capability::Idle) => {
                line(out, "+ idling");
                return Next::Idle(tag);
            }
            ("NOTIFY", Phase::Authenticated | Phase::Selected) if self.fake.offers(Capability::Notify) => self.notify_set(&tag, &args, out),
            ("LIST", Phase::Authenticated | Phase::Selected) => self.list(&tag, &args, out),
            ("SELECT" | "EXAMINE", Phase::Authenticated | Phase::Selected) => self.select(&tag, &name, &args, out),
            ("STATUS", Phase::Authenticated | Phase::Selected) => self.status(&tag, &args, out),
            ("CLOSE", Phase::Selected) => self.close(&tag, out),
            ("UID FETCH", Phase::Selected) => self.uid_fetch(&tag, &args, out),
            ("UID SEARCH", Phase::Selected) => self.uid_search(&tag, &args, out),
            ("UID STORE", Phase::Selected) => self.uid_store(&tag, &args, out),
            ("UID COPY", Phase::Selected) => self.uid_copy(&tag, &args, out, false),
            ("UID MOVE", Phase::Selected) if self.fake.offers(Capability::Move) => self.uid_copy(&tag, &args, out, true),
            ("UID EXPUNGE", Phase::Selected) if self.fake.offers(Capability::UidPlus) => self.uid_expunge(&tag, &args, out),
            ("EXPUNGE", Phase::Selected) => self.expunge(&tag, out),
            ("FETCH" | "STORE" | "SEARCH" | "COPY" | "MOVE", _) => {
                self.bad(out, Some(&tag), &format!("{name} is not available now (the fake knows the UID form only)"));
            }
            _ => return self.unavailable(out, &tag, &name),
        }
        Next::Go
    }

    fn unavailable(&mut self, out: &mut Vec<u8>, tag: &str, name: &str) -> Next {
        self.bad(out, Some(tag), &format!("{name} is not available now"));
        Next::Go
    }

    pub(super) fn phase(&self) -> Phase {
        match self.state {
            State::NotAuthenticated => Phase::NotAuthenticated,
            State::Authenticated => Phase::Authenticated,
            State::Selected(_) => Phase::Selected,
        }
    }

    pub(super) fn bad(&mut self, out: &mut Vec<u8>, tag: Option<&str>, reason: &str) {
        self.bad = self.bad.saturating_add(1);
        line(out, &format!("{} BAD {reason}", tag.unwrap_or("*")));
    }

    /// The capabilities as listed now.
    fn capabilities(&self) -> String {
        let mut names = vec!["IMAP4rev1"];
        names.extend(Capability::ALL.into_iter().filter(|&cap| self.listed(cap)).map(Capability::name));
        names.join(" ")
    }

    fn listed(&self, cap: Capability) -> bool {
        let signing_in = self.phase() == Phase::NotAuthenticated;
        self.fake.offers(cap)
            && match cap {
                Capability::StartTls => signing_in && !self.tls && self.fake.starttls.is_some(),
                Capability::LoginDisabled => signing_in && !self.tls,
                Capability::AuthPlain | Capability::SaslIr => signing_in && self.sign_in_allowed(),
                _ => true,
            }
    }

    /// The STARTTLS configuration, when STARTTLS is offered now.
    fn starttls(&self) -> Option<Arc<ServerConfig>> {
        self.fake.starttls.clone().filter(|_| self.listed(Capability::StartTls))
    }

    fn sign_in_allowed(&self) -> bool {
        self.tls || !self.fake.offers(Capability::LoginDisabled)
    }

    fn login(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        if !self.sign_in_allowed() {
            return no(out, tag, "[PRIVACYREQUIRED] Sign in after TLS");
        }
        let (Some(user), Some(password), None) = (args.first().and_then(Token::astring), args.get(1).and_then(Token::astring), args.get(2)) else {
            return self.bad(out, Some(tag), "LOGIN needs a user name and a password");
        };
        let good = user == self.fake.user.as_bytes() && password == self.fake.password.as_bytes();
        self.sign_in(tag, good, out);
    }

    fn authenticate(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) -> Next {
        if !self.sign_in_allowed() {
            no(out, tag, "[PRIVACYREQUIRED] Sign in after TLS");
            return Next::Go;
        }
        let Some(mechanism) = args.first().and_then(Token::atom).map(str::to_ascii_uppercase) else {
            self.bad(out, Some(tag), "AUTHENTICATE needs a mechanism");
            return Next::Go;
        };
        if mechanism != "PLAIN" || !self.fake.offers(Capability::AuthPlain) {
            self.bad(out, Some(tag), &format!("AUTHENTICATE {mechanism} is not offered"));
            return Next::Go;
        }
        match args.get(1) {
            None => return Next::Challenge(tag.to_owned()),
            Some(_) if !self.fake.offers(Capability::SaslIr) => self.bad(out, Some(tag), "SASL-IR is not offered"),
            Some(response) => self.plain(tag, response.astring().unwrap_or_default(), out),
        }
        Next::Go
    }

    /// Checks an AUTHENTICATE PLAIN response (RFC 4616): `*` cancels, `=` is empty.
    fn plain(&mut self, tag: &str, response: &[u8], out: &mut Vec<u8>) {
        if response == b"*" {
            return self.bad(out, Some(tag), "Authentication cancelled");
        }
        let decoded = if response == b"=" { Some(Vec::new()) } else { base64::decode(response) };
        let Some(decoded) = decoded else { return self.bad(out, Some(tag), "The response is not base64") };
        let parts: Vec<&[u8]> = decoded.split(|&byte| byte == 0).collect();
        let good = match parts.as_slice() {
            [authz, user, password] => {
                (authz.is_empty() || authz == user) && *user == self.fake.user.as_bytes() && *password == self.fake.password.as_bytes()
            }
            _ => false,
        };
        self.sign_in(tag, good, out);
    }

    fn sign_in(&mut self, tag: &str, good: bool, out: &mut Vec<u8>) {
        if !good {
            return no(out, tag, "[AUTHENTICATIONFAILED] Invalid credentials");
        }
        self.state = State::Authenticated;
        line(out, &format!("{tag} OK [CAPABILITY {}] Logged in", self.capabilities()));
    }
}

fn send(io: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    io.write_all(bytes)?;
    io.flush()
}
