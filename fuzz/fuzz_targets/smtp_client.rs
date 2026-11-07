// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The SMTP client and its reply parser on any bytes, in any state: nothing panics, a receive that fails is fatal and
//! closes the session, a refused command is not, nothing is written before TLS that carries the password or the
//! message, server text is clean and short, and once a password is given no error, event or `Debug` output shows it.
//!
//! The input is pieces separated by 0xFF: the first byte of each picks a command to start before the rest of the
//! piece is received; the very first byte of the input also picks the transport and whether a valid greeting and
//! EHLO answer come first.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_proto::imap::MAX_TEXT_CHARS;
use inboxcraft_proto::smtp::{Answer, Client, Event, MAX_EXTENSIONS, MailOptions, State, Transport};
use libfuzzer_sys::fuzz_target;

const PASSWORD: &str = "fuzz-secret-1";
/// `\0tim\0fuzz-secret-1` and `fuzz-secret-1` in base64.
const ENCODED: [&str; 2] = ["AHRpbQBmdXp6LXNlY3JldC0x", "ZnV6ei1zZWNyZXQtMQ"];
const MESSAGE: &[u8] = b"Subject: private\r\n\r\n.only for bela\r\n";

fn clean(text: &str) -> bool {
    text.chars().count() <= MAX_TEXT_CHARS + 3 && !text.chars().any(char::is_control)
}

fn shows_secret(text: &str) -> bool {
    text.contains(PASSWORD) || ENCODED.iter().any(|encoded| text.contains(encoded))
}

/// Starts a command; `Some(true)` when it gave the password.
fn start(client: &mut Client, command: u8) -> Option<bool> {
    let result = match command % 14 {
        0 => client.ehlo("client.example.org"),
        1 => client.starttls(),
        2 => client.tls_established(),
        3 => client.auth_plain("tim", PASSWORD),
        4 => client.auth_login("tim", PASSWORD),
        5 => client.mail("tim@example.org", MailOptions { size: Some(40), eight_bit: true, utf8: false }),
        6 => client.mail("tím@example.org", MailOptions::default()),
        7 => client.rcpt("bela@example.org"),
        8 => client.data(MESSAGE),
        9 => client.data("árvíz".as_bytes()),
        10 => client.rset(),
        11 => client.quit(),
        12 => client.mail("", MailOptions { size: Some(u64::MAX), ..MailOptions::default() }),
        _ => return None,
    };
    match result {
        Ok(()) => Some(matches!(command % 14, 3 | 4)),
        Err(error) => {
            assert!(!error.is_fatal() || client.state() == State::Closed, "{error:?}");
            None
        }
    }
}

fn check(event: &Event, given: bool) {
    match event {
        Event::Completed { outcome: Err(refusal), .. } => {
            assert!(clean(&refusal.text), "{refusal:?}");
            assert!((400..600).contains(&refusal.code), "{refusal:?}");
            assert!(refusal.enhanced.is_none_or(|code| u16::from(code.class()) == refusal.code / 100), "{refusal:?}");
        }
        Event::Completed { outcome: Ok(Answer::Sent(text)), .. } => assert!(clean(text), "{text:?}"),
        Event::Completed { outcome: Ok(Answer::Hello(extensions)), .. } => {
            assert!(extensions.keywords().count() <= MAX_EXTENSIONS);
            assert!(extensions.keywords().all(|keyword| !keyword.is_empty() && keyword.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')));
        }
        _ => {}
    }
    if given {
        let shown = format!("{event:?}");
        assert!(!shows_secret(&shown), "{shown}");
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&first, rest)) = data.split_first() else { return };
    let mut client = Client::new(if first & 1 == 0 { Transport::Tls } else { Transport::Plain });
    if first & 2 == 0 {
        let _ = client.receive(b"220 smtp.example.org ESMTP\r\n");
        let _ = client.ehlo("client.example.org");
        let _ = client.receive(b"250-smtp.example.org\r\n250-SIZE 100\r\n250-8BITMIME\r\n250-ENHANCEDSTATUSCODES\r\n250-STARTTLS\r\n250 AUTH PLAIN LOGIN\r\n");
    }
    let mut given = false;
    for piece in rest.split(|b| *b == 0xFF) {
        let Some((&command, bytes)) = piece.split_first() else { continue };
        given |= start(&mut client, command).unwrap_or(false);
        let received = client.receive(bytes);
        if let Err(error) = &received {
            assert!(error.is_fatal(), "{error:?}");
            assert_eq!(client.state(), State::Closed);
            if given {
                assert!(!shows_secret(&error.to_string()), "{error}");
            }
        }
        let output = client.take_output();
        if !client.is_encrypted() {
            let written = String::from_utf8_lossy(&output);
            assert!(!shows_secret(&written) && !written.contains("only for bela"), "{written}");
        }
        while let Some(event) = client.next_event() {
            check(&event, given);
        }
    }
    let shown = format!("{client:?}");
    assert!(!shows_secret(&shown), "{shown}");
});
