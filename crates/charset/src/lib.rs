// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Character set decoding for mail text.
//!
//! Layer 0, standalone: it depends on no other workspace crate (see `docs/architecture.md`).
//!
//! - [`Charset`] is one of the WHATWG character sets, found by any of its labels; [`decode`] turns bytes in it into
//!   UTF-8 text. A label the WHATWG list does not know is a [`CharsetError`], never a panic; bytes the charset cannot
//!   hold become U+FFFD and mark the result [`Decoded::malformed`].
//!
//! ```
//! use inboxcraft_charset::decode;
//!
//! let decoded = decode("iso-8859-2", b"\xe1rv\xedzt\xfbr\xf5")?;
//! assert_eq!(decoded.text, "árvíztűrő");
//! assert!(!decoded.malformed);
//! assert!(decode("x-unknown", b"").is_err());
//! # Ok::<(), inboxcraft_charset::CharsetError>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod charset;

pub use charset::{Charset, CharsetError, Decoded, MAX_LABEL_IN_ERROR, decode};

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_charset::name(), "inboxcraft-charset");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
