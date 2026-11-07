// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The model's checks of text from outside (addresses, display names, flags, hosts, folder names, message ids) on any
//! text: none panics, whatever is accepted holds no CR, LF or NUL, and no accepted address holds an invisible mark.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_model::{
    Address, EmailAddress, Envelope, Flag, FolderListing, FolderPath, Host, MessageRef, Signs, detect_role, labels,
};
use libfuzzer_sys::fuzz_target;

fn clean(text: &str) -> bool {
    !text.contains(['\r', '\n', '\0'])
}

/// Whether `text` holds none of the invisible direction and zero-width marks.
fn visible(text: &str) -> bool {
    !text.chars().any(|c| matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}'))
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let (head, tail) = text.split_at(text.char_indices().nth(text.chars().count() / 2).map_or(0, |(at, _)| at));

    if let Ok(email) = EmailAddress::new(&text) {
        assert!(clean(email.as_str()), "{email:?}");
        assert!(visible(email.as_str()), "{email:?}");
        let address = Address::new(Some(head), email);
        assert!(address.name().is_none_or(clean), "{address:?}");
        for label in labels(&[address.clone(), address.with_name(Some(tail))]) {
            assert!(clean(&label), "{label:?}");
        }
    }
    if let Ok(flag) = Flag::parse(&text) {
        assert!(clean(&flag.to_string()), "{flag:?}");
    }
    if let Ok(host) = Host::new(&text) {
        assert!(clean(host.as_str()), "{host:?}");
    }
    if let Some(reference) = MessageRef::new(&text) {
        assert!(clean(reference.as_str()), "{reference:?}");
    }
    assert!(clean(Envelope::new(&text).subject()));
    let signs = Signs::read([(head, tail), ("Precedence", tail), ("Auto-Submitted", head), ("Content-Type", text.as_ref())]);
    assert!(!signs.calendar(), "a header knows no parts");
    let delimiter = tail.chars().next();
    if let Ok(path) = FolderPath::new(head, delimiter) {
        assert!(path.components().count() <= head.len().saturating_add(1));
        let _ = detect_role(&FolderListing::new(path, Vec::new(), true));
    }
});
