// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake SMTP server's own command reader and session on any bytes from the client, before and after EHLO and
//! the sign-in: it never panics, greets first, and ends every line it sends with CRLF.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_testkit::imap::FakeImap;
use inboxcraft_testkit::smtp::{Bounce, FakeSmtp};
use libfuzzer_sys::fuzz_target;

/// EHLO and AUTH PLAIN with the fake's user and password.
const SIGN_IN: &[u8] = b"EHLO c.example.org\r\nAUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk\r\n";

fuzz_target!(|data: &[u8]| {
    let imap = FakeImap::new();
    let fake = FakeSmtp::new().max_size(4096).bounce(Bounce::new("nobody@example.org", &imap.postbox()));
    let signed_in = [SIGN_IN, data].concat();
    for input in [data, &signed_in] {
        let out = fake.answer(input);
        assert!(out.starts_with(b"220 "), "{:?}", String::from_utf8_lossy(&out));
        assert!(out.ends_with(b"\r\n"), "{:?}", String::from_utf8_lossy(&out));
    }
    let _ = imap.answer(b"a1 LOGIN ada@example.org not-a-real-password\r\na2 SELECT INBOX\r\na3 UID FETCH 1:* BODY[]\r\n");
});
