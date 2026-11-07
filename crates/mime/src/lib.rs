// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Internet messages and MIME (RFC 5322, RFC 2045 to 2049): reading and building them.
//!
//! Layer 2 (see `docs/architecture.md`); it uses `charset` for character sets and RFC 2047 encoded words. No I/O,
//! the same on native and wasm.
//!
//! - [`parse`] reads a message into a tree of [`Part`]s: [`Headers`] unfolded, the [`ContentType`] and
//!   [`Disposition`] with their RFC 2231 [`Params`], the [`TransferEncoding`], and the [`Body`]: a leaf, a
//!   [`Multipart`] or an attached message. Parts borrow the message's bytes; [`Part::decoded`] undoes the transfer
//!   encoding and [`Part::text`] the character set of one part when asked.
//! - [`Part::text_body`], [`Part::html_body`] and [`Part::attachments`] find what a reader shows.
//! - [`Part::delivery_status`] reads the [`DeliveryStatus`] of a bounce (RFC 3464).
//! - [`MessageBuilder`] writes a message from [`Mailbox`]es, text, HTML and [`Attachment`]s, behind a header
//!   injection guard: text for people loses CR, LF and NUL, and a value with a grammar of its own that does not fit
//!   it is [`MimeError::BadHeader`]. What it writes, [`parse`] reads back.
//!
//! Mail is hostile. Nothing panics on any bytes; what cannot be read is a [`MimeError`]: a message over
//! [`MAX_MESSAGE_BYTES`], more than [`MAX_FIELDS`] fields or a field over [`MAX_FIELD_BYTES`], nesting deeper than
//! [`MAX_DEPTH`], more than [`MAX_PARTS`] parts, a missing or bad boundary, broken base64, an unknown transfer
//! encoding or character set. Every allocation is bounded by the input, the work is linear in it for each level of
//! nesting, and header text for people ([`Headers::text`], [`Part::file_name`]) holds no control character and no
//! invisible direction mark.
//!
//! ```
//! use inboxcraft_mime::{Body, MimeError, parse};
//!
//! let bytes = b"Subject: =?UTF-8?Q?K=C3=A9p?=\r\n\
//!     Content-Type: multipart/mixed; boundary=b\r\n\
//!     \r\n\
//!     --b\r\n\
//!     Content-Type: text/plain; charset=iso-8859-2\r\n\
//!     Content-Transfer-Encoding: quoted-printable\r\n\
//!     \r\n\
//!     =C1rv=EDzt=FBr=F5\r\n\
//!     --b\r\n\
//!     Content-Type: image/png; name=kep.png\r\n\
//!     Content-Transfer-Encoding: base64\r\n\
//!     \r\n\
//!     iVBORw==\r\n\
//!     --b--\r\n";
//! let message = parse(bytes)?;
//! assert_eq!(message.headers().text("subject").as_deref(), Some("Kép"));
//! assert!(matches!(message.body(), Body::Multipart(_)));
//! assert_eq!(message.text_body().map(|part| part.text()).transpose()?.as_deref(), Some("Árvíztűrő"));
//! let attachments = message.attachments();
//! assert_eq!(attachments[0].file_name().as_deref(), Some("kep.png"));
//! assert_eq!(&*attachments[0].decoded()?, b"\x89PNG");
//!
//! assert_eq!(parse(b"Content-Type: multipart/mixed\r\n\r\nx").unwrap_err(), MimeError::MissingBoundary);
//! # Ok::<(), MimeError>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod build;
mod dsn;
mod encode;
mod error;
mod header;
mod params;
mod part;
mod transfer;

pub use build::{Attachment, Mailbox, MessageBuilder, is_media_type, is_message_id};
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
