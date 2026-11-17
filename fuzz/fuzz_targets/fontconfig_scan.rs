// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A fontconfig file on any bytes ([`parse`]): it never panics and never overflows the stack, whatever the nesting,
//! comments, instructions or declarations; what it names is only folders, includes and a family, and touches no file.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::Path;

use libfuzzer_sys::fuzz_target;

/// The desktop app's bytes-level fontconfig reader, as it is: it needs `std` and `roxmltree` only, so the target
/// builds without the app's window stack.
#[path = "../../apps/inboxcraft/src/fontconfig/xml.rs"]
mod xml;

use xml::{Named, Places, depth_of, parse, plain};

fuzz_target!(|data: &[u8]| {
    // The count runs on the plain text without panicking either.
    if let Ok(text) = std::str::from_utf8(data)
        && let Some(plain) = plain(text)
    {
        let _ = depth_of(&plain);
    }
    let places = Places::default();
    if let Some(named) = parse(data, Some(Path::new("/etc/fonts")), &places) {
        for item in named {
            if let Named::Sans(family) = item {
                assert!(!family.is_empty() && family.trim() == family, "{family:?}");
            }
        }
    }
});
