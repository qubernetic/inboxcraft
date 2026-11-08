// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One client's session with the fake: the greeting, EHLO, STARTTLS, AUTH, the mail transaction and DATA.

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rustls::ServerConfig;

use crate::base64;
use crate::line::{Line, LineReader};
use crate::server::{Ending, Transcript, pause};
use crate::smtp::outbox::Submission;
use crate::smtp::reader::{self, Command, Refused};
use crate::smtp::rules::{Action, When, clean, write_reply};
use crate::smtp::{DOMAIN, Extension, FakeSmtp};
use crate::{Error, Fault};

/// How many refused commands end the session.
const MAX_BAD: u32 = 20;
/// The most recipients of one message.
const MAX_RECIPIENTS: usize = 100;
/// `Username:` in base64, the first challenge of AUTH LOGIN.
const USERNAME: &str = "VXNlcm5hbWU6";
/// `Password:` in base64, the second challenge of AUTH LOGIN.
const PASSWORD: &str = "UGFzc3dvcmQ6";

/// Why [`Session::run`] returned.
pub(crate) enum Effect {
    /// The session is over.
    End(Ending),
    /// The server answered STARTTLS: the connection turns into TLS with this configuration, then the session goes on.
    StartTls(Arc<ServerConfig>),
}

/// What the server does after writing an answer.
enum Next {
    Go,
    Quit,
    StartTls(Arc<ServerConfig>),
    /// The `354` is sent: read the message.
    Data,
    /// A challenge is sent: read the AUTH response.
    Auth(Step),
}

/// The response an AUTH exchange waits for.
enum Step {
    /// The PLAIN response (RFC 4616).
    Plain,
    /// The LOGIN user name.
    User,
    /// The LOGIN password, after this user name.
    Password(Vec<u8>),
}

/// What playing the actions at a [`When`] did.
enum Played {
    /// Nothing that changes the answer.
    Go,
    /// A scripted reply was sent instead of the server's own.
    Replied,
    /// The session is over.
    End(Ending),
}

/// The mail transaction: MAIL FROM, then RCPT TO.
struct Transaction {
    from: String,
    params: Vec<String>,
    recipients: Vec<String>,
}

pub(crate) struct Session {
    fake: FakeSmtp,
    tls: bool,
    greeted: bool,
    /// A refused greeting leaves the server answering every command but QUIT with `503`.
    serving: bool,
    /// EHLO or HELO was said on this connection, since the last STARTTLS.
    hello: bool,
    user: Option<String>,
    mail: Option<Transaction>,
    bad: u32,
    reader: LineReader,
    transcript: Transcript,
}

impl Session {
    pub(crate) fn new(fake: FakeSmtp, tls: bool) -> Self {
        Self {
            fake,
            tls,
            greeted: false,
            serving: true,
            hello: false,
            user: None,
            mail: None,
            bad: 0,
            reader: LineReader::default(),
            transcript: Transcript::default(),
        }
    }

    /// Records what the client sends into `transcript`, from the start.
    pub(crate) fn recording(self, transcript: Transcript) -> Self {
        Self { transcript, ..self }
    }

    pub(crate) fn transcript(&self) -> Transcript {
        self.transcript.clone()
    }

    /// The fake as the session left it: its played rules gone.
    pub(crate) fn into_fake(self) -> FakeSmtp {
        self.fake
    }

