// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The time picker's typed time on any bytes, at any now and offset: it never panics, and a time it reads is ahead of
//! now and formats to text.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_time::{Offset, Timestamp};
use inboxcraft_ui_egui::component::typed_time::{long, parse, short};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&[s0, s1, s2, s3, s4, s5, s6, s7, o0, o1], tail)) = data.split_first_chunk::<10>() else { return };
    let (first, last) = (Timestamp::MIN.unix_seconds(), Timestamp::MAX.unix_seconds());
    let seconds = first.saturating_add(i64::from_le_bytes([s0, s1, s2, s3, s4, s5, s6, s7]).rem_euclid(last.saturating_sub(first)));
    let now = Timestamp::from_unix(seconds, 0).unwrap_or(Timestamp::UNIX_EPOCH);
    let offset = Offset::from_minutes(i16::from_le_bytes([o0, o1]) % 6000).unwrap_or(Offset::UTC);
    let text = String::from_utf8_lossy(tail);
    if let Some(at) = parse(&text, now, offset) {
        assert!(at > now, "{text:?} at {now:?} gave {at:?}");
        let _ = (long(at, now, offset), short(at, now, offset));
    }
});
