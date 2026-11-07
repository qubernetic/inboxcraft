// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The DAV XML reader and the multistatus reader on any bytes: they never panic, stay within their caps, and every
//! entry they keep is a path on the origin.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_dav::xml::{self, MAX_DEPTH, Node};
use inboxcraft_dav::{Origin, multistatus};
use libfuzzer_sys::fuzz_target;

fn depth(element: &xml::Element) -> usize {
    1 + element.children.iter().filter_map(|node| if let Node::Element(child) = node { Some(depth(child)) } else { None }).max().unwrap_or(0)
}

fuzz_target!(|data: &[u8]| {
    if let Ok(root) = xml::parse(data) {
        assert!(depth(&root) <= MAX_DEPTH);
        let written = xml::escape(&root.text());
        assert!(!written.contains('<'));
    }
    let Ok((origin, _)) = Origin::parse("https://dav.example.org/") else { return };
    if let Ok(answer) = multistatus::read(&origin, "/calendars/ada/work/", data) {
        assert!(answer.entries.iter().all(|entry| entry.href.starts_with('/') && !entry.href.chars().any(char::is_control)));
        assert!(answer.sync_token.iter().all(|token| token.len() <= multistatus::MAX_TOKEN));
    }
});
