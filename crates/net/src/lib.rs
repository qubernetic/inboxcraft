// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sockets, TLS, OAuth and the per-account sync workers.
//!
//! Layer 3, native only: the one crate that does network I/O. Nothing is implemented yet; the crate holds its place in the workspace (see
//! `docs/architecture.md`).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

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
