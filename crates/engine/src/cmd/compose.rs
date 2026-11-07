// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `compose`: drafts, reply, reply all, forward and templates. A new command goes into [`COMMANDS`] and its parameter
//! names into `FIELDS`.
//!
//! A draft is in the store after every command that changes it, so a kill loses nothing typed; its copy goes to the
//! server later (see `crate::draft`). Writing, changing and discarding a draft is `safe`: nothing leaves the
//! computer but the copy in the owner's own Drafts folder. The signature is a setting, so `owner`. A complaint about
//! the parameters repeats nothing that was sent, so no word of a draft lands in an error.

use inboxcraft_model::{Account, AccountId, Draft, DraftId, MessageId};
use inboxcraft_store::Change;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::draft::{self, json as draft_json};
use crate::params::decode_quietly;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[NEW, UPDATE, SHOW, LIST, DISCARD, SIGNATURE];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["id", "account", "answers", "to", "cc", "bcc", "subject", "text", "signature"];

/// The most recipients in one of `to`, `cc` and `bcc`.
pub(crate) const MAX_RECIPIENTS: usize = 500;

/// The longest recipient as typed, in bytes.
pub(crate) const MAX_RECIPIENT_BYTES: usize = 998;

/// The longest subject, in bytes.
pub(crate) const MAX_SUBJECT_BYTES: usize = 998;

/// The longest text, in bytes.
pub(crate) const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// The longest signature, in bytes.
pub(crate) const MAX_SIGNATURE_BYTES: usize = 16 * 1024;

/// `compose.new`: a new draft, kept at once.
pub(crate) const NEW: CommandSpec = CommandSpec {
    id: "compose.new",
    class: Some(AgentClass::Safe),
    summary: "Start a draft from the account that last sent, with its signature",
    run: new,
};

/// `compose.update`: changes a draft; the fields left out stay as they are.
pub(crate) const UPDATE: CommandSpec =
    CommandSpec { id: "compose.update", class: Some(AgentClass::Safe), summary: "Change a draft and keep it at once", run: update };

/// `compose.show`: one draft with its recipients checked.
pub(crate) const SHOW: CommandSpec =
    CommandSpec { id: "compose.show", class: Some(AgentClass::Safe), summary: "Show a draft with its recipients checked", run: show };

/// `compose.list`: the drafts, newest first.
pub(crate) const LIST: CommandSpec = CommandSpec { id: "compose.list", class: Some(AgentClass::Safe), summary: "List the drafts, newest first", run: list };

/// `compose.discard`: removes a draft here and its copy on the server.
pub(crate) const DISCARD: CommandSpec =
    CommandSpec { id: "compose.discard", class: Some(AgentClass::Safe), summary: "Discard a draft here and on the server", run: discard };

