// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The MX and SRV records of any DNS answer, and the name servers of any resolver configuration: none panics, and
//! every list stays within its cap.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_net::dns::{MAX_RECORDS, MAX_SERVERS, nameservers, read_mx, read_srv};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(hosts) = read_mx(data) {
        assert!(hosts.len() <= MAX_RECORDS);
    }
    if let Some(records) = read_srv(data) {
        assert!(records.len() <= MAX_RECORDS);
        assert!(records.iter().all(|record| record.port.get() > 0));
    }
    assert!(nameservers(&String::from_utf8_lossy(data)).len() <= MAX_SERVERS);
});
