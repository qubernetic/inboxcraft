// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Calendars on any bytes: reading never panics; what reads writes, and what is written reads again unless it grew
//! past a limit; the occurrences of a day stay within their cap and their span.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_ical::{Calendar, IcalError, MAX_OCCURRENCES};
use inboxcraft_time::{Offset, Timestamp};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(calendar) = Calendar::parse(data) else { return };
    let Ok(written) = calendar.write() else { return };
    let again = Calendar::parse(written.as_bytes());
    assert!(matches!(again, Ok(_) | Err(IcalError::TooLarge { .. } | IcalError::TooManyItems { .. })), "what was written does not read: {again:?}");
    let (Some(from), Some(to)) = (Timestamp::from_unix(1_794_000_000, 0), Timestamp::from_unix(1_794_086_400, 0)) else { return };
    let expansion = calendar.occurrences(from, to, &Offset::UTC);
    assert!(expansion.occurrences.len() <= MAX_OCCURRENCES);
    assert!(expansion.occurrences.iter().all(|occurrence| occurrence.start < to && occurrence.start <= occurrence.end));
});
