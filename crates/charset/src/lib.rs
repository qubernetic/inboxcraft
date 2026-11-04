// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Character set decoding for mail text.
//!
//! Layer 0, standalone: it depends on no other workspace crate (see `docs/architecture.md`).
//!
//! - [`Charset`] is one of the WHATWG character sets, found by any of its labels; [`decode`] turns bytes in it into
//!   UTF-8 text. A label the WHATWG list does not know is a [`CharsetError`], never a panic; bytes the charset cannot
//!   hold become U+FFFD and mark the result [`Decoded::malformed`].
//! - [`decode_text`] reads the RFC 2047 encoded words in header text, [`decode_word`] one word strictly (a broken
//!   word is a [`WordError`]), and [`encode_text`] writes the parts of text that need it as encoded words in UTF-8.
//!
//! ```
//! use inboxcraft_charset::{decode, decode_text, encode_text};
//!
//! let decoded = decode("iso-8859-2", b"\xe1rv\xedzt\xfbr\xf5")?;
//! assert_eq!(decoded.text, "árvíztűrő");
//! assert!(!decoded.malformed);
//! assert!(decode("x-unknown", b"").is_err());
//!
//! assert_eq!(decode_text("Re: =?ISO-8859-1?Q?Andr=E9?=").text, "Re: André");
//! assert_eq!(encode_text("Re: André"), "Re: =?UTF-8?Q?Andr=C3=A9?=");
//! # Ok::<(), inboxcraft_charset::CharsetError>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod base64;
mod charset;
mod rfc2047;

pub use charset::{Charset, CharsetError, Decoded, MAX_LABEL_IN_ERROR, decode};
pub use rfc2047::{MAX_WORD_LEN, WordError, decode_text, decode_word, encode_text};

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
