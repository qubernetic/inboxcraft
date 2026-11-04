// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The crate's error type.

use std::fmt;

use inboxcraft_charset::CharsetError;

/// Why a message, a part of it or a delivery report cannot be read.
///
/// Every limit is part of the error, so the caller can say what was too much. Text taken from the message (an
/// unknown transfer encoding) is capped and cleaned of control characters.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MimeError {
    /// The message is longer than `limit` bytes.
    TooLarge {
        /// The cap, [`crate::MAX_MESSAGE_BYTES`].
        limit: usize,
    },
    /// A header block has more than `limit` fields.
    TooManyFields {
        /// The cap, [`crate::MAX_FIELDS`].
        limit: usize,
    },
    /// A header field, unfolded, is longer than `limit` bytes.
    FieldTooLong {
        /// The cap, [`crate::MAX_FIELD_BYTES`].
        limit: usize,
    },
    /// Multiparts or attached messages are nested deeper than `limit`.
    TooDeep {
        /// The cap, [`crate::MAX_DEPTH`].
        limit: usize,
    },
    /// The message has more than `limit` parts.
    TooManyParts {
        /// The cap, [`crate::MAX_PARTS`].
        limit: usize,
    },
    /// A multipart has no `boundary` parameter.
    MissingBoundary,
    /// A multipart's boundary is empty, longer than 70 characters or not printable ASCII (RFC 2046 section 5.1.1).
    BadBoundary,
    /// A multipart's body has no line with its boundary.
    BoundaryNotFound,
    /// The `Content-Transfer-Encoding` is none of RFC 2045's.
    UnknownEncoding(String),
    /// A base64 body has bytes outside the base64 alphabet, or a lone character at the end of a group.
    BadBase64,
    /// The part's `charset` parameter names no character set that can be read.
    Charset(CharsetError),
    /// The message is no delivery report: no `multipart/report` with a `message/delivery-status` part.
    NotAReport,
    /// A delivery report has a line that is no field.
    MalformedReport,
    /// A delivery report lacks a field it must have.
    MissingField(&'static str),
    /// A delivery report field has a value that cannot be read.
    BadField(&'static str),
    /// A delivery report names more than `limit` recipients.
    TooManyRecipients {
        /// The cap, [`crate::MAX_RECIPIENTS`].
        limit: usize,
    },
}

impl fmt::Display for MimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { limit } => write!(f, "the message is longer than {limit} bytes"),
            Self::TooManyFields { limit } => write!(f, "a header has more than {limit} fields"),
            Self::FieldTooLong { limit } => write!(f, "a header field is longer than {limit} bytes"),
            Self::TooDeep { limit } => write!(f, "the message parts are nested deeper than {limit}"),
            Self::TooManyParts { limit } => write!(f, "the message has more than {limit} parts"),
            Self::MissingBoundary => f.write_str("a multipart has no boundary"),
            Self::BadBoundary => f.write_str("a multipart boundary is not valid"),
            Self::BoundaryNotFound => f.write_str("a multipart boundary is not in its body"),
            Self::UnknownEncoding(name) => write!(f, "unknown transfer encoding {name:?}"),
            Self::BadBase64 => f.write_str("a base64 body is broken"),
            Self::Charset(error) => write!(f, "{error}"),
            Self::NotAReport => f.write_str("the message is no delivery report"),
            Self::MalformedReport => f.write_str("a delivery report has a line that is no field"),
            Self::MissingField(name) => write!(f, "a delivery report lacks the {name} field"),
            Self::BadField(name) => write!(f, "a delivery report has a bad {name} field"),
            Self::TooManyRecipients { limit } => write!(f, "a delivery report names more than {limit} recipients"),
        }
    }
}

impl std::error::Error for MimeError {}

impl From<CharsetError> for MimeError {
    fn from(error: CharsetError) -> Self {
        Self::Charset(error)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
