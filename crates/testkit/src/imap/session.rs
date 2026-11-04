// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One client's session with the fake: the states of RFC 3501 section 3, the greeting, the capabilities, sign-in
//! and STARTTLS.

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rustls::ServerConfig;

use crate::imap::reader::{self, Command, MAX_LITERAL, MAX_LITERALS, Refused, Token};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    NotAuthenticated,
    Authenticated,
}

/// What the server does after writing an answer.
enum Next {
    Go,
    Logout,
    StartTls(Arc<ServerConfig>),
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
    fake: FakeImap,
    state: State,
    tls: bool,
    greeted: bool,
    bad: u32,
    reader: LineReader,
    transcript: Transcript,
}

impl Session {
    pub(crate) fn new(fake: FakeImap, tls: bool) -> Self {
        let state = if fake.preauth { State::Authenticated } else { State::NotAuthenticated };
        Self { fake, state, tls, greeted: false, bad: 0, reader: LineReader::default(), transcript: Transcript::default() }
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
            let kind = if self.state == State::Authenticated { "PREAUTH" } else { "OK" };
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
            }
        }
        Ok(None)
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
        match (name.as_str(), self.state) {
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
            ("STARTTLS", State::NotAuthenticated) => {
                let Some(config) = self.starttls() else { return self.unavailable(out, &tag, &name) };
                ok(out, &tag, "Begin TLS negotiation now");
                return Next::StartTls(config);
            }
            ("LOGIN", State::NotAuthenticated) => self.login(&tag, &args, out),
            ("AUTHENTICATE", State::NotAuthenticated) => return self.authenticate(&tag, &args, out),
            _ => return self.unavailable(out, &tag, &name),
        }
        Next::Go
    }

    fn unavailable(&mut self, out: &mut Vec<u8>, tag: &str, name: &str) -> Next {
        self.bad(out, Some(tag), &format!("{name} is not available now"));
        Next::Go
    }

    fn bad(&mut self, out: &mut Vec<u8>, tag: Option<&str>, reason: &str) {
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
        let signing_in = self.state == State::NotAuthenticated;
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

fn line(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(b"\r\n");
}

fn ok(out: &mut Vec<u8>, tag: &str, text: &str) {
    line(out, &format!("{tag} OK {text}"));
}

fn no(out: &mut Vec<u8>, tag: &str, text: &str) {
    line(out, &format!("{tag} NO {text}"));
}

/// `text` without CR, LF and NUL.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect()
}
