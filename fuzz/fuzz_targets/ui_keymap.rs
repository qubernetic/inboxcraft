// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The keys' file on any bytes: a key text read as one ([`read`]) writes back to the same key, and a keymap read from
//! any JSON ([`Keymap::from_value`]) gives every key to one action at most and writes back to itself.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_ui_egui::keymap::{Keymap, actions, read, written};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Some(shortcut) = read(&text) {
        assert_eq!(read(&written(shortcut)), Some(shortcut), "{text:?}");
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else { return };
    let keymap = Keymap::from_value(&value);
    let keys: Vec<_> = actions().into_iter().filter_map(|action| keymap.shortcut(action.id)).collect();
    for (at, key) in keys.iter().enumerate() {
        assert!(!keys.iter().skip(at + 1).any(|other| other == key), "one key runs one action");
    }
    assert_eq!(Keymap::from_value(&keymap.to_value()), keymap);
});
