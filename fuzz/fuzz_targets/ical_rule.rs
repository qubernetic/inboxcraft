// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Recurrence rules on any text: reading never panics, a rule that reads writes a value that reads back the same,
//! and its first occurrences rise from the start.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_ical::Rule;
use inboxcraft_time::CivilDateTime;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let Ok(rule) = Rule::parse(text) else { return };
    assert_eq!(Rule::parse(&rule.to_string()).as_ref(), Ok(&rule));
    let Some(start) = CivilDateTime::new(2026, 1, 31, 9, 30, 0) else { return };
    let occurrences: Vec<_> = rule.iter(start).take(50).collect();
    assert!(occurrences.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(occurrences.iter().all(|occurrence| *occurrence >= start));
});
