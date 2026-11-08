// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `template`: the owner's templates, kept on their account. A new command goes into [`COMMANDS`] and its parameter
//! names into `FIELDS`.
//!
//! Listing them is `safe`; saving and deleting one changes a setting, so `owner`. `compose.template` puts one into a
//! draft (see `crate::template` for the placeholders and blanks).

use inboxcraft_model::{Account, AccountId, Template};
use inboxcraft_store::Change;
use serde::Deserialize;
use serde_json::{Value, json};

use super::compose::held_account;
use crate::params::decode_quietly;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[LIST, SAVE, DELETE];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["account", "name", "subject", "text"];

/// `template.list`: the templates of every account, or of one.
pub(crate) const LIST: CommandSpec =
    CommandSpec { id: "template.list", class: Some(AgentClass::Safe), summary: "List the templates of every account, or of one", run: list };

/// `template.save`: keeps a template on an account, replacing the one of the same name.
pub(crate) const SAVE: CommandSpec = CommandSpec {
    id: "template.save",
    class: Some(AgentClass::Owner),
    summary: "Keep a template on an account, replacing the one of the same name",
    run: save,
};

/// `template.delete`: removes a template from an account.
pub(crate) const DELETE: CommandSpec =
    CommandSpec { id: "template.delete", class: Some(AgentClass::Owner), summary: "Remove a template from an account", run: delete };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    /// Only the templates of this account.
    account: Option<u64>,
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ListParams = decode_quietly(LIST.id, params)?;
    let accounts = match params.account {
        Some(id) => vec![held_account(context, id)?],
        None => context.store().accounts()?,
    };
    let templates: Vec<Value> = accounts.iter().flat_map(|account| account.templates().iter().map(|template| json(account.id(), template))).collect();
    Ok(Outcome::new(json!({ "templates": templates })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveParams {
    /// The account.
    account: u64,
    /// Its name.
    name: String,
    /// Its subject; none when left out.
    #[serde(default)]
    subject: String,
    /// Its text.
    text: String,
}

fn save(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SaveParams = decode_quietly(SAVE.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(SAVE.id, reason);
    let too_long = params.name.chars().count() > Template::MAX_NAME_CHARS
        || params.subject.chars().count() > Template::MAX_SUBJECT_CHARS
        || params.text.chars().count() > Template::MAX_TEXT_CHARS;
    if too_long {
        return Err(bad("the name, the subject or the text is too long"));
    }
    let template = Template::new(&params.name, &params.subject, &params.text).ok_or_else(|| bad("a template needs a name"))?;
    let account = held_account(context, params.account)?;
    let mut templates = account.templates().to_vec();
    let name = template.name().to_lowercase();
    match templates.iter().position(|held| held.name().to_lowercase() == name) {
        Some(at) => templates.splice(at..=at, [template.clone()]).for_each(drop),
        None if templates.len() >= Account::MAX_TEMPLATES => return Err(bad("the account holds as many templates as it can")),
        None => templates.push(template.clone()),
    }
    let account = account.with_templates(templates);
    Ok(Outcome::new(json(account.id(), &template)).with_change(Change::PutAccount(account)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteParams {
    /// The account.
    account: u64,
    /// The template's name, without case.
    name: String,
}

fn delete(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: DeleteParams = decode_quietly(DELETE.id, params)?;
    let account = held_account(context, params.account)?;
    let gone = account.template(&params.name).cloned().ok_or_else(|| EngineError::bad_params(DELETE.id, "the account has no template of that name"))?;
    let templates = account.templates().iter().filter(|held| **held != gone).cloned().collect();
    let account = account.with_templates(templates);
    Ok(Outcome::new(json!({ "account": params.account, "deleted": gone.name() })).with_change(Change::PutAccount(account)))
}

/// A template as the commands answer it.
fn json(account: AccountId, template: &Template) -> Value {
    json!({ "account": account.get(), "name": template.name(), "subject": template.subject(), "text": template.text() })
}
