// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Internet messages and MIME (RFC 5322, RFC 2045 to 2049).
//!
//! Layer 2. Nothing is implemented yet; the crate holds its place in the workspace (see
//! `docs/architecture.md`).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod error;
mod header;
mod params;

pub use error::MimeError;
pub use header::{Field, Headers, MAX_FIELD_BYTES, MAX_FIELDS};
pub use params::{ContentType, Disposition, MAX_PARAMS, Params};

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_mime::name(), "inboxcraft-mime");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
