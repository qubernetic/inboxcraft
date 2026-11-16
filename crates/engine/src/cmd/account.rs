// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `account.*`: the accounts. No command takes a password: the front end keeps it in the OS credential store.

use std::num::NonZeroU16;

use inboxcraft_model::{Account, AccountId, CheckStep, EmailAddress, Host, JobKind, Security, Server};
use inboxcraft_store::Change;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use crate::params::{NoParams, decode, decode_quietly};
use crate::{AgentClass, Caller, CommandSpec, Context, EngineError, Outcome, json};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[ADD, CHECK, LIST, REMOVE, UPDATE];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["account", "address", "label", "display_name", "username", "imap", "submission", "host", "port", "security", "dry_run", "recheck"];

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
/// IMAP sign-in, the SMTP sign-in; CalDAV is listed as not built yet). The answer lists the steps as they stand; the worker
/// reports each as it ends, and `account.list` shows them. While a check of the account runs, another one starts
/// nothing (`started`: false), so the password is not sent over and over. After a refused or unanswered sign-in, a
/// check an agent asked for stops before the sign-in (`needs_owner`); only the owner's check sends the password again.
/// The submission server is held so on its own: its sign-in alone waits, and the IMAP steps run.
pub(crate) const CHECK: CommandSpec = CommandSpec {
    id: "account.check",
    class: Some(AgentClass::Safe),
    summary: "Check an account step by step and name the cause of a failure",
    run: check,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountParams {
    /// The account.
    account: u64,
}

fn check(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: AccountParams = decode(CHECK.id, params)?;
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

/// `account.add`: adds an account from settings the owner entered or saw (Detect only proposes them), its IMAP server
/// and, to send, its submission server, and starts its first check, as the owner's. The password is no parameter: the front end puts it into the OS credential store under
/// the new account's id, and the worker reads it from there. A complaint about the parameters repeats nothing that was
/// sent, so a password typed into the wrong field does not come back in an error. Adding an account changes what
/// leaves the computer, so only the owner may.
pub(crate) const ADD: CommandSpec = CommandSpec {
    id: "account.add",
    class: Some(AgentClass::Owner),
    summary: "Add a mail account from its settings and check it step by step",
    run: add,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddParams {
    /// The account's address.
    address: String,
    /// The owner's name for the account; the address when left out.
    #[serde(default)]
    label: Option<String>,
    /// The owner's name on sent mail.
    #[serde(default)]
    display_name: Option<String>,
    /// The login name; the address when left out.
    #[serde(default)]
    username: Option<String>,
    /// The IMAP server.
    imap: ServerParams,
    /// The submission (SMTP) server, which sends; none when left out.
    #[serde(default)]
    submission: Option<ServerParams>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerParams {
    host: String,
    /// The usual port of `security` when left out.
    #[serde(default)]
    port: Option<NonZeroU16>,
    security: SecurityParam,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SecurityParam {
    Tls,
    #[serde(alias = "starttls")]
    StartTls,
}

/// The server `given` describes, on the usual port of its security when it names none.
fn server(given: &ServerParams, port: fn(Security) -> NonZeroU16, bad: &dyn Fn(&str) -> EngineError) -> Result<Server, EngineError> {
    let host = Host::new(&given.host).map_err(|error| bad(&error.to_string()))?;
    let security = match given.security {
        SecurityParam::Tls => Security::Tls,
        SecurityParam::StartTls => Security::StartTls,
    };
    Ok(Server::new(host, given.port.unwrap_or_else(|| port(security)), security))
}

fn add(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: AddParams = decode_quietly(ADD.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(ADD.id, reason);
    let address = EmailAddress::new(params.address.trim()).map_err(|error| bad(&error.to_string()))?;
    let imap = server(&params.imap, Security::imap_port, &bad)?;
    let submission = params.submission.as_ref().map(|given| server(given, Security::submission_port, &bad)).transpose()?;
    let username = params.username.unwrap_or_else(|| address.as_str().to_owned());
    let accounts = context.store().accounts()?;
    let account = Account::new(context.state().next_account(&accounts), params.label.as_deref().unwrap_or(""), address, &username, imap)
        .map_err(|error| bad(&error.to_string()))?
        .with_display_name(params.display_name.as_deref())
        .with_submission(submission);
    if accounts.iter().any(|held| held.imap().host() == account.imap().host() && held.username() == account.username()) {
        return Err(bad("an account with this user name on this server is already there"));
    }
    let answer = json!({ "account": json::account(&account, None), "started": true, "steps": json::steps(&CheckStep::plan()) });
    let id = account.id();
    Ok(Outcome::new(answer).with_change(Change::PutAccount(account)).with_job(id, JobKind::Check { by_owner: true }))
}

/// `account.update`: changes the name, the name on sent mail, the user name and the IMAP and submission servers of an
/// account, validated as `account.add` validates them; what is left out stays, and `null` clears the name on sent mail
/// or the submission server. A change of the user name, or of a server's host, port or security, means the stored
/// password would go somewhere new, so it starts the account's check, as the owner's, and the next job reaches a worker
/// that holds the new settings; when its sign-in passes, the sync starts again. The password is no parameter, and a
/// complaint repeats nothing that was sent. `recheck` starts that check with no such change: the front end has just
/// stored a new password, or moved the CalDAV address. With `dry_run` nothing changes and no check starts: the front
/// end asks first, before it replaces a password.
pub(crate) const UPDATE: CommandSpec = CommandSpec {
    id: "account.update",
    class: Some(AgentClass::Owner),
    summary: "Change a mail account's settings, and check it again when where it signs in changes",
    run: update,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateParams {
    /// The account.
    account: u64,
    /// The owner's name for the account; empty is the address.
    #[serde(default)]
    label: Option<String>,
    /// The owner's name on sent mail; `null` clears it.
    #[serde(default, deserialize_with = "given")]
    display_name: Option<Option<String>>,
    /// The login name.
    #[serde(default)]
    username: Option<String>,
    /// The IMAP server.
    #[serde(default)]
    imap: Option<ServerParams>,
    /// The submission (SMTP) server; `null` clears it.
    #[serde(default, deserialize_with = "given")]
    submission: Option<Option<ServerParams>>,
    /// Validate only.
    #[serde(default)]
    dry_run: bool,
    /// Start the check even when nothing here moved the sign-in.
    #[serde(default)]
    recheck: bool,
}

/// A field that can be left out (the outer `None`) or set to `null` (the inner one).
fn given<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

fn update(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: UpdateParams = decode_quietly(UPDATE.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(UPDATE.id, reason);
    let id = AccountId::new(params.account);
    let accounts = context.store().accounts()?;
    let held = accounts.iter().find(|account| account.id() == id).ok_or(EngineError::NotFound { what: "account", id: params.account })?;
    let imap = params.imap.as_ref().map(|given| server(given, Security::imap_port, &bad)).transpose()?;
    let submission = match &params.submission {
        Some(Some(given)) => Some(server(given, Security::submission_port, &bad)?),
        Some(None) => None,
        None => held.submission().cloned(),
    };
    let display_name = params.display_name.as_ref().map_or_else(|| held.display_name(), Option::as_deref);
    let changed = held
        .clone()
        .with_sign_in(
            params.label.as_deref().unwrap_or_else(|| held.label()),
            params.username.as_deref().unwrap_or_else(|| held.username()),
            imap.unwrap_or_else(|| held.imap().clone()),
        )
        .map_err(|error| bad(&error.to_string()))?
        .with_display_name(display_name)
        .with_submission(submission);
    if accounts.iter().any(|other| other.id() != id && other.imap().host() == changed.imap().host() && other.username() == changed.username()) {
        return Err(bad("an account with this user name on this server is already there"));
    }
    let moved = held.username() != changed.username() || held.imap() != changed.imap() || held.submission() != changed.submission();
    let check = moved || params.recheck;
    let answer = json!({
        "account": json::account(&changed, context.state().accounts.get(&id)),
        "started": check && !params.dry_run,
        "steps": json::steps(&CheckStep::plan()),
    });
    if params.dry_run {
        return Ok(Outcome::new(answer));
    }
    let outcome = Outcome::new(answer).with_change(Change::PutAccount(changed));
    Ok(if check { outcome.with_job(id, JobKind::Check { by_owner: true }).with_resync(id) } else { outcome })
}

/// `account.remove`: removes an account with its folders, messages and kept bodies, and forgets its connection, its
/// check and its queued jobs. The password stays where it is: the front end removes it from the OS credential store.
pub(crate) const REMOVE: CommandSpec =
    CommandSpec { id: "account.remove", class: Some(AgentClass::Owner), summary: "Remove a mail account with the mail kept of it", run: remove };

fn remove(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: AccountParams = decode(REMOVE.id, params)?;
    let id = AccountId::new(params.account);
    if !context.store().accounts()?.iter().any(|account| account.id() == id) {
        return Err(EngineError::NotFound { what: "account", id: params.account });
    }
    Ok(Outcome::new(json!({ "account": params.account, "removed": true })).with_removed(id))
}
