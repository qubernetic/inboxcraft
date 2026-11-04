// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sockets, TLS and the per-account sync workers.
//!
//! Layer 3, native only: the one crate that does network I/O (see `docs/architecture.md`).
//!
//! - [`TlsSettings`]: what the client trusts ([`Trust`]: the operating system's verifier, or given roots) and the
//!   clock certificates are checked at; every certificate fault has its own [`inboxcraft_model::ErrorKind`].
//! - [`Connector`] opens TCP connections with [`Timeouts`] and runs TLS over them.
//! - [`ImapConnection`] drives `proto`'s IMAP client over a connection: implicit TLS, or STARTTLS before anything
//!   else; a [`Password`] goes out only once the connection is encrypted.
//! - [`envelope`] reads the [`inboxcraft_model::Envelope`] of a message from its header block.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod connect;
pub mod envelope;
mod error;
mod imap;
mod tls;

pub use connect::{Connector, Timeouts};
pub use error::NetError;
pub use imap::{ImapConnection, Password};
pub use tls::{TlsSettings, Trust, classify};

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
