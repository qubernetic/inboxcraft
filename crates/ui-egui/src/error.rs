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
    /// Running in the background or starting with the computer could not be changed.
    Desk(String),
    /// The front end refused what was asked; the sentence, translated, says why.
    Refused(String),
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
            Self::Desk(detail) | Self::Refused(detail) => write!(f, "{detail}"),
            Self::Answer { command, detail } => write!(f, "{command} answered in an unexpected shape: {detail}"),
        }
    }
}

impl std::error::Error for UiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Sample(_) | Self::Shot(_) | Self::Picture(_) | Self::Keep(_) | Self::Desk(_) | Self::Refused(_) | Self::Answer { .. } => None,
        }
    }
}

/// The line for the app's own standard error when `error` is a store failure with the database's own words, unless
/// they are the words of the line before (`logged`, then kept). The caller (the CLI, MCP, the window's banner) is
/// answered only the fixed sentence; the words, which can name the store's tables and columns or carry a crafted
/// file's trigger text, cut to [`inboxcraft_store::MAX_ERROR_DETAIL`] characters without control characters, stay on
/// the owner's machine.
#[must_use]
pub fn store_log(error: &EngineError, logged: &mut Option<String>) -> Option<String> {
    let EngineError::Store(store) = error else { return None };
    let detail = store.detail()?;
    if logged.as_deref() == Some(detail) {
        return None;
    }
    *logged = Some(detail.to_owned());
    Some(format!("inboxcraft: a command failed: {store} (the database said: {detail})"))
}

impl From<EngineError> for UiError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
