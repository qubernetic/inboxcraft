// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake CalDAV server's own request and XML readers and its session on any bytes from the client, signed in or
//! not: it never panics, and every answer it sends starts with a status line.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_testkit::dav::{FakeDav, WORK};
use libfuzzer_sys::fuzz_target;

/// The head of a PROPFIND with the fake's password, waiting for its body.
const SIGNED_IN: &[u8] = b"REPORT /calendars/ada/work/ HTTP/1.1\r\nAuthorization: Basic YWRhQGV4YW1wbGUub3JnOm5vdC1hLXJlYWwtcGFzc3dvcmQ=\r\nDepth: 0\r\n";

fuzz_target!(|data: &[u8]| {
    let fake = FakeDav::new().page(2);
    let _ = fake.calendars().put(&format!("{WORK}a.ics"), "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n");
    let with_body = [SIGNED_IN, format!("Content-Length: {}\r\n\r\n", data.len()).as_bytes(), data].concat();
    for input in [data, &with_body] {
        let out = fake.answer(input);
        assert!(out.is_empty() || out.starts_with(b"HTTP/1.1 "), "{:?}", String::from_utf8_lossy(&out));
    }
});
