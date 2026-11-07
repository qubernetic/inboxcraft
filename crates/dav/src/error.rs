// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The crate's error type.

use std::fmt;

/// Why a request was not made, or why an exchange with the server is over.
///
/// No variant carries a password or the server's own words: a status code, a cleaned URL or the name of what was
/// wrong, nothing more.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DavError {
    /// A password would go over a connection that is not encrypted: Basic auth is sent over `https` only.
    NotEncrypted,
    /// A URL or a reference from the server cannot be used: another scheme, a user name in it, a bad host or port.
    BadUrl(&'static str),
    /// An argument cannot be sent: it is empty, too long, or holds a character a request line or header cannot carry.
    BadArgument(&'static str),
    /// The server sent the client to another origin (scheme, host or port). The client never follows it on its own,
    /// as the password would go with it; the caller may start again there once the owner agrees.
    OtherOrigin {
        /// Where the server pointed, cleaned: `scheme://host[:port]/path`.
        url: String,
    },
    /// More than [`crate::MAX_REDIRECTS`] redirects in a row.
    TooManyRedirects,
    /// The server did not take the user name and password (401).
    Unauthorized,
    /// The server answered with a status the exchange cannot go on from.
    Refused {
        /// The HTTP status code.
        status: u16,
    },
    /// The server's answer lacks what the exchange needs, such as the principal or the calendar home set.
    Missing(&'static str),
    /// The server's answer is not HTTP as RFC 9112 writes it.
    BadResponse(&'static str),
    /// The server's XML is not well-formed, or uses what this reader refuses (a document type declaration).
    BadXml(&'static str),
    /// The server's answer is larger than the client reads.
    TooLarge {
        /// What was too large.
        what: &'static str,
        /// The cap.
        limit: usize,
    },
    /// The exchange is over: it has no request left and takes no answer.
    Finished,
}

impl fmt::Display for DavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEncrypted => f.write_str("a password goes over an encrypted connection only"),
            Self::BadUrl(what) => write!(f, "the URL cannot be used: {what}"),
            Self::BadArgument(what) => write!(f, "the argument cannot be sent: {what}"),
            Self::OtherOrigin { url } => write!(f, "the server points to another server: {url}"),
            Self::TooManyRedirects => f.write_str("the server redirects too often"),
            Self::Unauthorized => f.write_str("the server did not take the user name and password"),
            Self::Refused { status } => write!(f, "the server answered {status}"),
            Self::Missing(what) => write!(f, "the server's answer lacks the {what}"),
            Self::BadResponse(what) => write!(f, "the server's answer is not HTTP: {what}"),
            Self::BadXml(what) => write!(f, "the server's XML cannot be read: {what}"),
            Self::TooLarge { what, limit } => write!(f, "the server's {what} is larger than {limit}"),
            Self::Finished => f.write_str("the exchange is over"),
        }
    }
}

impl std::error::Error for DavError {}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
