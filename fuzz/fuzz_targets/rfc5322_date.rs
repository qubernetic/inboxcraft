// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The RFC 5322 date parser on any bytes: it never panics, and a date it reads formats to text it reads back as the
//! same date.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_time::MailDate;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(date) = MailDate::parse(&text) {
        let formatted = date.to_string();
        assert!(formatted.is_ascii(), "{formatted:?}");
        assert_eq!(MailDate::parse(&formatted), Ok(date), "{text:?} formatted as {formatted:?}");
    }
});
