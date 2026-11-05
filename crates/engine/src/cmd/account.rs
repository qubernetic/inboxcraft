// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `account.*`: the accounts.

use inboxcraft_model::{AccountId, CheckStep, JobKind};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::params::{NoParams, decode};
use crate::{AgentClass, Caller, CommandSpec, Context, EngineError, Outcome, json};

/// `account.list`: every account, with how its connection stands, its last failure and its last check.
pub(crate) const LIST: CommandSpec =
    CommandSpec { id: "account.list", class: Some(AgentClass::Safe), summary: "List the accounts and how each connection stands", run: list };

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(LIST.id, params)?;
    let accounts: Vec<Value> =
        context.store().accounts()?.iter().map(|account| json::account(account, context.state().accounts.get(&account.id()))).collect();
    Ok(Outcome::new(json!({ "accounts": accounts })))
}

/// `account.check`: checks an account step by step on a connection of its own (DNS, TCP, TLS, the certificate, the
/// IMAP sign-in; SMTP and CalDAV are listed as not built yet). The answer lists the steps as they stand; the worker
/// reports each as it ends, and `account.list` shows them. While a check of the account runs, another one starts
/// nothing (`started`: false), so the password is not sent over and over. After a refused or unanswered sign-in, a
/// check an agent asked for stops before the sign-in (`needs_owner`); only the owner's check sends the password again.
pub(crate) const CHECK: CommandSpec = CommandSpec {
    id: "account.check",
    class: Some(AgentClass::Safe),
    summary: "Check an account step by step and name the cause of a failure",
    run: check,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckParams {
    /// The account.
    account: u64,
}

fn check(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: CheckParams = decode(CHECK.id, params)?;
    let id = AccountId::new(params.account);
    if !context.store().accounts()?.iter().any(|account| account.id() == id) {
        return Err(EngineError::NotFound { what: "account", id: params.account });
    }
    if let Some(running) = context.state().accounts.get(&id).and_then(|state| state.check.as_ref()).filter(|check| check.is_running()) {
        return Ok(Outcome::new(json!({ "account": params.account, "started": false, "steps": json::steps(&running.steps) })));
    }
    let answer = json!({ "account": params.account, "started": true, "steps": json::steps(&CheckStep::plan()) });
    Ok(Outcome::new(answer).with_job(id, JobKind::Check { by_owner: context.caller() == Caller::Owner }))
}
