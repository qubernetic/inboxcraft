// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent classes: who may run a command.

use std::fmt;

/// What an agent may do with a command. Every command declares one; an unclassified command never runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentClass {
    /// Reading, searching, drafting into Drafts, checking an account: an agent may run it.
    Safe,
    /// Sending, deleting, moving, blocking, changing settings: only with the owner's explicit approval.
    Owner,
    /// Exporting credentials, changing agent grants: never offered to an agent.
    Never,
}

impl AgentClass {
    /// The class's name, as the CLI and the tool catalogue write it.
    ///
    /// ```
    /// use inboxcraft_engine::AgentClass;
    ///
    /// assert_eq!(AgentClass::Safe.name(), "safe");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Owner => "owner",
            Self::Never => "never",
        }
    }

    /// Whether `caller` may run a command of this class. The owner may run every command; an agent only `safe` ones
    /// until the owner's approval arrives with the first `owner` command.
    #[must_use]
    pub const fn allows(self, caller: Caller) -> bool {
        matches!((caller, self), (Caller::Owner, _) | (Caller::Agent, Self::Safe))
    }
}

impl fmt::Display for AgentClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Who asks for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Caller {
    /// The owner, through the window, a shortcut or the palette.
    Owner,
    /// An agent, through the CLI, the control channel or MCP.
    Agent,
}
