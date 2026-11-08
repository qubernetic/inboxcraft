// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `remote.*`: the senders whose remote pictures load without asking.
//!
//! A remote picture tells its server that the message was opened, when and from where. The reader asks the owner
//! every time, unless the owner allowed the sender always; only the owner may allow or forget a sender, because it
//! changes what leaves the computer. Nothing here fetches anything: the front end does, through `net`, once allowed.

use inboxcraft_model::EmailAddress;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::params::{NoParams, decode};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[ALLOW, FORGET, LIST];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["address"];

/// `remote.allow`: the sender's remote pictures load without asking, in every account.
pub(crate) const ALLOW: CommandSpec = CommandSpec {
    id: "remote.allow",
    class: Some(AgentClass::Owner),
    summary: "Let the remote pictures of one sender load without asking",
    run: allow,
};

/// `remote.forget`: the reader asks again before the sender's remote pictures load.
pub(crate) const FORGET: CommandSpec = CommandSpec {
    id: "remote.forget",
    class: Some(AgentClass::Owner),
    summary: "Ask again before the remote pictures of one sender load",
    run: forget,
};

/// `remote.list`: the senders allowed.
pub(crate) const LIST: CommandSpec = CommandSpec {
    id: "remote.list",
    class: Some(AgentClass::Safe),
    summary: "List the senders whose remote pictures load without asking",
    run: list,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SenderParams {
    /// The sender's address.
    address: String,
}

/// The sender of `params` as the set keeps it: the address in lower case.
fn sender(command: &'static str, params: &Value) -> Result<String, EngineError> {
    let params: SenderParams = decode(command, params)?;
    let address = EmailAddress::new(params.address.trim()).map_err(|error| EngineError::bad_params(command, &error.to_string()))?;
    Ok(address.as_str().to_lowercase())
}

fn allow(_context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let sender = sender(ALLOW.id, params)?;
    Ok(Outcome::new(json!({ "address": sender, "allowed": true })).with_sender(sender, true))
}

fn forget(_context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let sender = sender(FORGET.id, params)?;
    Ok(Outcome::new(json!({ "address": sender, "allowed": false })).with_sender(sender, false))
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(LIST.id, params)?;
    Ok(Outcome::new(json!({ "senders": context.state().remote_senders.iter().collect::<Vec<_>>() })))
}

/// Whether the remote pictures of a message from `from` load without asking: its first sender is allowed.
pub(crate) fn allowed(context: &Context<'_>, from: &[inboxcraft_model::Address]) -> bool {
    from.first().is_some_and(|first| context.state().remote_senders.contains(&first.email().as_str().to_lowercase()))
}
