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
//!   else; a [`Password`] goes out only once the connection is encrypted. After the sign-in it switches QRESYNC on
//!   and compresses the stream (COMPRESS=DEFLATE) where the server offers them.
//! - [`Worker`]: one thread per account. It takes the engine's [`inboxcraft_model::Job`]s in order on one
//!   signed-in connection and reports [`inboxcraft_model::Event`]s: the folder list, the first sync of a folder
//!   (headers with PREVIEW, then the bodies of the messages of the last [`SyncConfig::body_days`] days), and the
//!   body of a message the owner opens. Folders are opened read-only (EXAMINE): nothing changes on the server.
//!   Without a connection, jobs wait in an offline queue while the worker reconnects after a [`Backoff`] wait.
//!   A known folder is resynced: changed flags, removed and new messages (QRESYNC where the server offers it). The
//!   folder of an Idle job is watched between jobs with IDLE ([`WatchConfig`]) and resynced after each change and
//!   each reconnect; a folder whose UIDVALIDITY changed is reloaded.
//! - [`check_account`] runs the account check step by step ([`inboxcraft_model::CheckStep`]): DNS, TCP (with the
//!   blocked-mail-ports test), TLS, the certificate (its issuer from [`certificate`], and the clock check) and the
//!   IMAP sign-in, which sends a password only after the certificate verified and only to a [`CheckTarget`] the owner
//!   has seen. A worker runs it for its account on a connection of its own ([`inboxcraft_model::JobKind::Check`]).
//! - [`envelope`] reads the [`inboxcraft_model::Envelope`] of a message from its header block.
//! - [`autoconfig`] reads Mozilla's autoconfig file: the encrypted IMAP and submission servers a provider publishes.
//! - [`detect`] proposes settings from an address: MX ([`dns`]), autoconfig at the domain over HTTPS ([`web`]), the
//!   ISPDB only when switched on, SRV, and common host names as a hint. A proposal is detected settings: no password
//!   goes to them before the owner has seen them and TLS has verified.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod autoconfig;
mod backoff;
pub mod certificate;
mod check;
mod compress;
mod connect;
pub mod detect;
pub mod dns;
pub mod envelope;
mod error;
mod imap;
pub mod picture;
mod sync;
mod tls;
mod watch;
pub mod web;
mod worker;

pub use backoff::Backoff;
pub use check::{CheckTarget, HTTPS_PORT, MAIL_PORTS, check_account};
pub use connect::{Connector, Dialer, SystemDialer, Timeouts};
pub use error::NetError;
pub use imap::{ImapConnection, Password};
pub use sync::SyncConfig;
pub use tls::{TlsSettings, Trust, classify};
pub use watch::WatchConfig;
pub use worker::{Credentials, MAX_QUEUED, Worker, WorkerSettings};

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
