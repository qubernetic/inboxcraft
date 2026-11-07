// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The engine's error type.

use std::fmt;

use inboxcraft_store::StoreError;

use crate::AgentClass;

/// The longest command id or parameter complaint an error repeats, in characters.
pub const MAX_ERROR_TEXT: usize = 160;

/// Why a command or an event was refused. A refused command changes nothing.
///
/// No variant carries message text: a parameter complaint repeats at most [`MAX_ERROR_TEXT`] characters of what the
/// caller sent, without control characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// No command has this id (cleaned and cut).
    UnknownCommand(String),
    /// The command declares no agent class, so it never runs.
    Unclassified(&'static str),
    /// A command with this id is already registered.
    DuplicateCommand(&'static str),
    /// A command id is not lowercase words joined by dots (`mail.list`).
    BadCommandId(&'static str),
    /// The command's agent class does not let this caller run it.
    NotAllowed {
        /// The command.
        command: &'static str,
        /// Its class.
        class: AgentClass,
    },
    /// The parameters do not fit the command.
    BadParams {
        /// The command.
        command: &'static str,
        /// What is wrong, cleaned and cut.
        reason: String,
    },
    /// The command names something the store does not hold.
    NotFound {
        /// What kind of thing: `account`, `folder`, `message` or `schedule entry`.
        what: &'static str,
        /// Its id.
        id: u64,
    },
    /// The store refused the change or could not answer.
    Store(StoreError),
    /// The command panicked; the session is as it was before the command.
    Panicked(&'static str),
    /// The command runs only in a session over the invented sample mailbox (`Session::sample`).
    SampleOnly(&'static str),
}

impl EngineError {
    /// A stable snake_case code for the CLI's `--json` errors and MCP.
    ///
    /// ```
    /// use inboxcraft_engine::EngineError;
    ///
    /// assert_eq!(EngineError::Unclassified("mail.list").code(), "unclassified");
    /// ```
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownCommand(_) => "unknown_command",
            Self::Unclassified(_) => "unclassified",
            Self::DuplicateCommand(_) => "duplicate_command",
            Self::BadCommandId(_) => "bad_command_id",
            Self::NotAllowed { .. } => "not_allowed",
            Self::BadParams { .. } => "bad_params",
            Self::NotFound { .. } => "not_found",
            Self::Store(_) => "store",
            Self::Panicked(_) => "panicked",
            Self::SampleOnly(_) => "sample_only",
        }
    }

    /// A parameter complaint about `command`, cleaned and cut to [`MAX_ERROR_TEXT`].
    pub(crate) fn bad_params(command: &'static str, reason: &str) -> Self {
        Self::BadParams { command, reason: clean(reason) }
    }
}

/// `text` without control characters, cut to [`MAX_ERROR_TEXT`] characters.
pub(crate) fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(MAX_ERROR_TEXT).collect()
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand(id) => write!(f, "there is no command {id:?}"),
            Self::Unclassified(id) => write!(f, "the command {id} has no agent class, so it does not run"),
            Self::DuplicateCommand(id) => write!(f, "the command {id} is registered twice"),
            Self::BadCommandId(id) => write!(f, "{id:?} is not a command id"),
            Self::NotAllowed { command, class } => write!(f, "the command {command} is {class}: an agent may not run it"),
            Self::BadParams { command, reason } => write!(f, "bad parameters for {command}: {reason}"),
            Self::NotFound { what, id } => write!(f, "there is no {what} {id}"),
            Self::Store(error) => write!(f, "{error}"),
            Self::Panicked(id) => write!(f, "the command {id} failed inside; nothing was changed"),
            Self::SampleOnly(id) => write!(f, "the command {id} runs only on the sample mailbox (--sample)"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StoreError> for EngineError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
