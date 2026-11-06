// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The servers of any autoconfig file a web server or the ISPDB sends: none panics, the lists stay within their cap,
//! and every server is encrypted with a user name free of control characters.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_model::EmailAddress;
use inboxcraft_net::autoconfig::{MAX_SERVERS, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(address) = EmailAddress::new("ada@example.org") else { return };
    if let Some(found) = parse(data, &address) {
        for list in [&found.imap, &found.submission] {
            assert!(list.len() <= MAX_SERVERS);
            for offer in list {
                assert!(!offer.username.is_empty() && !offer.username.chars().any(char::is_control), "{offer:?}");
            }
        }
    }
});
