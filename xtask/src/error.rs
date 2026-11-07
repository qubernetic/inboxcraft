// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The one error type of xtask.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// What can stop a gate from giving an answer at all. A gate that runs and finds problems is not an error: it reports
/// them and fails.
#[derive(Debug)]
pub enum Error {
    /// A file or directory could not be read or written.
    Io { path: PathBuf, source: io::Error },
    /// A configuration file is missing a value or cannot be parsed.
    Config { path: PathBuf, message: String },
    /// A picture could not be read or written.
    Image { path: PathBuf, message: String },
    /// A tool could not be started.
    Spawn { program: String, source: io::Error },
    /// The report could not be written.
    Output(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Config { path, message } | Self::Image { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Spawn { program, source } => write!(f, "cannot run `{program}`: {source}"),
            Self::Output(source) => write!(f, "cannot write the report: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::Spawn { source, .. } | Self::Output(source) => Some(source),
            Self::Config { .. } | Self::Image { .. } => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::Output(source)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
