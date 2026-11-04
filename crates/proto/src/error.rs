// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The crate's error type.

use std::fmt;

use crate::imap::State;

/// Why a command was not sent, or why the session is over.
///
/// The first group is refused before anything is written and leaves the session as it was; the second
/// ([`ProtoError::is_fatal`]) means the server broke the protocol or a limit, and the connection must be closed.
/// No variant carries a password, and text from the server is cleaned and capped.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProtoError {
    /// A password would go over a connection that is not encrypted: LOGIN and AUTHENTICATE need TLS first.
    NotEncrypted,
    /// STARTTLS on a connection that is encrypted already.
    AlreadyEncrypted,
    /// The command is not allowed in the session's state (FETCH before SELECT, LOGIN after LOGIN).
    WrongState {
        /// The command, as IMAP names it.
        command: &'static str,
        /// The state the session was in.
        state: State,
    },
    /// Another command is waiting for its answer; this client sends one at a time.
    Busy,
    /// What the server said it can do rules the command out: no `STARTTLS`, no `AUTH=PLAIN`, or `LOGINDISABLED`.
    NotOffered(&'static str),
    /// The command depends on the capabilities, and the server has not listed them yet: send CAPABILITY first.
    CapabilitiesUnknown,
    /// An argument cannot be sent: it is empty, too long, or holds a character the command cannot carry.
    BadArgument(&'static str),

    /// The server sent a line, or the text of one response, longer than `limit` bytes.
    LineTooLong {
        /// The cap, [`crate::imap::MAX_LINE`].
        limit: usize,
    },
    /// The server announced a literal of `size` bytes, more than the `limit` one response may carry.
    LiteralTooLarge {
        /// The size the server announced (`u64::MAX` when it did not fit).
        size: u64,
        /// The cap, [`crate::imap::MAX_LITERAL`].
        limit: usize,
    },
    /// A response carries more than `limit` literals.
    TooManyLiterals {
        /// The cap, [`crate::imap::MAX_LITERALS`].
        limit: usize,
    },
    /// Parenthesised lists are nested deeper than `limit`.
    TooDeep {
        /// The cap, [`crate::imap::MAX_DEPTH`].
        limit: usize,
    },
    /// A response does not fit the grammar where it matters: the named part of it.
    Malformed(&'static str),
    /// A tagged answer for a command that was not sent, or a request for more of a command that has nothing more.
    Unexpected(&'static str),
    /// The server greeted with PREAUTH on a connection that is not encrypted, which would rule STARTTLS out.
    PreauthWithoutTls,
    /// The server sent bytes after its go-ahead for STARTTLS, before the TLS handshake: they could be anyone's.
    PlaintextAfterStartTls,
    /// The server ended the session (BYE) without being asked to; its words, cleaned, and empty before TLS.
    Bye(String),
    /// The session is over, after a fatal error or LOGOUT.
    Closed,
}

impl ProtoError {
    /// Whether the session is over: the server broke the protocol or a limit, or ended the session.
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        !matches!(
            self,
            Self::NotEncrypted
                | Self::AlreadyEncrypted
                | Self::WrongState { .. }
                | Self::Busy
                | Self::NotOffered(_)
                | Self::CapabilitiesUnknown
                | Self::BadArgument(_)
        )
    }
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEncrypted => f.write_str("the connection is not encrypted, so no password is sent"),
            Self::AlreadyEncrypted => f.write_str("the connection is encrypted already"),
            Self::WrongState { command, state } => write!(f, "{command} cannot be sent in the {state} state"),
            Self::Busy => f.write_str("another command is waiting for its answer"),
            Self::NotOffered(what) => write!(f, "the server does not offer {what}"),
            Self::CapabilitiesUnknown => f.write_str("the server has not said what it can do yet"),
            Self::BadArgument(what) => write!(f, "the {what} cannot be sent"),
            Self::LineTooLong { limit } => write!(f, "the server sent a response longer than {limit} bytes"),
            Self::LiteralTooLarge { size, limit } => write!(f, "the server announced {size} bytes in one response, more than {limit}"),
            Self::TooManyLiterals { limit } => write!(f, "the server sent more than {limit} literals in one response"),
            Self::TooDeep { limit } => write!(f, "the server nested lists deeper than {limit}"),
            Self::Malformed(what) => write!(f, "the server sent a malformed {what}"),
            Self::Unexpected(what) => write!(f, "the server sent an unexpected {what}"),
            Self::PreauthWithoutTls => f.write_str("the server signed in without encryption (PREAUTH)"),
            Self::PlaintextAfterStartTls => f.write_str("the server sent data before encryption started"),
            Self::Bye(text) if text.is_empty() => f.write_str("the server ended the session"),
            Self::Bye(text) => write!(f, "the server ended the session: {text}"),
            Self::Closed => f.write_str("the session is over"),
        }
    }
}

impl std::error::Error for ProtoError {}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
