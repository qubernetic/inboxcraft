// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Internet messages and MIME (RFC 5322, RFC 2045 to 2049).
//!
//! Layer 2. Nothing is implemented yet; the crate holds its place in the workspace (see
//! `docs/architecture.md`).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod dsn;
mod error;
mod header;
mod params;
mod part;
mod transfer;

pub use dsn::{Action, DeliveryStatus, MAX_RECIPIENTS, RecipientStatus, Status, TypedValue};
pub use error::MimeError;
pub use header::{Field, Headers, MAX_FIELD_BYTES, MAX_FIELDS};
pub use inboxcraft_charset::CharsetError;
pub use params::{ContentType, Disposition, MAX_PARAMS, Params};
pub use part::{Body, MAX_DEPTH, MAX_MESSAGE_BYTES, MAX_PARTS, Multipart, Part, parse};
pub use transfer::{MAX_NAME_IN_ERROR, TransferEncoding};

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
