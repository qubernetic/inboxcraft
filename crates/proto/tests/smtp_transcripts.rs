// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Golden SMTP transcripts: every file in `tests/transcripts/smtp/` is a session with invented data, played against
//! the SMTP client.
//!
//! One directive per line, as in `transcripts.rs`:
//!
//! - `# ...` a comment; an empty line is nothing.
//! - `@ tls` or `@ plain`: how the connection started (the first directive).
//! - `S: <line>`: the server sends the line and CRLF; consecutive `S:` lines arrive in one piece.
//! - `C: <line>`: the next line the client wrote must be this one.
//! - `> <command> [arguments]`: a call on the client (see [`call`]).
//! - `= <line>`: the next line of the events, as [`render`] writes them, must be this one.
//! - `! <error>`: the last call or the last bytes received failed with this error (`Debug` form).
//! - `? <state>`: the session is in this state (`Debug` form).
//!
//! At the end nothing may be left: no output, no event and no error that a `C:`, `=` or `!` line did not check.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::VecDeque;
use std::path::Path;

use inboxcraft_proto::ProtoError;
use inboxcraft_proto::smtp::{Answer, Client, Event, MailOptions, Transport};

/// `\n`, `\r`, `\0`, `\\` and `\xHH` written out: the message text of `> data`.
fn unescape(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut bytes = text.bytes();
    while let Some(b) = bytes.next() {
        if b != b'\\' {
            out.push(b);
            continue;
        }
        match bytes.next().unwrap() {
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b'0' => out.push(0),
            b'\\' => out.push(b'\\'),
            b'x' => {
                let hex: Vec<u8> = bytes.by_ref().take(2).collect();
                out.push(u8::from_str_radix(std::str::from_utf8(&hex).unwrap(), 16).unwrap());
            }
            other => panic!("unknown escape \\{}", other as char),
        }
    }
    out
}

fn text(escaped: &str) -> String {
    String::from_utf8(unescape(escaped)).unwrap()
}

/// Calls: `ehlo <domain>`, `starttls`, `tls`, `auth-plain <user> <password>`, `auth-login <user> <password>`,
/// `mail <from> [size=<n>] [8bit] [utf8]` (`-` for the null path), `rcpt <to>`, `data <text>`, `rset`, `quit`.
fn call(client: &mut Client, line: &str) -> Result<(), ProtoError> {
    let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
    let mut words: VecDeque<&str> = rest.split(' ').filter(|w| !w.is_empty()).collect();
    match command {
        "ehlo" => client.ehlo(&text(rest)),
        "starttls" => client.starttls(),
        "tls" => client.tls_established(),
        "auth-plain" => client.auth_plain(words[0], &text(words[1])),
        "auth-login" => client.auth_login(words[0], &text(words[1])),
        "mail" => {
            let from = words.pop_front().unwrap();
            let mut options = MailOptions::default();
            for word in words {
                match word.split_once('=') {
                    Some(("size", n)) => options.size = Some(n.parse().unwrap()),
                    _ if word == "8bit" => options.eight_bit = true,
                    _ if word == "utf8" => options.utf8 = true,
                    _ => panic!("unknown mail option {word}"),
                }
            }
            client.mail(&if from == "-" { String::new() } else { text(from) }, options)
        }
        "rcpt" => client.rcpt(&text(rest)),
        "data" => client.data(&unescape(rest)),
        "rset" => client.rset(),
        "quit" => client.quit(),
        other => panic!("unknown call {other}"),
    }
}

fn render(event: &Event) -> String {
    match event {
        Event::Greeted => "greeted".to_owned(),
        Event::Completed { command, outcome: Ok(answer) } => {
            let answer = match answer {
                Answer::Hello(extensions) => format!("hello {}", extensions.keywords().collect::<Vec<_>>().join(" ")),
                Answer::StartTls => "go ahead".to_owned(),
                Answer::SignedIn => "signed in".to_owned(),
                Answer::MailAccepted => "sender accepted".to_owned(),
                Answer::RecipientAccepted => "recipient accepted".to_owned(),
                Answer::Sent(text) => format!("sent {text}"),
                Answer::Reset => "reset".to_owned(),
                Answer::Quit => "bye".to_owned(),
            };
            format!("{command} {}", answer.trim_end())
        }
        Event::Completed { command, outcome: Err(refusal) } => {
            let enhanced = refusal.enhanced.map(|code| format!(" {code}")).unwrap_or_default();
            let kind = if refusal.is_permanent() { "refused" } else { "refused for now" };
            format!("{command} {kind} {}{enhanced} {}", refusal.code, refusal.text).trim_end().to_owned()
        }
    }
}

