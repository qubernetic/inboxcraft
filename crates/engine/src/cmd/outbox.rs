// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `outbox`: what waits to be sent, refused sends and bounces. A new command goes into [`COMMANDS`] and its parameter
//! names into `FIELDS`.
//!
//! `outbox.list` (`safe`) shows each outbox item with where it stands: `waiting`, `sending`, or `refused` with the
//! server's reply, its enhanced status code (RFC 3463) where the reply has one, and whether it is permanent. A refused
//! draft is kept as it was; `outbox.retry` (`owner`: mail leaves the computer) sends it again as `mail.send` does. A
//! message that went but bounced later is marked on its Sent copy (`crate::arrive::bounce`), not here.

use inboxcraft_mime::Status;
use inboxcraft_model::{DraftId, OutboxState};
use serde::Deserialize;
use serde_json::{Value, json};

use super::send::SEND;
use crate::params::{NoParams, decode, decode_quietly};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[LIST, RETRY];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["id"];

/// `outbox.list`: every outbox item and where it stands.
pub(crate) const LIST: CommandSpec = CommandSpec {
    id: "outbox.list",
    class: Some(AgentClass::Safe),
    summary: "List the outbox: what waits, what is on its way and what the server refused, with its reply",
    run: list,
};

/// `outbox.retry`: sends a refused draft again.
pub(crate) const RETRY: CommandSpec =
    CommandSpec { id: "outbox.retry", class: Some(AgentClass::Owner), summary: "Send a refused draft again", run: retry };

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(LIST.id, params)?;
    let store = context.store();
    let mut items = Vec::new();
    for item in store.outbox()? {
        let draft = store.draft(item.draft)?;
        let (state, reply) = match &item.state {
            OutboxState::Waiting => ("waiting", None),
            OutboxState::Sending => ("sending", None),
            OutboxState::Refused { reply } => ("refused", Some(reply.as_str())),
        };
        let code = reply.and_then(enhanced_code);
        items.push(json!({
            "draft": item.draft.get(),
            "account": draft.as_ref().map(|draft| draft.account.get()),
            "subject": draft.as_ref().map(|draft| draft.subject.as_str()),
            "to": draft.as_ref().map(|draft| draft.to.as_slice()),
            "state": state,
            "reply": reply,
            "code": code.map(|code| code.to_string()),
            "permanent": reply.is_some_and(|reply| permanent(reply, code)),
        }));
    }
    Ok(Outcome::new(json!({ "items": items })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryParams {
    /// The draft.
    id: u64,
}

fn retry(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: RetryParams = decode_quietly(RETRY.id, params)?;
    let item = context
        .store()
        .outbox()?
        .into_iter()
        .find(|item| item.draft == DraftId::new(params.id))
        .ok_or(EngineError::NotFound { what: "outbox item", id: params.id })?;
    if !matches!(item.state, OutboxState::Refused { .. }) {
        return Err(EngineError::bad_params(RETRY.id, "only a refused draft is sent again"));
    }
    (SEND.run)(context, &json!({ "id": params.id }))
}

/// The reply code an SMTP reply starts with (`550`), if it does.
fn reply_code(reply: &str) -> Option<&str> {
    reply.split_whitespace().next().filter(|code| code.len() == 3 && code.bytes().all(|b| b.is_ascii_digit()))
}

/// The enhanced status code of an SMTP reply (`550 5.1.1 No such user`): the word after the reply code.
fn enhanced_code(reply: &str) -> Option<Status> {
    reply_code(reply)?;
    reply.split_whitespace().nth(1).and_then(Status::parse)
}

/// Whether the refusal in `reply` stands however often the message is sent as it is: a permanent enhanced code, or a
/// `5xx` reply without one.
fn permanent(reply: &str, code: Option<Status>) -> bool {
    code.map_or_else(|| reply_code(reply).is_some_and(|code| code.starts_with('5')), Status::is_permanent_failure)
}