    /// Serves the client until the session ends or the connection has to change.
    pub(crate) fn run(&mut self, io: &mut (impl Read + Write), stop: &AtomicBool) -> Result<Effect, Error> {
        if !self.greeted {
            self.greeted = true;
            match self.play(io, &When::Greeting, stop)? {
                Played::End(end) => return Ok(Effect::End(end)),
                Played::Replied => {}
                Played::Go => send(io, format!("220 {DOMAIN} ESMTP InboxCraft fake SMTP ready\r\n").as_bytes())?,
            }
        }
        loop {
            let line = match self.reader.read_line(io)? {
                Line::Complete(line) => line,
                Line::Eof => return Ok(Effect::End(Ending::Dropped)),
                Line::TooLong(cut) => return self.cut(io, &cut),
            };
            self.hear(&line);
            let (verb, command) = reader::read(&line);
            let mut out = Vec::new();
            if !self.serving && verb != "QUIT" {
                self.refuse(&mut out, 503, "5.5.1", "No service");
                send(io, &out)?;
                if self.too_bad(io)? {
                    return Ok(Effect::End(Ending::Finished));
                }
                continue;
            }
            match self.play(io, &When::Command(verb.clone()), stop)? {
                Played::End(end) => return Ok(Effect::End(end)),
                Played::Replied => continue,
                Played::Go => {}
            }
            if let Ok(Command::Rcpt { path, .. }) = &command {
                match self.play(io, &When::Rcpt(path.to_ascii_lowercase()), stop)? {
                    Played::End(end) => return Ok(Effect::End(end)),
                    Played::Replied => continue,
                    Played::Go => {}
                }
            }
            let next = match command {
                Ok(command) => self.answer(command, &mut out),
                Err(refused) => {
                    self.refused(refused, &mut out);
                    Next::Go
                }
            };
            send(io, &out)?;
            let next = match next {
                Next::Data => match self.data(io, stop)? {
                    Some(end) => return Ok(Effect::End(end)),
                    None => Next::Go,
                },
                Next::Auth(step) => match self.auth(io, step)? {
                    Some(end) => return Ok(Effect::End(end)),
                    None => Next::Go,
                },
                next => next,
            };
            if let Next::Quit = next {
                return Ok(Effect::End(Ending::Finished));
            }
            if self.too_bad(io)? {
                return Ok(Effect::End(Ending::Finished));
            }
            if let Played::End(end) = self.play(io, &When::Answered(verb), stop)? {
                return Ok(Effect::End(end));
            }
            if let Next::StartTls(config) = next {
                self.discard();
                self.tls = true;
                self.hello = false;
                self.user = None;
                self.mail = None;
                return Ok(Effect::StartTls(config));
            }
        }
    }

    /// Records a line the client sent.
    fn hear(&mut self, line: &[u8]) {
        self.transcript.push(String::from_utf8_lossy(line).into_owned(), self.tls);
    }

    /// Records what the client sent past STARTTLS, unanswered: it came before the change, so after it the bytes mean
    /// nothing, yet a password among them went over the wire as it was.
    fn discard(&mut self) {
        for line in self.reader.discard() {
            self.hear(&line);
        }
    }

    /// Records the lines a client sent in clear between the STARTTLS answer and its handshake, unanswered.
    pub(crate) fn heard_in_clear(&mut self, lines: Vec<Vec<u8>>) {
        for line in lines {
            self.transcript.push(String::from_utf8_lossy(&line).into_owned(), false);
        }
    }

    /// Records the first [`crate::MAX_LINE`] bytes of a line that is too long and says goodbye.
    fn cut(&mut self, io: &mut impl Write, cut: &[u8]) -> Result<Effect, Error> {
        self.hear(cut);
        send(io, b"421 4.7.0 Line too long\r\n")?;
        Err(Error::LineTooLong)
    }

    /// Says goodbye after too many refused commands; returns whether it did.
    fn too_bad(&self, io: &mut impl Write) -> io::Result<bool> {
        if self.bad < MAX_BAD {
            return Ok(false);
        }
        let mut out = Vec::new();
        self.reply(&mut out, 421, "4.7.0", "Too many errors");
        send(io, &out)?;
        Ok(true)
    }

    /// Plays the actions waiting for `when`.
    fn play(&mut self, io: &mut impl Write, when: &When, stop: &AtomicBool) -> Result<Played, Error> {
        let mut played = Played::Go;
        for action in self.fake.rules.take(when) {
            match action {
                Action::Fault(Fault::Drop) => return Ok(Played::End(Ending::Dropped)),
                Action::Fault(Fault::Delay(wait)) => {
                    if pause(wait, stop) {
                        return Ok(Played::End(Ending::Dropped));
                    }
                }
                Action::Fault(Fault::Garbage(bytes)) => send(io, &bytes)?,
                Action::Send(line) => send(io, format!("{}\r\n", clean(&line)).as_bytes())?,
                Action::Reply(reply) => {
                    if *when == When::Greeting {
                        self.serving = reply.positive();
                    }
                    let mut out = Vec::new();
                    reply.write(&mut out, self.fake.offers(Extension::EnhancedStatusCodes));
                    send(io, &out)?;
                    played = Played::Replied;
                }
            }
        }
        Ok(played)
    }

    /// Answers one command into `out`.
    fn answer(&mut self, command: Command, out: &mut Vec<u8>) -> Next {
        match command {
            Command::Ehlo(domain) => self.ehlo(&domain, out),
            Command::Helo(domain) => {
                self.hello = true;
                self.mail = None;
                write_reply(out, 250, None, &[&format!("{DOMAIN} greets {domain}")]);
            }
            Command::StartTls => return self.starttls(out),
            Command::Auth { mechanism, initial } => return self.start_auth(&mechanism, initial.as_deref(), out),
            Command::Mail { path, params } => self.mail_from(path, params, out),
            Command::Rcpt { path, params } => self.rcpt_to(path, &params, out),
            Command::Data => return self.start_data(out),
            Command::Rset => {
                self.mail = None;
                self.reply(out, 250, "2.0.0", "Reset");
            }
            Command::Noop => self.reply(out, 250, "2.0.0", "OK"),
            Command::Quit => {
                self.reply(out, 221, "2.0.0", "Bye");
                return Next::Quit;
            }
        }
        Next::Go
    }

