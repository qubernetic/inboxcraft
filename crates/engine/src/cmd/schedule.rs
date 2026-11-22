// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `schedule.*`: the time presets, the scheduler's entries, and cancelling one. The entries are made by the commands
//! that schedule something (send later, snooze, follow-up, calendar alerts) and fire through
//! [`crate::Session::tick`].

use inboxcraft_model::{AccountId, Due, ScheduleId, Scheduled};
use inboxcraft_store::Change;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::params::{NoParams, decode};
use crate::schedule::presets as presets_at;
use crate::notify::{self, line};
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
    summary: "List the time presets still ahead: later today, this evening, tomorrow morning, this weekend, next week",
    run: presets,
};

/// `schedule.list`: every schedule entry, by its time.
pub(crate) const LIST: CommandSpec =
    CommandSpec { id: "schedule.list", class: Some(AgentClass::Safe), summary: "List the scheduled entries by their time", run: list };

/// `schedule.cancel`: takes one entry off the schedule; what it named stays as it is, but a send is taken back as
/// `mail.unsend` does, so its draft does not wait for a time that never comes.
pub(crate) const CANCEL: CommandSpec =
    CommandSpec { id: "schedule.cancel", class: Some(AgentClass::Owner), summary: "Cancel a scheduled entry", run: cancel };

fn presets(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(PRESETS.id, params)?;
    let presets: Vec<Value> =
        presets_at(context.now(), context.offsets(), context.state().week).into_iter().map(|(preset, at)| json!({ "preset": preset.name(), "at": json::date(at) })).collect();
    Ok(Outcome::new(json!({ "presets": presets })))
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(LIST.id, params)?;
    let mut entries = Vec::new();
    // An entry whose draft, message or alert the store cannot read is left out and counted, not the whole list refused.
    let mut skipped: u64 = 0;
    for entry in context.store().schedule()? {
        let Ok(about) = about(context, &entry) else {
            skipped = skipped.saturating_add(1);
            continue;
        };
        let mut fields = Map::new();
        fields.insert("id".to_owned(), json!(entry.id.get()));
        fields.insert("due".to_owned(), json::date(entry.due));
        if let Value::Object(what) = what(&entry.what) {
            fields.extend(what);
        }
        fields.extend(about);
        entries.push(Value::Object(fields));
    }
    Ok(Outcome::new(json!({ "entries": entries, "skipped": skipped })))
}

/// What a row of the scheduled list shows of `entry`, read from the store: the account, the subject (an alert's event
/// title) and who (a send's recipients, a snooze's sender, a follow-up's recipient), each one short line without
/// control characters or invisible marks (the sender's data, never instructions); an alert adds its event, when the
/// occurrence starts and how many minutes before. What the store no longer holds answers no account and empty text.
fn about(context: &Context<'_>, entry: &Scheduled) -> Result<Map<String, Value>, EngineError> {
    let store = context.store();
    let (account, subject, who) = match entry.what {
        Due::Send(draft) => match store.draft(draft)? {
            Some(draft) => (Some(draft.account), line(&draft.subject), recipients(&draft.to)),
            None => (None, String::new(), String::new()),
        },
        Due::Unsnooze(message) | Due::FollowUp(message) => match store.message(message)? {
            Some(entry_of) => {
                let message = entry_of.message();
                let who = notify::whom(message, matches!(entry.what, Due::Unsnooze(_)));
                (Some(message.account()), line(message.summary().envelope().subject()), who)
            }
            None => (None, String::new(), String::new()),
        },
        Due::Alert(alert) => {
            let mut fields = Map::new();
            let about = notify::alert_about(store, alert, (context.offsets(), context.state().all_day_alert), context.now())?;
            let before = about.as_ref().and_then(|about| about.start.duration_since(entry.due)).map(|lead| lead.as_secs() / 60);
            fields.insert("account".to_owned(), json!(about.as_ref().map(|about| about.account.get())));
            fields.insert("subject".to_owned(), json!(about.as_ref().map(|about| about.summary.as_str()).unwrap_or_default()));
            fields.insert("who".to_owned(), json!(""));
            fields.insert("event".to_owned(), json!(about.as_ref().map(|about| about.event.get())));
            fields.insert("starts".to_owned(), about.as_ref().map_or(Value::Null, |about| json::date(about.start)));
            fields.insert("before_minutes".to_owned(), json!(before));
            return Ok(fields);
        }
    };
    let mut fields = Map::new();
    fields.insert("account".to_owned(), json!(account.map(AccountId::get)));
    fields.insert("subject".to_owned(), json!(subject));
    fields.insert("who".to_owned(), json!(who));
    Ok(fields)
}

/// The recipients of a draft as typed, by name where one is given (`Name <address>`), as one short clean line.
fn recipients(to: &[String]) -> String {
    let names: Vec<String> = to
        .iter()
        .take(notify::MAX_TEXT)
        .map(|typed| {
            let name = typed.split_once('<').map_or(typed.as_str(), |(name, _)| name).trim().trim_matches('"');
            line(if line(name).is_empty() { typed } else { name })
        })
        .filter(|name| !name.is_empty())
        .collect();
    line(&names.join(", "))
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
    let entry = context.store().schedule()?.into_iter().find(|entry| entry.id == id).ok_or(EngineError::NotFound { what: "schedule entry", id: params.id })?;
    if let Due::Send(draft) = entry.what
        && let Some(draft) = context.store().draft(draft)?
    {
        let mut outcome = super::send::taken_back(context, &draft)?;
        *outcome.value_mut() = json!({ "cancelled": params.id });
        return Ok(outcome);
    }
    Ok(Outcome::new(json!({ "cancelled": params.id })).with_change(Change::RemoveScheduled(id)))
}
