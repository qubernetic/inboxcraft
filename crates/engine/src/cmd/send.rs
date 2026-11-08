// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `send`: sending a draft, undo send and send later. A new command goes into [`COMMANDS`] and its parameter names
//! into `FIELDS`.
//!
//! `mail.send` (`owner`: mail leaves the computer) makes the draft into a message with its own `Message-ID` and
//! `Date`, puts it in the outbox and hands it to its account's worker, which sends it through the submission server.
//! Bcc recipients are in the envelope alone, never in a header. A forward takes the attachments of the message it
//! forwards from its kept body; one whose body is gone is refused. What the worker answers lands in `crate::send`:
//! once sent, the draft and its copy in the Drafts folder go and the message is appended to the Sent folder; a
//! failure keeps the draft, its outbox item showing the server's reply. The answer to an invitation (`invite.answer`)
//! goes out the same way, through [`sending`].

use std::collections::BTreeSet;

use inboxcraft_model::{AccountId, Draft, DraftId, EmailAddress, JobKind, OutboxState, Outgoing};
use inboxcraft_store::Change;
use inboxcraft_time::{MailDate, Zone};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{draft, reply};
use crate::params::decode_quietly;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[SEND];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["id"];

/// `mail.send`: sends a draft through its account's submission server.
pub(crate) const SEND: CommandSpec =
    CommandSpec { id: "mail.send", class: Some(AgentClass::Owner), summary: "Send a draft through its account's submission server", run: send };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendParams {
    /// The draft.
    id: u64,
}

fn send(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SendParams = decode_quietly(SEND.id, params)?;
    let draft = context.store().draft(DraftId::new(params.id))?.ok_or(EngineError::NotFound { what: "draft", id: params.id })?;
    let sending = sending(context, SEND.id, &draft)?;
    Ok(Outcome::new(sending.answer).with_change(sending.outgoing).with_job(sending.account, sending.job))
}

/// What sends a draft: the answer, its outbox item and the job for its account's worker.
pub(crate) struct Sending {
    pub(crate) answer: Value,
    pub(crate) outgoing: Change,
    pub(crate) account: AccountId,
    pub(crate) job: JobKind,
}

/// What sends `draft`, held or about to be, checked as `mail.send` checks it (`command` names it in a refusal): it can
/// go, is not on its way already, and its account has a submission server.
pub(crate) fn sending(context: &Context<'_>, command: &'static str, draft: &Draft) -> Result<Sending, EngineError> {
    let bad = |reason: &str| EngineError::bad_params(command, reason);
    if let Some(block) = draft::blocks(context.store(), draft)?.first() {
        return Err(bad(&format!("the draft cannot be sent: {block}")));
    }
    if draft::on_its_way(context.store(), draft.id)? {
        return Err(bad("the draft is on its way already"));
    }
    let account = context
        .store()
        .accounts()?
        .into_iter()
        .find(|account| account.id() == draft.account)
        .ok_or(EngineError::NotFound { what: "account", id: draft.account.get() })?;
    if account.submission().is_none() {
        return Err(bad("the account has no submission server"));
    }
    let now = context.now();
    let message_id = message_id(draft, account.address(), now);
    let date = MailDate::new(now, Zone::Fixed(context.offsets().offset_at(now)));
    let attachments = reply::forwarded(context.store(), draft)?.ok_or_else(|| bad("the message it forwards is no longer kept, so its attachments are not"))?;
    let raw = draft::built(&account, draft, date, Some(&message_id), &attachments).map_err(|_| bad("the draft cannot be made into a message"))?;
    let recipients = envelope(draft);
    let answer = json!({ "id": draft.id.get(), "message_id": message_id, "recipients": recipients.len(), "state": "sending" });
    let outgoing = Outgoing { message: raw.clone(), ..Outgoing::new(draft.id, OutboxState::Sending) };
    let job = JobKind::Send { draft: draft.id, from: account.address().clone(), recipients, raw };
    Ok(Sending { answer, outgoing: Change::PutOutgoing(outgoing), account: account.id(), job })
}

/// The envelope's recipients: To, Cc and Bcc, each address once, in that order.
fn envelope(draft: &Draft) -> Vec<EmailAddress> {
    let mut seen = BTreeSet::new();
    draft
        .to
        .iter()
        .chain(&draft.cc)
        .chain(&draft.bcc)
        .filter_map(|text| draft::recipient(text).ok().map(|(_, address)| address))
        .filter(|address| seen.insert(address.as_str().to_lowercase()))
        .collect()
}

/// A `Message-ID` of our own: the time to the nanosecond and the draft, at the domain of the sender's address (or
/// `invalid` where it is not ASCII), so no two sends share one and it tells nothing of the computer.
fn message_id(draft: &Draft, from: &EmailAddress, now: inboxcraft_time::Timestamp) -> String {
    let domain = Some(from.domain().to_ascii_lowercase()).filter(|domain| domain.is_ascii() && !domain.is_empty()).unwrap_or_else(|| "invalid".to_owned());
    format!("<{}.{:09}.{}@{domain}>", now.unix_seconds(), now.subsec_nanos(), draft.id.get())
}
