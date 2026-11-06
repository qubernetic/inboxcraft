// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The parts of the client only visible from inside; the behaviour is tested through the public interface in
//! `tests/client.rs` and `tests/transcripts.rs`.

use super::{Client, Tag, Transport};

#[test]
fn debug_output_shows_no_password() {
    let mut client = Client::new(Transport::Tls);
    assert!(client.receive(b"* OK [CAPABILITY IMAP4rev1] ready\r\n").is_ok());
    assert!(client.login("ada@example.org", "hunter2-secret").is_ok());
    let shown = format!("{client:?}");
    assert!(shown.contains("A0001"), "{shown}");
    assert!(!shown.contains("hunter2"), "{shown}");
    assert!(!shown.contains("ada@"), "{shown}");
}

#[test]
fn tags_show_as_sent() {
    let tag = Tag("A0007".into());
    assert_eq!(tag.as_str(), "A0007");
    assert_eq!(tag.to_string(), "A0007");
}
