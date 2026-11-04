// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A kept message body: the source as the server sent it and its text for the search.

use crate::StoreError;

/// The largest message source the store keeps, in bytes.
pub const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// The longest body text the store indexes, in bytes; longer text is cut at a character boundary.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// The body of one message: the RFC 5322 source and the plain text the caller read from it.
///
/// The store does not parse mail (it sits below `mime`), so the caller hands in both. The text is only searched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    source: Vec<u8>,
    text: String,
}

impl Body {
    /// A body; `text` is cut to [`MAX_TEXT_BYTES`].
    ///
    /// # Errors
    ///
    /// [`StoreError::TooLarge`] for a source over [`MAX_SOURCE_BYTES`].
    pub fn new(source: Vec<u8>, text: &str) -> Result<Self, StoreError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(StoreError::TooLarge);
        }
        let mut end = text.len().min(MAX_TEXT_BYTES);
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        Ok(Self { source, text: text.get(..end).unwrap_or_default().to_owned() })
    }

    /// The source, as the server sent it.
    #[must_use]
    pub fn source(&self) -> &[u8] {
        &self.source
    }

    /// The text that is searched.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}
