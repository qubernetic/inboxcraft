// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The IMAP client and its response parser on any bytes, in any state: nothing panics, a receive that fails is fatal
//! and closes the session, a refused command is not, lists stay within their caps, server text is clean and short,
//! and once a password is given no error or event shows it.
//!
//! The input is pieces separated by 0xFF: the first byte of each picks a command to start before the rest of the
//! piece is received; the very first byte of the input also picks the transport and whether a valid greeting comes
//! first.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_model::{Uid, UidSet};
use inboxcraft_proto::imap::{
    Answer, Client, Event, FetchAttributes, MAX_FLAGS, MAX_MAILBOXES, MAX_TEXT_CHARS, SearchKey, Section, State, Transport,
};
use libfuzzer_sys::fuzz_target;

const PASSWORD: &str = "fuzz-secret-1";
/// `\0tim\0fuzz-secret-1` in base64.
const PLAIN: &str = "AHRpbQBmdXp6LXNlY3JldC0x";

fn clean(text: &str) -> bool {
    text.chars().count() <= MAX_TEXT_CHARS + 3 && !text.chars().any(char::is_control)
}

fn start(client: &mut Client, command: u8) -> Option<bool> {
    let one = UidSet::from_iter(Uid::new(1));
    let attributes = FetchAttributes { flags: true, internal_date: true, size: true, section: Some(Section::Full) };
    let result = match command % 12 {
        0 => client.capability(),
        1 => client.starttls(),
        2 => client.tls_established(),
        3 => client.login("tim", PASSWORD),
        4 => client.authenticate_plain("tim", PASSWORD),
        5 => client.list(),
        6 => client.select("INBOX"),
        7 => client.examine("Arch&AO0-vum"),
        8 => client.uid_fetch(&one, attributes),
        9 => client.uid_search(&SearchKey::Subject("Árvíz".into())),
        10 => client.logout(),
        _ => return None,
    };
    match result {
        Ok(_) => Some(matches!(command % 12, 3 | 4)),
        Err(error) => {
            assert!(!error.is_fatal() || client.state() == State::Closed, "{error:?}");
            None
        }
    }
}

fn check(event: &Event, given: bool) {
    match event {
        Event::Alert(text) => assert!(clean(text), "{text:?}"),
        Event::Completed { outcome: Err(refusal), .. } => assert!(clean(&refusal.text), "{refusal:?}"),
        Event::Completed { outcome: Ok(Answer::Listed(entries)), .. } => {
            assert!(entries.len() <= MAX_MAILBOXES);
            for entry in entries {
                assert!(!entry.raw.chars().any(char::is_control), "{entry:?}");
                assert!(entry.attributes.len() <= MAX_FLAGS);
            }
        }
        Event::Fetched(item) => assert!(item.flags.as_ref().is_none_or(|flags| flags.len() <= MAX_FLAGS)),
        _ => {}
    }
    if given && !matches!(event, Event::Fetched(_)) {
        let shown = format!("{event:?}");
        assert!(!shown.contains(PASSWORD) && !shown.contains(PLAIN), "{shown}");
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&first, rest)) = data.split_first() else { return };
    let mut client = Client::new(if first & 1 == 0 { Transport::Tls } else { Transport::Plain });
    if first & 2 == 0 {
        let _ = client.receive(b"* OK [CAPABILITY IMAP4rev1 STARTTLS AUTH=PLAIN SASL-IR LIST-EXTENDED SPECIAL-USE] hi\r\n");
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
                let shown = error.to_string();
                assert!(!shown.contains(PASSWORD) && !shown.contains(PLAIN), "{shown}");
            }
        }
        let _ = client.take_output();
        while let Some(event) = client.next_event() {
            check(&event, given);
        }
    }
    let shown = format!("{client:?}");
    assert!(!shown.contains(PASSWORD), "{shown}");
});
