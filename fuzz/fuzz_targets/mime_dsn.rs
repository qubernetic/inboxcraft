// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Delivery reports on any bytes: reading never panics, and a report that reads names between one and
//! `MAX_RECIPIENTS` recipients.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_mime::{DeliveryStatus, MAX_RECIPIENTS};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(report) = DeliveryStatus::parse(data) {
        assert!((1..=MAX_RECIPIENTS).contains(&report.recipients.len()));
    }
});
