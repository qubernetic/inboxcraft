// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The issuer and validity of any bytes a server presents as its certificate: none panics, and the issuer holds no
//! control character and stays within its caps.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_net::certificate::read;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(certificate) = read(data) {
        let issuer = certificate.issuer();
        assert!(!issuer.chars().any(char::is_control), "{issuer:?}");
        // Sixteen attributes of at most 128 characters, a label and a separator each.
        assert!(issuer.chars().count() <= 16 * (128 + 5), "{}", issuer.len());
    }
});
