// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The envelope of any header block, and address lists and message ids of any text: none panics, every address
//! list stays within the cap, and no name or subject holds a control character.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_model::MAX_ADDRESSES;
use inboxcraft_net::envelope::{addresses, envelope, message_refs};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let envelope = envelope(data);
    assert!(!envelope.subject().chars().any(char::is_control), "{:?}", envelope.subject());
    for list in [envelope.from(), envelope.to(), envelope.cc(), envelope.bcc(), envelope.reply_to()] {
        assert!(list.len() <= MAX_ADDRESSES);
        for address in list {
            assert!(!address.name().unwrap_or_default().chars().any(char::is_control), "{address:?}");
        }
    }
    assert!(!envelope.signs().calendar(), "a header block knows no parts");
    let text = String::from_utf8_lossy(data);
    assert!(addresses(&text).len() <= MAX_ADDRESSES);
    let _ = message_refs(&text);
});
