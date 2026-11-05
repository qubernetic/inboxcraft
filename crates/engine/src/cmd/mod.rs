// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own commands, one file per area.
//!
//! A new command goes into [`all`]; the registry then refuses it until it has an agent class, and
//! `hostile_params_never_panic` runs it with junk parameters.

mod account;
mod folder;
mod mail;

pub use mail::MAX_LIST;

use crate::CommandSpec;

/// Every built-in command.
pub(crate) fn all() -> Vec<CommandSpec> {
    vec![account::CHECK, account::LIST, folder::LIST, mail::ATTACHMENT, mail::LIST, mail::READ, mail::SEARCH, mail::THREAD]
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod account_tests;
#[cfg(test)]
#[path = "hostile_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "reader_tests.rs"]
mod reader_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