    fn refused(&mut self, refused: Refused, out: &mut Vec<u8>) {
        match refused {
            Refused::Unknown => self.refuse(out, 500, "5.5.2", "Command not recognised"),
            Refused::Syntax(reason) => self.refuse(out, 501, "5.5.4", reason),
        }
    }

    /// EHLO: the greeting line and the extensions listed now.
    fn ehlo(&mut self, domain: &str, out: &mut Vec<u8>) {
        self.hello = true;
        self.mail = None;
        let mut lines = vec![format!("{DOMAIN} greets {domain}")];
        for extension in Extension::ALL.into_iter().filter(|&extension| self.listed(extension)) {
            match extension {
                Extension::Size => lines.push(format!("SIZE {}", self.fake.max_size)),
                Extension::AuthPlain | Extension::AuthLogin => {}
                _ => lines.push(extension.keyword().to_owned()),
            }
        }
        let mechanisms: Vec<&str> = [(Extension::AuthPlain, "PLAIN"), (Extension::AuthLogin, "LOGIN")]
            .into_iter()
            .filter(|&(extension, _)| self.listed(extension))
            .map(|(_, name)| name)
            .collect();
        if !mechanisms.is_empty() {
            lines.push(format!("AUTH {}", mechanisms.join(" ")));
        }
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        write_reply(out, 250, None, &lines);
    }

    /// Whether `extension` is listed now.
    fn listed(&self, extension: Extension) -> bool {
        self.fake.offers(extension)
            && match extension {
                Extension::StartTls => !self.tls && self.fake.starttls.is_some(),
                Extension::AuthPlain | Extension::AuthLogin => self.sign_in_allowed(),
                _ => true,
            }
    }

    fn sign_in_allowed(&self) -> bool {
        self.tls || self.fake.auth_in_clear
    }

    fn starttls(&mut self, out: &mut Vec<u8>) -> Next {
        if self.tls {
            self.refuse(out, 503, "5.5.1", "TLS is already active");
            return Next::Go;
        }
        let Some(config) = self.fake.starttls.clone().filter(|_| self.listed(Extension::StartTls)) else {
            self.refuse(out, 502, "5.5.1", "STARTTLS is not available");
            return Next::Go;
        };
        self.reply(out, 220, "2.0.0", "Ready to start TLS");
        Next::StartTls(config)
    }

    fn start_auth(&mut self, mechanism: &str, initial: Option<&str>, out: &mut Vec<u8>) -> Next {
        if !self.hello {
            self.refuse(out, 503, "5.5.1", "Say EHLO first");
        } else if self.user.is_some() {
            self.refuse(out, 503, "5.5.1", "Already signed in");
        } else if self.mail.is_some() {
            self.refuse(out, 503, "5.5.1", "AUTH is not allowed in a mail transaction");
        } else if !self.sign_in_allowed() {
            self.refuse(out, 530, "5.7.0", "Must issue a STARTTLS command first");
        } else {
            match (mechanism, initial) {
                ("PLAIN", None) if self.fake.offers(Extension::AuthPlain) => {
                    out.extend_from_slice(b"334 \r\n");
                    return Next::Auth(Step::Plain);
                }
                ("PLAIN", Some(response)) if self.fake.offers(Extension::AuthPlain) => self.plain(response.as_bytes(), out),
                ("LOGIN", None) if self.fake.offers(Extension::AuthLogin) => {
                    write_reply(out, 334, None, &[USERNAME]);
                    return Next::Auth(Step::User);
                }
                ("LOGIN", Some(user)) if self.fake.offers(Extension::AuthLogin) => {
                    let Some(user) = self.decode(user.as_bytes(), out) else { return Next::Go };
                    write_reply(out, 334, None, &[PASSWORD]);
                    return Next::Auth(Step::Password(user));
                }
                _ => self.refuse(out, 504, "5.5.4", "Unrecognised authentication type"),
            }
        }
        Next::Go
    }

