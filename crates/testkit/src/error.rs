// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The testkit's own error: setting up a fake server or a test CA, and a script the client did not follow.

use std::fmt;
use std::io;

/// Why setting up, running or finishing a fake server failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A socket call failed.
    Io(io::Error),
    /// The TLS library refused a configuration.
    Tls(rustls::Error),
    /// Generating a test certificate failed.
    Certificate(rcgen::Error),
    /// No client connected before the accept timeout.
    NoClient,
    /// The client sent a line the script did not expect.
    Unexpected {
        /// The line the script expected.
        expected: String,
        /// The line the client sent.
        got: String,
    },
    /// The client closed the connection while the script still expected a line.
    ClientGone,
    /// The client sent a line or a literal longer than the server reads, or a literal it refused.
    LineTooLong,
    /// The server thread ended without a result.
    Thread,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "socket error: {err}"),
            Self::Tls(err) => write!(f, "TLS configuration error: {err}"),
            Self::Certificate(err) => write!(f, "test certificate error: {err}"),
            Self::NoClient => f.write_str("no client connected before the accept timeout"),
            Self::Unexpected { expected, got } => write!(f, "the script expected {expected:?}, the client sent {got:?}"),
            Self::ClientGone => f.write_str("the client closed the connection while the script expected a line"),
            Self::LineTooLong => f.write_str("the client sent a line or literal longer than the server reads"),
            Self::Thread => f.write_str("the server thread ended without a result"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Tls(err) => Some(err),
            Self::Certificate(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<rustls::Error> for Error {
    fn from(err: rustls::Error) -> Self {
        Self::Tls(err)
    }
}

impl From<rcgen::Error> for Error {
    fn from(err: rcgen::Error) -> Self {
        Self::Certificate(err)
    }
}
