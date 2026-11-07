// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own commands, one file per area.
//!
//! Every module lists its commands in `COMMANDS` and the parameter names they read in `FIELDS`, so a new command
//! edits only its own module. The registry then refuses it until it has an agent class, and
//! `hostile_params_never_panic` runs it with junk parameters, its `FIELDS` among them. A new module is one more name
//! in the `modules!` line below.

pub use mail::MAX_LIST;

use crate::CommandSpec;

/// One module's commands.
pub(crate) struct Module {
    /// The file name without `.rs`.
    #[cfg(test)]
    pub(crate) name: &'static str,
    /// Its commands, in the registry.
    pub(crate) commands: &'static [CommandSpec],
    /// The parameter names its commands read, for `hostile_params_never_panic`.
    #[cfg(test)]
    pub(crate) fields: &'static [&'static str],
}

/// Declares the command modules and [`MODULES`], their lists in the order the registry takes them.
macro_rules! modules {
    ($($name:ident),* $(,)?) => {
        $(mod $name;)*

        /// Every command module.
        pub(crate) const MODULES: &[Module] = &[$(Module {
            #[cfg(test)]
            name: stringify!($name),
            commands: $name::COMMANDS,
            #[cfg(test)]
            fields: $name::FIELDS,
        }),*];
    };
}

modules!(account, act, compose, folder, mail, notify, outbox, remote, sample, schedule, send, triage, ui);

/// Every built-in command.
pub(crate) fn all() -> Vec<CommandSpec> {
    MODULES.iter().flat_map(|module| module.commands).copied().collect()
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod account_tests;
#[cfg(test)]
#[path = "act_tests.rs"]
pub(crate) mod act_tests;
#[cfg(test)]
#[path = "add_tests.rs"]
mod add_tests;
#[cfg(test)]
#[path = "compose_tests.rs"]
pub(crate) mod compose_tests;
#[cfg(test)]
#[path = "done_tests.rs"]
mod done_tests;
#[cfg(test)]
#[path = "group_tests.rs"]
mod group_tests;
#[cfg(test)]
#[path = "hostile_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "modules_tests.rs"]
mod modules_tests;
#[cfg(test)]
#[path = "reader_tests.rs"]
mod reader_tests;
#[cfg(test)]
#[path = "sample_tests.rs"]
mod sample_tests;
#[cfg(test)]
#[path = "schedule_tests.rs"]
mod schedule_tests;
#[cfg(test)]
#[path = "snooze_tests.rs"]
mod snooze_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
#[path = "triage_tests.rs"]
mod triage_tests;
#[cfg(test)]
#[path = "ui_tests.rs"]
mod ui_tests;
