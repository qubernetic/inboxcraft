// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sockets, TLS and the per-account sync workers.
//!
//! Layer 3, native only: the one crate that does network I/O (see `docs/architecture.md`).
//!
//! - [`envelope`] reads the [`inboxcraft_model::Envelope`] of a message from its header block.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod envelope;

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_net::name(), "inboxcraft-net");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
