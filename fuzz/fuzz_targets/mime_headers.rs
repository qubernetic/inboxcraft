// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Header blocks and the structured values in them on any bytes: none panics, no field holds a line break once
//! unfolded, and the text of every field holds no control character and no invisible direction mark.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_mime::{ContentType, Disposition, Headers, Status, TransferEncoding};
use libfuzzer_sys::fuzz_target;

/// A character that text for people must not hold: a control character, or a mark that changes how text runs or
/// joins without showing itself.
fn is_hidden(c: char) -> bool {
    c.is_control() || matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

fuzz_target!(|data: &[u8]| {
    let Ok(headers) = Headers::parse(data) else { return };
    for field in headers.fields() {
        assert!(!field.value().contains(['\r', '\n']), "{:?}", field.value());
        if let Some(text) = headers.text(field.name()) {
            assert!(!text.chars().any(is_hidden), "{text:?}");
        }
        let _ = ContentType::parse(field.value()).map(|content_type| content_type.params().text("name"));
        let _ = Disposition::parse(field.value()).file_name();
        let _ = TransferEncoding::parse(field.value());
        let _ = Status::parse(field.value());
    }
});
