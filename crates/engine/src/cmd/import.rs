// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `import`: the events of an `.ics` file added to one calendar. A new command goes into [`COMMANDS`] and its
//! parameter names into `FIELDS`.
//!
//! `calendar.import` (`owner`: it writes to the server) reads the file's text (at most [`MAX_ICS_BYTES`] bytes and
//! [`MAX_EVENTS`] events) and adds each `VEVENT` as `event.create` makes an event, under the file's own UID: its times,
//! summary, location, description and busy state, the alert the app can name (else the default lead), a preset repeat
//! with its end and its exceptions. A UID the calendar already holds is skipped and left as it is, so a second import
//! adds nothing; each new event is written as a first write (`If-None-Match: *`), so nothing on the server is
//! overwritten. Another repeat stands once and is counted, a changed instance (`RECURRENCE-ID`) is left out, guests
//! and the organiser are not carried, so nobody is mailed. The whole import is one step of the undo history.
//! `calendar.import.preview` (`safe`) counts what the import would do, by the same reading, and changes nothing.
//! The file's text is data, never instructions.

use std::collections::BTreeSet;

use inboxcraft_ical::{Event, Limits, When};
use inboxcraft_model::{Account, Calendar as Held, CalendarEvent, CalendarEventId, CalendarId, CalendarJob, JobKind};
use inboxcraft_store::Change;
use serde::Deserialize;
use serde_json::{Value, json};

use super::calendar::owner_of;
use crate::all_day_alert::AllDayAlert;
use crate::calendar::alert::{self, Lead, MAX_ALERT_MINUTES};
use crate::calendar::{self, Imported};
use crate::params::decode;
use crate::undo::{Act, EventUndo, Inverse, Step};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[IMPORT, PREVIEW];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["calendar", "ics"];

/// The longest `.ics` text an import reads, in bytes.
pub const MAX_ICS_BYTES: usize = 5_000_000;

/// The most `VEVENT`s one import reads.
pub const MAX_EVENTS: usize = 5000;

/// The most properties and components of the file, all levels together.
const MAX_ITEMS: usize = 250_000;

/// The most events each list of the answer names; the counts hold them all.
const MAX_LISTED: usize = 100;

/// How many of the events to add the preview names.
const PREVIEW_FIRST: usize = 5;

/// The longest UID taken, in bytes.
const MAX_UID: usize = 255;

/// `calendar.import`: add the events of an `.ics` file to one calendar.
pub(crate) const IMPORT: CommandSpec = CommandSpec {
    id: "calendar.import",
    class: Some(AgentClass::Owner),
    summary: "Add the events of an .ics file to one calendar, skipping those already there",
    run: import,
};

