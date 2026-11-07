// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `schedule.*`: the time presets, the scheduler's entries, and cancelling one. The entries are made by the commands
//! that schedule something (send later, snooze, follow-up, calendar alerts) and fire through
//! [`crate::Session::tick`].

use inboxcraft_model::{Due, ScheduleId};
use inboxcraft_store::Change;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::params::{NoParams, decode};
use crate::schedule::presets as presets_at;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, json};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[PRESETS, LIST, CANCEL];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["id"];

/// `schedule.presets`: the time presets still ahead, in the computer's wall-clock time.
pub(crate) const PRESETS: CommandSpec = CommandSpec {
    id: "schedule.presets",
    class: Some(AgentClass::Safe),
    summary: "List the time presets still ahead: later today, this evening, tomorrow morning",
    run: presets,
};

/// `schedule.list`: every schedule entry, by its time.
pub(crate) const LIST: CommandSpec =
    CommandSpec { id: "schedule.list", class: Some(AgentClass::Safe), summary: "List the scheduled entries by their time", run: list };

/// `schedule.cancel`: takes one entry off the schedule; what it named stays as it is.
pub(crate) const CANCEL: CommandSpec =
    CommandSpec { id: "schedule.cancel", class: Some(AgentClass::Owner), summary: "Cancel a scheduled entry", run: cancel };

fn presets(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(PRESETS.id, params)?;
    let presets: Vec<Value> =
        presets_at(context.now(), context.offsets()).into_iter().map(|(preset, at)| json!({ "preset": preset.name(), "at": json::date(at) })).collect();
    Ok(Outcome::new(json!({ "presets": presets })))
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(LIST.id, params)?;
    let mut entries = Vec::new();
    for entry in context.store().schedule()? {
        let mut fields = Map::new();
        fields.insert("id".to_owned(), json!(entry.id.get()));
        fields.insert("due".to_owned(), json::date(entry.due));
        if let Value::Object(what) = what(&entry.what) {
            fields.extend(what);
        }
        entries.push(Value::Object(fields));
    }
    Ok(Outcome::new(json!({ "entries": entries })))
}

/// What an entry does, and the id of what it names.
pub(crate) fn what(due: &Due) -> Value {
    match due {
        Due::Send(draft) => json!({ "what": "send", "draft": draft.get() }),
        Due::Unsnooze(message) => json!({ "what": "unsnooze", "message": message.get() }),
        Due::FollowUp(message) => json!({ "what": "follow_up", "message": message.get() }),
        Due::Alert(alert) => json!({ "what": "alert", "alert": alert.get() }),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelParams {
    /// The entry.
    id: u64,
}

fn cancel(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: CancelParams = decode(CANCEL.id, params)?;
    let id = ScheduleId::new(params.id);
    if !context.store().schedule()?.iter().any(|entry| entry.id == id) {
        return Err(EngineError::NotFound { what: "schedule entry", id: params.id });
    }
    Ok(Outcome::new(json!({ "cancelled": params.id })).with_change(Change::RemoveScheduled(id)))
}
