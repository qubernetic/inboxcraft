// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Charset decoding on any label and any bytes: it never panics, and UTF-8 that is not broken decodes as itself.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_charset::{Charset, decode};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The first line is the label, the rest the bytes.
    let (label, bytes) = match data.iter().position(|&b| b == b'\n') {
        Some(at) => (data.get(..at).unwrap_or_default(), data.get(at + 1..).unwrap_or_default()),
        None => (data, &[][..]),
    };
    let _ = decode(&String::from_utf8_lossy(label), bytes);
    if let Ok(text) = std::str::from_utf8(data) {
        let decoded = Charset::UTF_8.decode(data);
        assert!(!decoded.malformed);
        assert_eq!(decoded.text, text.strip_prefix('\u{feff}').unwrap_or(text));
    }
});
