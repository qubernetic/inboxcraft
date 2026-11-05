// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `account.*`: the accounts.

use serde_json::{Value, json};

use crate::params::{NoParams, decode};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, json};

/// `account.list`: every account, with how its connection stands and its last failure.
pub(crate) const LIST: CommandSpec =
    CommandSpec { id: "account.list", class: Some(AgentClass::Safe), summary: "List the accounts and how each connection stands", run: list };

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(LIST.id, params)?;
    let accounts: Vec<Value> =
        context.store().accounts()?.iter().map(|account| json::account(account, context.state().accounts.get(&account.id()))).collect();
    Ok(Outcome::new(json!({ "accounts": accounts })))
}