    /// Reads the AUTH responses after a challenge, until the exchange ends; returns how the session ended when the
    /// client went.
    fn auth(&mut self, io: &mut (impl Read + Write), mut step: Step) -> Result<Option<Ending>, Error> {
        loop {
            let response = match self.reader.read_line(io)? {
                Line::Complete(response) => response,
                Line::Eof => return Ok(Some(Ending::Dropped)),
                Line::TooLong(cut) => return self.cut(io, &cut).map(|_| None),
            };
            self.hear(&response);
            let mut out = Vec::new();
            step = match step {
                Step::Plain => {
                    self.plain(&response, &mut out);
                    send(io, &out)?;
                    return Ok(None);
                }
                Step::User => {
                    let Some(user) = self.decode(&response, &mut out) else {
                        send(io, &out)?;
                        return Ok(None);
                    };
                    write_reply(&mut out, 334, None, &[PASSWORD]);
                    send(io, &out)?;
                    Step::Password(user)
                }
                Step::Password(user) => {
                    if let Some(password) = self.decode(&response, &mut out) {
                        let good = user == self.fake.user.as_bytes() && password == self.fake.password.as_bytes();
                        self.sign_in(good, &user, &mut out);
                    }
                    send(io, &out)?;
                    return Ok(None);
                }
            };
        }
    }

    /// Decodes an AUTH response: `*` cancels, `=` is empty (RFC 4954); refuses into `out` and gives `None` otherwise.
    fn decode(&mut self, response: &[u8], out: &mut Vec<u8>) -> Option<Vec<u8>> {
        if response == b"*" {
            self.refuse(out, 501, "5.7.0", "Authentication cancelled");
            return None;
        }
        let decoded = if response == b"=" { Some(Vec::new()) } else { base64::decode(response) };
        if decoded.is_none() {
            self.refuse(out, 501, "5.5.2", "Cannot decode the response");
        }
        decoded
    }

    /// Checks an AUTH PLAIN response (RFC 4616).
    fn plain(&mut self, response: &[u8], out: &mut Vec<u8>) {
        let Some(decoded) = self.decode(response, out) else { return };
        let parts: Vec<&[u8]> = decoded.split(|&byte| byte == 0).collect();
        let (good, user) = match parts.as_slice() {
            [authz, user, password] => {
                ((authz.is_empty() || authz == user) && *user == self.fake.user.as_bytes() && *password == self.fake.password.as_bytes(), *user)
            }
            _ => (false, &[][..]),
        };
        self.sign_in(good, user, out);
    }

    fn sign_in(&mut self, good: bool, user: &[u8], out: &mut Vec<u8>) {
        if !good {
            return self.reply(out, 535, "5.7.8", "Authentication credentials invalid");
        }
        self.user = Some(String::from_utf8_lossy(user).into_owned());
        self.reply(out, 235, "2.7.0", "Authentication successful");
    }

    fn mail_from(&mut self, from: String, params: Vec<String>, out: &mut Vec<u8>) {
        let signs_in = self.fake.offers(Extension::AuthPlain) || self.fake.offers(Extension::AuthLogin);
        if !self.hello {
            return self.refuse(out, 503, "5.5.1", "Say EHLO first");
        }
        if signs_in && self.user.is_none() {
            return self.refuse(out, 530, "5.7.0", "Authentication required");
        }
        if self.mail.is_some() {
            return self.refuse(out, 503, "5.5.1", "Nested MAIL command");
        }
        let mut utf8 = false;
        for param in &params {
            let (key, value) = param.split_once('=').map_or((param.as_str(), None), |(key, value)| (key, Some(value)));
            let key = key.to_ascii_uppercase();
            let refusal = match (key.as_str(), value) {
                ("SIZE", _) if !self.fake.offers(Extension::Size) => Some((555, "5.5.4", "SIZE is not offered")),
                ("SIZE", Some(size)) => match size.parse::<usize>() {
                    Ok(size) if size > self.fake.max_size => Some((552, "5.3.4", "Message size exceeds fixed maximum message size")),
                    Ok(_) => None,
                    Err(_) => Some((501, "5.5.4", "SIZE needs a number")),
                },
                ("BODY", _) if !self.fake.offers(Extension::EightBitMime) => Some((555, "5.5.4", "BODY is not offered")),
                ("BODY", Some(body)) if body.eq_ignore_ascii_case("7BIT") || body.eq_ignore_ascii_case("8BITMIME") => None,
                ("SMTPUTF8", None) if self.fake.offers(Extension::SmtpUtf8) => {
                    utf8 = true;
                    None
                }
                ("SIZE" | "BODY", _) => Some((501, "5.5.4", "The parameter's value cannot be read")),
                _ => Some((555, "5.5.4", "Parameter not recognised")),
            };
            if let Some((code, enhanced, text)) = refusal {
                return self.refuse(out, code, enhanced, text);
            }
        }
        if !utf8 && !from.is_ascii() {
            return self.refuse(out, 553, "5.6.7", "The address is not ASCII and SMTPUTF8 was not asked for");
        }
        self.mail = Some(Transaction { from, params, recipients: Vec::new() });
        self.reply(out, 250, "2.1.0", "Sender OK");
    }

