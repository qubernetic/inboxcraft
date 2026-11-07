// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `send`: sending a draft, undo send and send later. No command yet; a new one goes into [`COMMANDS`] and its
//! parameter names into `FIELDS`.

use crate::CommandSpec;

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &[];
