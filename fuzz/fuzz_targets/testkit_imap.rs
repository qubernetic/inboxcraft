// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server's own command reader and session on any bytes from the client, before and after a sign-in
//! and SELECT: it never panics, greets first, and ends every line it sends with CRLF.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_testkit::imap::{FakeImap, PASSWORD, USER};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let fake = FakeImap::sample();
    let signed_in = [format!("s1 LOGIN {USER} {PASSWORD}\r\ns2 SELECT INBOX\r\n").as_bytes(), data].concat();
    for input in [data, &signed_in] {
        let out = fake.answer(input);
        assert!(out.starts_with(b"* OK [CAPABILITY "), "{:?}", String::from_utf8_lossy(&out));
        assert!(out.ends_with(b"\r\n"), "{:?}", String::from_utf8_lossy(&out));
    }
});
