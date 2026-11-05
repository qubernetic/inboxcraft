// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own commands, one file per area.
//!
//! A new command goes into [`all`]; the registry then refuses it until it has an agent class, and
//! `hostile_params_never_panic` runs it with junk parameters.

mod account;

use crate::CommandSpec;

/// Every built-in command.
pub(crate) fn all() -> Vec<CommandSpec> {
    vec![account::LIST]
}

#[cfg(test)]
#[path = "hostile_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
