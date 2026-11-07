// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CalDAV (RFC 4791, RFC 6764) as sans-I/O state machines.
//!
//! Layer 3 (see `docs/architecture.md`): it reads and writes bytes, never sockets, and builds for wasm. `net` owns
//! the sockets and the TLS: it writes what [`Request::encode`] gives to the request's [`Request::origin`], feeds what
//! comes back to a [`ResponseReader`], and hands the [`Response`] to the exchange that asked.
//!
//! - [`Discovery`] finds the owner's calendars: the well-known path (RFC 6764), the principal (RFC 5397), the
//!   calendar home set and the [`Calendar`]s in it, with their name, colour, ctag, sync token and components.
//!   Redirects are followed on the same origin only; one to another origin ends with [`DavError::OtherOrigin`], as the
//!   password would go with it.
//! - [`Sync`] brings one calendar in step: a sync report from the last token (RFC 6578), started again without one
//!   when the server forgot it, or a listing of entity tags where the server has no sync report; then what changed is
//!   fetched with `calendar-multiget`. [`Changes`] lists what is new or changed, what is gone and the next token.
//! - [`put`] and [`delete`] write and remove one resource with `If-Match` or `If-None-Match: *`; a change made
//!   elsewhere comes back as [`Written::Conflict`] or [`Deleted::Conflict`] (412), never overwritten.
//! - [`xml`] is the small XML reader: no document type declaration (so no entity can grow), nesting, nodes and
//!   attributes capped.
//!
//! Basic auth ([`Credentials`]) is written into a request to an `https` origin only: [`Request::encode`] refuses a
//! password for `http` with [`DavError::NotEncrypted`]. `Debug` never shows it. Everything a server sends is hostile:
//! the head, the body, the XML, the number of resources and of redirects are capped, and a bad answer is an error,
//! never a panic.
//!
//! ```
//! use inboxcraft_dav::{Credentials, DavError, Discovery};
//!
//! let credentials = Credentials::new("ada@example.org", "not-a-real-password")?;
//! let discovery = Discovery::new("https://dav.example.org")?;
//! let bytes = discovery.request()?.encode(Some(&credentials))?;
//! assert!(bytes.starts_with(b"PROPFIND /.well-known/caldav HTTP/1.1\r\nHost: dav.example.org\r\nAuthorization: Basic "));
//! // No password over a connection that is not encrypted.
//! let clear = Discovery::new("http://dav.example.org")?;
//! assert_eq!(clear.request()?.encode(Some(&credentials)), Err(DavError::NotEncrypted));
//! # Ok::<(), DavError>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod auth;
mod discovery;
mod error;
mod http;
pub mod multistatus;
mod sync;
mod url;
mod write;
pub mod xml;

pub use auth::Credentials;
pub use discovery::{Account, Calendar, Discovery, MAX_REDIRECTS, WELL_KNOWN};
pub use error::DavError;
pub use http::{MAX_BODY, MAX_HEAD, MAX_HEADERS, Method, Request, Response, ResponseReader};
pub use sync::{Changes, MAX_MULTIGET, MAX_RESOURCES, MAX_ROUNDS, Resource, Sync};
pub use url::{MAX_URL, Origin};
pub use write::{Deleted, MAX_PUT, Precondition, Written, delete, delete_outcome, put, put_outcome};

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_dav::name(), "inboxcraft-dav");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
