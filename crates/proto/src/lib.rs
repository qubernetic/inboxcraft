// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Mail protocols as sans-I/O state machines.
//!
//! Layer 3 (see `docs/architecture.md`): it reads and writes bytes, never sockets, and builds for wasm. `net` owns
//! the sockets and the TLS and drives the state machines.
//!
//! - [`imap::Client`]: an IMAP client core. Bytes from the server go into [`imap::Client::receive`], bytes for the
//!   server come out of [`imap::Client::take_output`], and what happened comes out of [`imap::Client::next_event`]
//!   as [`imap::Event`]s. LOGIN and AUTHENTICATE PLAIN are refused until the connection is encrypted; after STARTTLS
//!   and after a sign-in the capabilities are read again. For keeping a mailbox in step it has ENABLE, CONDSTORE,
//!   QRESYNC (VANISHED, and a changed UIDVALIDITY reported as [`imap::Event::UidValidityChanged`]), IDLE and
//!   NOTIFY; each is sent only to a server that offers it. UID MOVE falls back to UID COPY, a `\Deleted` flag
//!   and UID EXPUNGE, and says how far it got ([`imap::Removal`]); COPYUID ([`imap::CopyUid`]) is kept only when it
//!   fits what was asked for. COMPRESS=DEFLATE is negotiated here and run by `net`.
//!
//! Everything a server sends is hostile. Lines, literals, nesting and lists are capped; a server that breaks the
//! protocol or a limit gets a fatal [`ProtoError`], never a panic. No error, event or `Debug` output carries a
//! password: text the server echoes has every form of it hidden.
//!
//! ```
//! use inboxcraft_proto::imap::{Answer, Client, Event, State, Transport};
//! use inboxcraft_proto::ProtoError;
//!
//! let mut client = Client::new(Transport::Plain);
//! client.receive(b"* OK [CAPABILITY IMAP4rev1 STARTTLS AUTH=PLAIN] ready\r\n")?;
//! assert_eq!(client.next_event(), Some(Event::Greeted { preauth: false }));
//! // No password over a connection that is not encrypted.
//! assert_eq!(client.login("ada@example.org", "secret"), Err(ProtoError::NotEncrypted));
//!
//! let tag = client.starttls()?;
//! assert_eq!(client.take_output(), b"A0001 STARTTLS\r\n");
//! client.receive(b"A0001 OK begin TLS\r\n")?;
//! assert_eq!(client.next_event(), Some(Event::Completed { tag, outcome: Ok(Answer::StartTls) }));
//! assert_eq!(client.state(), State::StartingTls);
//! // The caller runs the TLS handshake here, then:
//! client.tls_established()?;
//! assert_eq!(client.take_output(), b"A0002 CAPABILITY\r\n");
//! # Ok::<(), ProtoError>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod error;
pub mod imap;

pub use error::ProtoError;

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_proto::name(), "inboxcraft-proto");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