/// `compose.signature`: sets or clears the signature new drafts of an account get.
pub(crate) const SIGNATURE: CommandSpec = CommandSpec {
    id: "compose.signature",
    class: Some(AgentClass::Owner),
    summary: "Set or clear the signature of an account's new drafts",
    run: signature,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewParams {
    /// The account it is from; the one that last sent when left out.
    account: Option<u64>,
    /// The message it answers.
    answers: Option<u64>,
}

fn new(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: NewParams = decode_quietly(NEW.id, params)?;
    let account = match params.account {
        Some(id) => held_account(context, id)?,
        None => draft::last_sent(context)?.ok_or_else(|| EngineError::bad_params(NEW.id, "there is no account to write from"))?,
    };
    let answers = match params.answers {
        Some(id) => Some(context.store().message(MessageId::new(id))?.ok_or(EngineError::NotFound { what: "message", id })?.message().id()),
        None => None,
    };
    let draft = Draft { text: draft::signed(&account), answers, ..Draft::new(draft::next_id(context.state()), account.id(), context.now()) };
    Ok(Outcome::new(draft_json(&draft)).with_change(Change::PutDraft(draft)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateParams {
    /// The draft.
    id: u64,
    /// The account it is from.
    account: Option<u64>,
    /// `To`, as typed.
    to: Option<Vec<String>>,
    /// `Cc`, as typed.
    cc: Option<Vec<String>>,
    /// `Bcc`, as typed.
    bcc: Option<Vec<String>>,
    /// The subject.
    subject: Option<String>,
    /// The text.
    text: Option<String>,
}

fn update(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: UpdateParams = decode_quietly(UPDATE.id, params)?;
    for list in [&params.to, &params.cc, &params.bcc].into_iter().flatten() {
        if list.len() > MAX_RECIPIENTS || list.iter().any(|text| text.len() > MAX_RECIPIENT_BYTES) {
            return Err(EngineError::bad_params(UPDATE.id, "too many recipients, or one too long"));
        }
    }
    if params.subject.as_ref().is_some_and(|subject| subject.len() > MAX_SUBJECT_BYTES) {
        return Err(EngineError::bad_params(UPDATE.id, "the subject is too long"));
    }
    if params.text.as_ref().is_some_and(|text| text.len() > MAX_TEXT_BYTES) {
        return Err(EngineError::bad_params(UPDATE.id, "the text is too long"));
    }
    let mut draft = held_draft(context, params.id)?;
    let mut outcome = Outcome::new(Value::Null);
    draft.to = params.to.unwrap_or(draft.to);
    draft.cc = params.cc.unwrap_or(draft.cc);
    draft.bcc = params.bcc.unwrap_or(draft.bcc);
    draft.subject = params.subject.unwrap_or(draft.subject);
    draft.text = params.text.unwrap_or(draft.text);
    if let Some(id) = params.account.filter(|id| *id != draft.account.get()) {
        let to = held_account(context, id)?;
        let from = held_account(context, draft.account.get())?;
        draft.text = draft::resigned(&draft.text, &from, &to);
        if let Some(copy) = draft.uploaded.take()
            && let Some(kind) = draft::take_back(context.store(), from.id(), copy)?
        {
            outcome = outcome.with_kept_job(from.id(), kind);
        }
        draft.synced = None;
        draft.account = to.id();
    }
    draft.saved = context.now();
    *outcome.value_mut() = draft_json(&draft);
    Ok(outcome.with_change(Change::PutDraft(draft)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    /// The draft.
    id: u64,
}

fn show(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode_quietly(SHOW.id, params)?;
    Ok(Outcome::new(draft_json(&held_draft(context, params.id)?)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    /// Only the drafts of this account.
    account: Option<u64>,
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ListParams = decode_quietly(LIST.id, params)?;
    let accounts = crate::view::accounts(context, params.account.map(AccountId::new))?;
    let mut drafts = Vec::new();
    for account in accounts {
        drafts.extend(context.store().drafts(account)?);
    }
    drafts.sort_by(|a, b| b.saved.cmp(&a.saved).then(b.id.cmp(&a.id)));
    Ok(Outcome::new(json!({ "drafts": drafts.iter().map(draft_json).collect::<Vec<_>>() })))
}

fn discard(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode_quietly(DISCARD.id, params)?;
    let draft = held_draft(context, params.id)?;
    let mut outcome = Outcome::new(json!({ "discarded": params.id })).with_change(Change::RemoveDraft(draft.id));
    if let Some(copy) = draft.uploaded
        && let Some(kind) = draft::take_back(context.store(), draft.account, copy)?
    {
        outcome = outcome.with_kept_job(draft.account, kind);
    }
    Ok(outcome)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignatureParams {
    /// The account.
    account: u64,
    /// The signature; none when left out.
    signature: Option<String>,
}

fn signature(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SignatureParams = decode_quietly(SIGNATURE.id, params)?;
    if params.signature.as_ref().is_some_and(|text| text.len() > MAX_SIGNATURE_BYTES) {
        return Err(EngineError::bad_params(SIGNATURE.id, "the signature is too long"));
    }
    let account = held_account(context, params.account)?.with_signature(params.signature.as_deref());
    let answer = json!({ "account": params.account, "signature": account.signature() });
    Ok(Outcome::new(answer).with_change(Change::PutAccount(account)))
}

fn held_account(context: &Context<'_>, id: u64) -> Result<Account, EngineError> {
    context.store().accounts()?.into_iter().find(|account| account.id().get() == id).ok_or(EngineError::NotFound { what: "account", id })
}

fn held_draft(context: &Context<'_>, id: u64) -> Result<Draft, EngineError> {
    context.store().draft(DraftId::new(id))?.ok_or(EngineError::NotFound { what: "draft", id })
}
