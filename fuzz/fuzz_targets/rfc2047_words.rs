// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! RFC 2047 encoded words on any text: reading never panics, and any text written as encoded words reads back as
//! itself, without CR, LF and NUL.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_charset::{MAX_WORD_LEN, decode_text, decode_word, encode_text};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = decode_word(&text);
    let _ = decode_text(&text);

    let written = encode_text(&text);
    assert!(!written.contains(['\r', '\n', '\0']), "{written:?}");
    for word in written.split([' ', '\t']).filter(|part| part.starts_with("=?UTF-8?")) {
        assert!(word.len() <= MAX_WORD_LEN, "{word:?}");
    }
    let read = decode_text(&written);
    assert!(!read.malformed, "{written:?}");
    assert_eq!(read.text, text.replace(['\r', '\n', '\0'], ""), "{written:?}");
});
