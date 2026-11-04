// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The crate's error type: a model [`ErrorKind`], one per cause, and a detail for the log.

use std::fmt;
use std::io;

use inboxcraft_model::ErrorKind;
use inboxcraft_proto::ProtoError;
use inboxcraft_time::Timestamp;

use crate::tls;

/// Why connecting, signing in or a job failed.
///
/// The [`kind`](Self::kind) is what the owner is told; the detail is for the log and never holds a password or
/// words a server sent before TLS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetError {
    kind: ErrorKind,
    detail: String,
}

impl NetError {
    /// An error of `kind`, with `detail` for the log.
    #[must_use]
    pub fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        Self { kind, detail: detail.into() }
    }

    /// The cause.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// What went wrong, for the log.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Whether the connection is unusable after this error: everything but a refused command, a folder or message
    /// that is gone, and a missing password.
    #[must_use]
    pub fn ends_connection(&self) -> bool {
        !matches!(self.kind, ErrorKind::CommandRefused | ErrorKind::FolderGone | ErrorKind::MessageGone | ErrorKind::CredentialMissing)
    }

    /// Whether a later attempt may get over this error: the network, a timeout, a dropped connection, or a server
    /// that refused or broke the protocol. A certificate fault, a refused sign-in and a missing password are not: they
    /// need the owner.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::HostNotFound
                | ErrorKind::ConnectionRefused
                | ErrorKind::TimedOut
                | ErrorKind::ConnectionDropped
                | ErrorKind::CommandRefused
                | ErrorKind::Protocol
                | ErrorKind::TooLarge
        )
    }

    /// A socket error; a TLS error inside it is classified by its certificate fault at the clock's `now`.
    pub(crate) fn io(error: &io::Error, now: Timestamp) -> Self {
        if let Some(tls) = error.get_ref().and_then(|inner| inner.downcast_ref::<rustls::Error>()) {
            return Self::tls(tls, now);
        }
        let kind = match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ErrorKind::TimedOut,
            io::ErrorKind::ConnectionRefused => ErrorKind::ConnectionRefused,
            // A reset, an end in the middle, a broken pipe, and whatever else stops a socket.
            _ => ErrorKind::ConnectionDropped,
        };
        Self::new(kind, error.to_string())
    }

    /// A TLS error, classified by its certificate fault at `now`.
    pub(crate) fn tls(error: &rustls::Error, now: Timestamp) -> Self {
        Self::new(tls::classify(error, now), error.to_string())
    }

    /// A fatal error of the IMAP session, or a command `proto` would not send.
    pub(crate) fn proto(error: &ProtoError) -> Self {
        let kind = match error {
            ProtoError::LineTooLong { .. } | ProtoError::LiteralTooLarge { .. } | ProtoError::TooManyLiterals { .. } | ProtoError::TooDeep { .. } => {
                ErrorKind::TooLarge
            }
            ProtoError::NotOffered("STARTTLS") => ErrorKind::StartTlsMissing,
            ProtoError::NotOffered("LOGIN" | "AUTH=PLAIN") => ErrorKind::NoLoginMethod,
            ProtoError::Bye(_) | ProtoError::Closed => ErrorKind::ConnectionDropped,
            _ => ErrorKind::Protocol,
        };
        Self::new(kind, error.to_string())
    }
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.kind, self.detail)
    }
}

impl std::error::Error for NetError {}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