    fn rcpt_to(&mut self, to: String, params: &[String], out: &mut Vec<u8>) {
        let Some(mail) = &self.mail else { return self.refuse(out, 503, "5.5.1", "Need MAIL first") };
        let utf8 = mail.params.iter().any(|param| param.eq_ignore_ascii_case("SMTPUTF8"));
        let refusal = if !params.is_empty() {
            Some((555, "5.5.4", "Parameter not recognised"))
        } else if to.is_empty() {
            Some((501, "5.1.3", "Bad recipient address"))
        } else if !utf8 && !to.is_ascii() {
            Some((553, "5.6.7", "The address is not ASCII and SMTPUTF8 was not asked for"))
        } else if mail.recipients.len() >= MAX_RECIPIENTS {
            Some((452, "4.5.3", "Too many recipients"))
        } else {
            None
        };
        if let Some((code, enhanced, text)) = refusal {
            return self.refuse(out, code, enhanced, text);
        }
        if let Some(mail) = &mut self.mail {
            mail.recipients.push(to);
        }
        self.reply(out, 250, "2.1.5", "Recipient OK");
    }

    fn start_data(&mut self, out: &mut Vec<u8>) -> Next {
        match &self.mail {
            None => self.refuse(out, 503, "5.5.1", "Need MAIL first"),
            Some(mail) if mail.recipients.is_empty() => self.refuse(out, 554, "5.5.1", "No valid recipients"),
            Some(_) => {
                write_reply(out, 354, None, &["End data with <CR><LF>.<CR><LF>"]);
                return Next::Data;
            }
        }
        Next::Go
    }

    /// Reads the message up to its final dot, undoing the dot-stuffing, and answers it; returns how the session ended
    /// when it did. A message over the size limit is read to its end but not kept.
    fn data(&mut self, io: &mut (impl Read + Write), stop: &AtomicBool) -> Result<Option<Ending>, Error> {
        let mut data = Vec::new();
        let mut too_big = false;
        loop {
            let line = match self.reader.read_line(io)? {
                Line::Complete(line) => line,
                Line::Eof => return Ok(Some(Ending::Dropped)),
                Line::TooLong(cut) => return self.cut(io, &cut).map(|_| None),
            };
            self.hear(&line);
            if line == b"." {
                break;
            }
            let line = line.strip_prefix(b".").unwrap_or(&line);
            if too_big || data.len().saturating_add(line.len()).saturating_add(2) > self.fake.max_size {
                too_big = true;
                data = Vec::new();
                continue;
            }
            data.extend_from_slice(line);
            data.extend_from_slice(b"\r\n");
        }
        let Some(mail) = self.mail.take() else { return Ok(None) };
        let mut out = Vec::new();
        if too_big {
            self.reply(&mut out, 552, "5.3.4", "Message too big");
            send(io, &out)?;
            return Ok(None);
        }
        match self.play(io, &When::Message, stop)? {
            Played::End(end) => return Ok(Some(end)),
            Played::Replied => return Ok(None),
            Played::Go => {}
        }
        let mut submission =
            Submission { id: 0, from: mail.from, params: mail.params, recipients: mail.recipients, data, user: self.user.clone(), tls: self.tls };
        submission.id = self.fake.outbox.capture(submission.clone());
        for recipient in &submission.recipients {
            for bounce in self.fake.bounces.iter().filter(|bounce| bounce.matches(recipient)) {
                bounce.report(&submission);
            }
        }
        self.reply(&mut out, 250, "2.0.0", &format!("Queued as {}", submission.id));
        send(io, &out)?;
        Ok(None)
    }

    /// Adds a one-line reply to `out`, with its enhanced code when the server offers them.
    fn reply(&self, out: &mut Vec<u8>, code: u16, enhanced: &str, text: &str) {
        write_reply(out, code, self.fake.offers(Extension::EnhancedStatusCodes).then_some(enhanced), &[text]);
    }

    /// Adds a refusal to `out` and counts it.
    fn refuse(&mut self, out: &mut Vec<u8>, code: u16, enhanced: &str, text: &str) {
        self.bad = self.bad.saturating_add(1);
        self.reply(out, code, enhanced, text);
    }
}

fn send(io: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    io.write_all(bytes)?;
    io.flush()
}