struct Session {
    client: Client,
    output: Vec<u8>,
    events: VecDeque<String>,
    error: Option<ProtoError>,
    incoming: Vec<u8>,
}

impl Session {
    fn collect(&mut self) {
        self.output.extend(self.client.take_output());
        while let Some(event) = self.client.next_event() {
            self.events.push_back(render(&event));
        }
    }

    fn flush(&mut self, at: &str) {
        if !self.incoming.is_empty() {
            assert!(self.error.is_none(), "{at}: unchecked error {:?}", self.error);
            let incoming = std::mem::take(&mut self.incoming);
            self.error = self.client.receive(&incoming).err();
        }
        self.collect();
    }

    fn output_line(&mut self, at: &str) -> String {
        let end = self
            .output
            .windows(2)
            .position(|w| w == b"\r\n")
            .unwrap_or_else(|| panic!("{at}: no line written, have {:?}", self.output.escape_ascii().to_string()));
        let line: Vec<u8> = self.output.drain(..end + 2).take(end).collect();
        line.escape_ascii().to_string()
    }
}

fn play(path: &Path) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path).unwrap();
    let mut session: Option<Session> = None;
    for (n, line) in text.lines().enumerate() {
        let at = format!("{name}:{}", n + 1);
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (directive, rest) = line.split_once(' ').unwrap_or((line, ""));
        if directive == "@" {
            let transport = match rest {
                "tls" => Transport::Tls,
                "plain" => Transport::Plain,
                other => panic!("{at}: unknown transport {other}"),
            };
            if let Some(done) = session.take() {
                finish(done, &at);
            }
            session =
                Some(Session { client: Client::new(transport), output: Vec::new(), events: VecDeque::new(), error: None, incoming: Vec::new() });
            continue;
        }
        let session = session.as_mut().unwrap_or_else(|| panic!("{at}: no `@` line first"));
        if directive == "S:" {
            session.incoming.extend_from_slice(&unescape(rest));
            session.incoming.extend_from_slice(b"\r\n");
            continue;
        }
        session.flush(&at);
        match directive {
            "C:" => assert_eq!(session.output_line(&at), rest, "{at}"),
            "=" => assert_eq!(session.events.pop_front().as_deref(), Some(rest), "{at}"),
            "!" => assert_eq!(session.error.take().map(|e| format!("{e:?}")).as_deref(), Some(rest), "{at}"),
            "?" => assert_eq!(format!("{:?}", session.client.state()), rest, "{at}"),
            ">" => {
                assert!(session.error.is_none(), "{at}: unchecked error {:?}", session.error);
                session.error = call(&mut session.client, rest).err();
                session.collect();
            }
            other => panic!("{at}: unknown directive {other}"),
        }
    }
    finish(session.unwrap_or_else(|| panic!("{name}: empty")), &format!("{name}:end"));
}

/// Checks that a session left nothing unchecked.
fn finish(mut session: Session, at: &str) {
    session.flush(at);
    assert!(session.error.is_none(), "{at}: unchecked error {:?}", session.error);
    assert!(session.events.is_empty(), "{at}: unchecked events {:?}", session.events);
    assert!(session.output.is_empty(), "{at}: unchecked output {:?}", session.output.escape_ascii().to_string());
}

#[test]
fn every_smtp_transcript_plays() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/transcripts/smtp");
    let mut paths: Vec<_> =
        std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect();
    paths.sort();
    assert!(paths.len() >= 8, "{} transcripts", paths.len());
    for path in &paths {
        play(path);
    }
}
