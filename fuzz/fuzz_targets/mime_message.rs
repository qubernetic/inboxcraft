// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The message reader on any bytes: reading, and everything asked of every part, never panics; a message that
//! reads has at most `MAX_PARTS` parts, and the header text for people holds no control character.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_mime::{MAX_PARTS, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(message) = parse(data) else { return };
    let parts = message.parts();
    assert!(parts.len() <= MAX_PARTS);
    for part in parts {
        let _ = part.decoded();
        let _ = part.text();
        let _ = part.delivery_status();
        let _ = part.is_attachment();
        for text in [part.file_name(), part.content_id(), part.headers().text("subject")].into_iter().flatten() {
            assert!(!text.chars().any(char::is_control), "{text:?}");
        }
    }
    let _ = message.attachments();
    let _ = message.text_body();
    let _ = message.html_body();
});
