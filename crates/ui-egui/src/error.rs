// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What can go wrong in the front end.

use std::fmt;

use inboxcraft_engine::EngineError;

/// A failure of the front end. No variant carries message text.
#[derive(Debug)]
pub enum UiError {
    /// The sample mailbox could not be built.
    Sample(String),
    /// A command failed.
    Engine(EngineError),
    /// A `ui_shot` script or picture failed.
    Shot(String),
    /// A picture of a message could not be shown.
    Picture(String),
    /// The appearance choices could not be kept on disk.
    Keep(String),
    /// A command answered in a shape the front end does not read.
    Answer {
        /// The command.
        command: &'static str,
        /// What did not fit.
        detail: String,
    },
}

impl UiError {
    /// A [`UiError::Sample`] from any error.
    pub(crate) fn sample(error: impl fmt::Display) -> Self {
        Self::Sample(error.to_string())
    }
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sample(detail) => write!(f, "the sample mailbox could not be built: {detail}"),
            Self::Engine(error) => write!(f, "{error}"),
            Self::Shot(detail) => write!(f, "ui_shot: {detail}"),
            Self::Picture(detail) => write!(f, "{detail}"),
            Self::Keep(detail) => write!(f, "the appearance could not be kept: {detail}"),
            Self::Answer { command, detail } => write!(f, "{command} answered in an unexpected shape: {detail}"),
        }
    }
}

impl std::error::Error for UiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Sample(_) | Self::Shot(_) | Self::Picture(_) | Self::Keep(_) | Self::Answer { .. } => None,
        }
    }
}

impl From<EngineError> for UiError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