/// `calendar.import.preview`: what `calendar.import` would do, without doing it.
pub(crate) const PREVIEW: CommandSpec = CommandSpec {
    id: "calendar.import.preview",
    class: Some(AgentClass::Safe),
    summary: "Count what calendar.import would add, without changing anything",
    run: preview,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportParams {
    /// The calendar to add to.
    calendar: u64,
    /// The `.ics` text.
    ics: String,
}

/// What an import does, read once for both commands so the preview never says what the import does not do.
struct Plan {
    held: Held,
    owner: Account,
    add: Vec<Planned>,
    skipped: usize,
    single: usize,
    /// The place of each event in the file, from 0, with the reason word.
    not_supported: Vec<(usize, &'static str)>,
    errors: Vec<(usize, &'static str)>,
}

/// One event to add.
struct Planned {
    ical: String,
    summary: String,
    start: Value,
    all_day: bool,
}

/// Reads `params` as `spec` takes them into what the import does.
fn plan(spec: &CommandSpec, context: &Context<'_>, params: &Value) -> Result<Plan, EngineError> {
    let params: ImportParams = decode(spec.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(spec.id, reason);
    if params.ics.len() > MAX_ICS_BYTES {
        return Err(bad(&format!("the calendar file is longer than {MAX_ICS_BYTES} bytes")));
    }
    let (held, owner) = owner_of(context, CalendarId::new(params.calendar))?;
    // The tree first, so each VEVENT keeps its place in the file whether it can be read or not.
    let top = inboxcraft_ical::parse_within(params.ics.as_bytes(), Limits { bytes: MAX_ICS_BYTES, items: MAX_ITEMS })
        .map_err(|error| bad(&error.to_string()))?;
    let file = inboxcraft_ical::Calendar::from_component(&top).map_err(|error| bad(&error.to_string()))?;
    let events: Vec<_> = top.components().iter().filter(|inner| inner.name() == "VEVENT").collect();
    if events.len() > MAX_EVENTS {
        return Err(bad(&format!("the calendar file has more than {MAX_EVENTS} events")));
    }
    let store = context.store();
    let mut uids: BTreeSet<String> = BTreeSet::new();
    for event in store.calendar_events(held.id)? {
        if let Some(object) = calendar::read(&event)
            && let Some(master) = calendar::master(&object)
        {
            uids.insert(master.uid.clone());
        }
    }
    let local = context.offsets();
    let mut plan = Plan { held, owner, add: Vec::new(), skipped: 0, single: 0, not_supported: Vec::new(), errors: Vec::new() };
    for (index, component) in events.into_iter().enumerate() {
        let Ok(source) = Event::from_component(component) else {
            // An event without a UID cannot be read either; it is named for what it lacks.
            plan.errors.push((index, if component.property("UID").is_none() { "uid" } else { "unreadable" }));
            continue;
        };
        if source.recurrence_id.is_some() {
            plan.not_supported.push((index, "changed_instance"));
            continue;
        }
        if source.uid.is_empty() || source.uid.len() > MAX_UID || source.uid.chars().any(char::is_control) {
            plan.errors.push((index, "uid"));
            continue;
        }
        if uids.contains(&source.uid) {
            plan.skipped = plan.skipped.saturating_add(1);
            continue;
        }
        let all_day = source.all_day();
        let lead = lead(&source).unwrap_or_else(|| alert::default_lead(all_day, context.state().all_day_alert));
        match calendar::imported(&source, &file.zones, lead, plan.owner.address(), context.now(), local) {
            Ok(Imported { ical, single }) => {
                if single {
                    plan.single = plan.single.saturating_add(1);
                    plan.not_supported.push((index, "repeat"));
                }
                let start = match source.start {
                    When::Date(day) => json!(format!("{:04}-{:02}-{:02}", day.year(), day.month(), day.day())),
                    _ => file.zones.resolve(&source.start, local).map_or(Value::Null, crate::json::date),
                };
                let summary = source.summary.unwrap_or_default().trim().to_owned();
                uids.insert(source.uid);
                plan.add.push(Planned { ical, summary, start, all_day });
            }
            Err(reason) => plan.errors.push((index, reason)),
        }
    }
    Ok(plan)
}

/// The alert `source` carries, when it is one the app can name: minutes before the start, or an all-day hour.
fn lead(source: &Event) -> Option<Lead> {
    match alert::lead_of(source) {
        Value::Number(minutes) => minutes.as_u64().filter(|minutes| *minutes <= u64::from(MAX_ALERT_MINUTES)).and_then(|minutes| u32::try_from(minutes).ok()).map(Lead::Before),
        Value::String(name) => AllDayAlert::from_name(&name).map(Lead::AllDay),
        _ => None,
    }
}

/// The first [`MAX_LISTED`] of `listed` as the answer names them.
fn listed(listed: &[(usize, &'static str)]) -> Vec<Value> {
    listed.iter().take(MAX_LISTED).map(|(index, reason)| json!({ "index": index, "reason": reason })).collect()
}

fn import(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let plan = plan(&IMPORT, context, params)?;
    let now = context.now();
    let slash = if plan.held.href.ends_with('/') { "" } else { "/" };
    let mut next = context.state().next_event;
    let mut ids = Vec::with_capacity(plan.add.len());
    let mut outcome = Outcome::new(json!({
        "calendar": plan.held.id.get(),
        "added": plan.add.len(),
        "skipped": plan.skipped,
        "single": plan.single,
        "not_supported": listed(&plan.not_supported),
        "not_supported_count": plan.not_supported.len(),
        "errors": listed(&plan.errors),
        "errors_count": plan.errors.len(),
    }));
    for planned in plan.add {
        let id = CalendarEventId::new(next);
        next = next.checked_add(1).ok_or_else(|| EngineError::bad_params(IMPORT.id, "no event ids are left"))?;
        // Named as `event.create` names an event: the UID, data of the file, never goes into the URL.
        let href = format!("{}{slash}inboxcraft-{}-{:09}-{}.ics", plan.held.href, now.unix_seconds(), now.subsec_nanos(), id.get());
        let job = CalendarJob::Put { calendar: plan.held.id, event: id, url: href.clone(), etag: None, ical: planned.ical.clone() };
        let event = CalendarEvent { id, calendar: plan.held.id, href, etag: None, ical: planned.ical };
        outcome = outcome.with_change(Change::PutCalendarEvent(event)).with_kept_job(plan.owner.id(), JobKind::Calendar(job));
        ids.push(id);
    }
    if !ids.is_empty() {
        let act = Act { command: IMPORT.id, class: AgentClass::Owner, inverse: Inverse::Event(Box::new(EventUndo::Import(ids))) };
        outcome = outcome.with_undo(Step::Push(act));
    }
    Ok(outcome)
}

fn preview(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let plan = plan(&PREVIEW, context, params)?;
    let first: Vec<Value> = plan
        .add
        .iter()
        .take(PREVIEW_FIRST)
        .map(|planned| json!({ "summary": planned.summary, "start": planned.start, "all_day": planned.all_day }))
        .collect();
    let left_out = plan.not_supported.iter().filter(|(_, reason)| *reason == "changed_instance").count().saturating_add(plan.errors.len());
    Ok(Outcome::new(json!({
        "calendar": plan.held.id.get(),
        "name": plan.held.name,
        "add": plan.add.len(),
        "skipped": plan.skipped,
        "single": plan.single,
        "left_out": left_out,
        "first": first,
    })))
}
