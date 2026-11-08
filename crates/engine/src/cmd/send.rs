// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `send`: sending a draft, undo send and send later. A new command goes into [`COMMANDS`] and its parameter names
//! into `FIELDS`.
//!
//! `mail.send` (`owner`: mail leaves the computer) puts the draft in the outbox, waiting for the owner's hold (5 to
//! 60 seconds, `send.set`), and `mail.send_later` (`owner`) until a time the owner picks; `mail.unsend` (`owner`)
//! takes it back until then, its draft as it was. At its time the draft is made into a message with its own
//! `Message-ID` and `Date` and handed to its account's worker, which sends it through the submission server (see
//! `crate::send`). Bcc recipients are in the envelope alone, never in a header. A forward takes the attachments of
//! the message it forwards from its kept body; one whose body is gone is refused. What the worker answers lands in
//! `crate::send`: once sent, the draft and its copy in the Drafts folder go and the message is appended to the Sent
//! folder; a failure keeps the draft, its outbox item showing the server's reply. Nothing goes while the computer is
//! off or InboxCraft is closed: a time missed then is sent at the next start, with a notice.
//!
//! The answer to an invitation (`invite.answer`) goes at once, through [`sending`], with no hold: it is taken back by
//! changing the answer, not by `mail.unsend`.

use inboxcraft_model::{AccountId, Draft, DraftId, Due, JobKind, OutboxState, Outgoing};
use inboxcraft_store::Change;
use inboxcraft_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::params::{NoParams, decode, decode_quietly, parse_date};
use crate::schedule::{presets, reschedule};
use crate::send::{Hold, OFF, ready};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, draft, json};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[SEND, SEND_LATER, UNSEND, SETTINGS, SET];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["id", "at", "preset", "hold_seconds"];

/// `mail.send`: sends a draft through its account's submission server once the hold is over.
pub(crate) const SEND: CommandSpec = CommandSpec {
    id: "mail.send",
    class: Some(AgentClass::Owner),
    summary: "Send a draft through its account's submission server, after the hold in which it can be taken back",
    run: send,
};

/// `mail.send_later`: sends a draft at a time the owner picks.
pub(crate) const SEND_LATER: CommandSpec =
    CommandSpec { id: "mail.send_later", class: Some(AgentClass::Owner), summary: "Send a draft at a later time", run: send_later };

/// `mail.unsend`: takes a send back while it waits, its draft as it was.
pub(crate) const UNSEND: CommandSpec = CommandSpec {
    id: "mail.unsend",
    class: Some(AgentClass::Owner),
    summary: "Take a send back while it waits for its hold or its time, the draft as it was",
    run: unsend,
};

/// `send.settings`: how long a send is held.
pub(crate) const SETTINGS: CommandSpec =
    CommandSpec { id: "send.settings", class: Some(AgentClass::Safe), summary: "Show how long every send is held before it goes", run: settings };

/// `send.set`: changes how long a send is held.
pub(crate) const SET: CommandSpec =
    CommandSpec { id: "send.set", class: Some(AgentClass::Owner), summary: "Change how long every send is held, 5 to 60 seconds", run: set };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    /// The draft.
    id: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LaterParams {
    /// The draft.
    id: u64,
    /// The time it goes: `2026-11-05T18:00:00Z`, with an offset, or a date (midnight UTC).
    at: Option<String>,
    /// Or a preset of `schedule.presets`: `later_today`, `this_evening`, `tomorrow_morning`.
    preset: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    /// How long every send is held, in seconds.
    hold_seconds: u64,
}

fn send(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode_quietly(SEND.id, params)?;
    let hold = context.state().hold;
    let at = context.now().checked_add(hold.duration()).ok_or_else(|| EngineError::bad_params(SEND.id, "the clock is out of range"))?;
    let mut outcome = waiting(context, SEND.id, params.id, at)?;
    if let Value::Object(answer) = outcome.value_mut() {
        answer.insert("hold_seconds".to_owned(), json!(hold.seconds()));
    }
    Ok(outcome)
}

fn send_later(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: LaterParams = decode(SEND_LATER.id, params)?;
    let at = time(context, params.at.as_deref(), params.preset.as_deref())?;
    let mut outcome = waiting(context, SEND_LATER.id, params.id, at)?;
    if let Value::Object(answer) = outcome.value_mut() {
        answer.insert("note".to_owned(), json!(OFF));
    }
    Ok(outcome)
}

