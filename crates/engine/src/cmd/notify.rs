// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `notify`: which arriving mail and reminders notify the owner (see [`crate::notify`]). A new command goes into
//! [`COMMANDS`] and its parameter names into `FIELDS`.
//!
//! `notify.settings` (`safe`) shows every account's level and whether returned snoozes, follow-up reminders and
//! calendar alerts tell the owner; `notify.set` (`owner`, a setting) changes any of them.

use inboxcraft_model::AccountId;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::notify::{Level, Settings};
use crate::params::{NoParams, decode};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[SETTINGS, SET];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["account", "level", "returns", "reminders", "alerts"];

/// `notify.settings`: what tells the owner.
pub(crate) const SETTINGS: CommandSpec = CommandSpec {
    id: "notify.settings",
    class: Some(AgentClass::Safe),
    summary: "Show which new mail, returned snoozes, reminders and calendar alerts notify the owner",
    run: settings,
};

/// `notify.set`: change what tells the owner.
pub(crate) const SET: CommandSpec = CommandSpec {
    id: "notify.set",
    class: Some(AgentClass::Owner),
    summary: "Notify of all new mail of an account, of people only, or of none; returned snoozes, reminders and calendar alerts on or off",
    run: set,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    /// The account whose level changes; with `level`.
    account: Option<u64>,
    /// `all`, `people` or `none`; with `account`.
    level: Option<String>,
    /// Returned snoozes tell the owner.
    returns: Option<bool>,
    /// Follow-up reminders tell the owner.
    reminders: Option<bool>,
    /// Calendar alerts tell the owner.
    alerts: Option<bool>,
}

fn settings(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(SETTINGS.id, params)?;
    Ok(Outcome::new(answer(context, &context.state().notify)?))
}

fn set(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SetParams = decode(SET.id, params)?;
    let mut settings = context.state().notify.clone();
    match (params.account, params.level.as_deref()) {
        (Some(account), Some(level)) => {
            let level = Level::from_name(level).ok_or_else(|| EngineError::bad_params(SET.id, "level is all, people or none"))?;
            let account = AccountId::new(account);
            if !context.store().accounts()?.iter().any(|held| held.id() == account) {
                return Err(EngineError::NotFound { what: "account", id: account.get() });
            }
            settings.set_level(account, level);
        }
        (None, None) => {
            if params.returns.is_none() && params.reminders.is_none() && params.alerts.is_none() {
                return Err(EngineError::bad_params(SET.id, "nothing to change: give account and level, returns, reminders or alerts"));
            }
        }
        _ => return Err(EngineError::bad_params(SET.id, "account and level go together")),
    }
    settings.returns = params.returns.unwrap_or(settings.returns);
    settings.reminders = params.reminders.unwrap_or(settings.reminders);
    settings.alerts = params.alerts.unwrap_or(settings.alerts);
    Ok(Outcome::new(answer(context, &settings)?).with_notify(settings))
}

/// `settings` for every account the store holds.
fn answer(context: &Context<'_>, settings: &Settings) -> Result<Value, EngineError> {
    let accounts: Vec<Value> = context
        .store()
        .accounts()?
        .iter()
        .map(|account| json!({ "account": account.id().get(), "address": account.address().as_str(), "level": settings.level(account.id()).name() }))
        .collect();
    let levels: Vec<&str> = Level::ALL.iter().map(|level| level.name()).collect();
    Ok(json!({ "accounts": accounts, "levels": levels, "returns": settings.returns, "reminders": settings.reminders, "alerts": settings.alerts }))
}
