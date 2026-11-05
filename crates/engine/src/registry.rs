// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command registry: every command by id, each with its agent class.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::{AgentClass, Context, EngineError, Outcome};

/// What a command does: it reads the session through `Context` and returns its answer with the changes to make. It
/// never changes anything itself, so a command that fails or panics leaves the session as it was.
pub type Run = fn(&Context<'_>, &Value) -> Result<Outcome, EngineError>;

/// One command.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    /// The stable id menus, shortcuts, the palette, the CLI, the control channel and MCP dispatch by: lowercase words
    /// joined by dots (`mail.list`).
    pub id: &'static str,
    /// Who may run it. `None` is a command that has not picked a class: the registry refuses it.
    pub class: Option<AgentClass>,
    /// One line on what it does, in English, for the tool catalogue (never shown in the UI).
    pub summary: &'static str,
    /// The command itself.
    pub run: Run,
}

/// Every command the session knows, by id.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// Each command with the class it was registered with.
    specs: BTreeMap<&'static str, (CommandSpec, AgentClass)>,
}

impl Registry {
    /// A registry without commands.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The registry with InboxCraft's own commands.
    ///
    /// # Errors
    ///
    /// As [`Registry::register`], for a built-in command that breaks a rule; a test keeps this from happening.
    pub fn builtin() -> Result<Self, EngineError> {
        let mut registry = Self::new();
        for spec in crate::cmd::all() {
            registry.register(spec)?;
        }
        Ok(registry)
    }

    /// Adds `spec`.
    ///
    /// # Errors
    ///
    /// [`EngineError::Unclassified`] for a command without an agent class, [`EngineError::BadCommandId`] for an id
    /// that is not lowercase words joined by dots, [`EngineError::DuplicateCommand`] for an id already taken.
    pub fn register(&mut self, spec: CommandSpec) -> Result<(), EngineError> {
        let Some(class) = spec.class else { return Err(EngineError::Unclassified(spec.id)) };
        if !is_command_id(spec.id) {
            return Err(EngineError::BadCommandId(spec.id));
        }
        if self.specs.contains_key(spec.id) {
            return Err(EngineError::DuplicateCommand(spec.id));
        }
        self.specs.insert(spec.id, (spec, class));
        Ok(())
    }

    /// The command `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.specs.get(id).map(|(spec, _)| spec)
    }

    /// The command `id` with its class.
    pub(crate) fn lookup(&self, id: &str) -> Option<(CommandSpec, AgentClass)> {
        self.specs.get(id).copied()
    }

    /// Every command, in id order.
    pub fn specs(&self) -> impl Iterator<Item = &CommandSpec> {
        self.specs.values().map(|(spec, _)| spec)
    }
}

/// Whether `id` is two or more words of lowercase letters, digits and `_`, each starting with a letter, joined by dots.
///
/// ```
/// use inboxcraft_engine::is_command_id;
///
/// assert!(is_command_id("mail.list"));
/// assert!(is_command_id("account.check_all"));
/// assert!(!is_command_id("mail"));
/// assert!(!is_command_id("Mail.list"));
/// assert!(!is_command_id("mail..list"));
/// ```
#[must_use]
pub fn is_command_id(id: &str) -> bool {
    let word = |word: &str| {
        word.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && word.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    id.len() <= 64 && id.split('.').count() >= 2 && id.split('.').all(word)
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