/// The outcome putting the draft `id` in the outbox, waiting until `at`: refused at once when it cannot go now.
fn waiting(context: &Context<'_>, command: &'static str, id: u64, at: Timestamp) -> Result<Outcome, EngineError> {
    let bad = |reason: &str| EngineError::bad_params(command, reason);
    let draft = held(context, id)?;
    if let Some(block) = draft::blocks(context.store(), &draft)?.first() {
        return Err(bad(&format!("the draft cannot be sent: {block}")));
    }
    if draft::on_its_way(context.store(), draft.id)? {
        return Err(bad("the draft is on its way already"));
    }
    ready(context, &draft)?.map_err(bad)?;
    let answer = json!({ "id": id, "state": "waiting", "at": json::date(at) });
    let mut outcome = Outcome::new(answer).with_change(Change::PutOutgoing(Outgoing::new(draft.id, OutboxState::Waiting)));
    for change in reschedule(context.store(), Due::Send(draft.id), Some(at), context.next_schedule_id())? {
        outcome = outcome.with_change(change);
    }
    Ok(outcome)
}

/// What sends a draft at once: the answer, its outbox item on its way and the job for its account's worker.
pub(crate) struct Sending {
    pub(crate) answer: Value,
    pub(crate) outgoing: Change,
    pub(crate) account: AccountId,
    pub(crate) job: JobKind,
}

/// What sends `draft` now, with no hold, checked as `mail.send` checks it (`command` names it in a refusal): it can
/// go, is not on its way already, and its account has a submission server.
pub(crate) fn sending(context: &Context<'_>, command: &'static str, draft: &Draft) -> Result<Sending, EngineError> {
    let bad = |reason: &str| EngineError::bad_params(command, reason);
    if let Some(block) = draft::blocks(context.store(), draft)?.first() {
        return Err(bad(&format!("the draft cannot be sent: {block}")));
    }
    if draft::on_its_way(context.store(), draft.id)? {
        return Err(bad("the draft is on its way already"));
    }
    let ready = ready(context, draft)?.map_err(bad)?;
    let recipients = match &ready.job {
        JobKind::Send { recipients, .. } => recipients.len(),
        _ => 0,
    };
    let answer = json!({ "id": draft.id.get(), "recipients": recipients, "state": "sending" });
    Ok(Sending { answer, outgoing: Change::PutOutgoing(ready.outgoing), account: ready.account, job: ready.job })
}

fn unsend(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode_quietly(UNSEND.id, params)?;
    let draft = held(context, params.id)?;
    let item = context.store().outbox()?.into_iter().find(|item| item.draft == draft.id);
    match item.map(|item| item.state) {
        None => Err(EngineError::NotFound { what: "send waiting", id: params.id }),
        Some(OutboxState::Waiting) => Ok(taken_back(context, &draft)?),
        Some(_) => Err(EngineError::bad_params(UNSEND.id, "only a send that waits is taken back: this one went")),
    }
}

/// The outcome taking the waiting send of `draft` back: off the schedule and out of the outbox, the draft as it was.
pub(crate) fn taken_back(context: &Context<'_>, draft: &Draft) -> Result<Outcome, EngineError> {
    let answer = json!({ "unsent": draft.id.get(), "draft": draft::json(context.store(), draft)? });
    let mut outcome = Outcome::new(answer).with_change(Change::RemoveOutgoing(draft.id));
    for change in reschedule(context.store(), Due::Send(draft.id), None, context.next_schedule_id())? {
        outcome = outcome.with_change(change);
    }
    Ok(outcome)
}

fn settings(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(SETTINGS.id, params)?;
    let hold = context.state().hold;
    Ok(Outcome::new(json!({ "hold_seconds": hold.seconds(), "min": Hold::MIN, "max": Hold::MAX, "note": OFF })))
}

fn set(_: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SetParams = decode(SET.id, params)?;
    let hold = Hold::new(params.hold_seconds).ok_or_else(|| EngineError::bad_params(SET.id, "a send is held 5 to 60 seconds"))?;
    Ok(Outcome::new(json!({ "hold_seconds": hold.seconds() })).with_hold(hold))
}

/// The draft `id`.
fn held(context: &Context<'_>, id: u64) -> Result<Draft, EngineError> {
    context.store().draft(DraftId::new(id))?.ok_or(EngineError::NotFound { what: "draft", id })
}

/// The time of a send later: `at` or a preset still ahead, after now.
fn time(context: &Context<'_>, at: Option<&str>, preset: Option<&str>) -> Result<Timestamp, EngineError> {
    let bad = |reason: &str| EngineError::bad_params(SEND_LATER.id, reason);
    let now = context.now();
    let at = match (at, preset) {
        (Some(text), None) => parse_date(text.trim()).ok_or_else(|| bad("at is a time such as 2026-11-05T18:00:00Z"))?,
        (None, Some(name)) => presets(now, context.offsets())
            .into_iter()
            .find(|(preset, _)| preset.name() == name)
            .map(|(_, at)| at)
            .ok_or_else(|| bad("the preset is one of schedule.presets still ahead"))?,
        _ => return Err(bad("give at or a preset")),
    };
    if at <= now {
        return Err(bad("the time has passed"));
    }
    Ok(at)
}
