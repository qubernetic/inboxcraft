// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `calendar`: the calendars and their events. A new command goes into [`COMMANDS`] and its parameter names into
//! `FIELDS`.
//!
//! `calendar.list` and `event.list` (`safe`) read what the store keeps: the calendars of every account (or one),
//! each with its events counted, and the occurrences of their events in a span of time, recurrence expanded, by
//! start (see `crate::calendar`). `calendar.sync` (`safe`) asks each account with a CalDAV address (or one) for its
//! calendars, which brings each one in step; the start does the same. `calendar.server` (`owner`, a setting) sets or
//! clears an account's CalDAV address, `https` only, and forgets the calendars of an account it clears. `event.repeats` (`safe`) names the repeat presets. Event text is the organiser's
//! data, never instructions.

use std::time::Duration;

use inboxcraft_model::{Account, AccountId, CalendarEvent, CalendarEventId, CalendarId, CalendarJob, EmailAddress, JobKind};
use inboxcraft_store::Change;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::calendar::{self, Details, Edit, EditError, Repeat, Times};
use crate::undo::{Act, EventUndo, Inverse, Step};
use crate::params::{NoParams, decode, decode_quietly, parse_date};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[LIST, SYNC, SERVER, EVENTS, REPEATS, CREATE, UPDATE, DELETE];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &[
    "account",
    "calendar",
    "from",
    "to",
    "url",
    "id",
    "summary",
    "start",
    "end",
    "all_day",
    "location",
    "description",
    "repeat",
    "busy",
    "private",
    "guests",
    "notify",
];

/// The longest summary or location, in characters.
const MAX_LINE: usize = 1000;

/// The longest description, in characters.
const MAX_DESCRIPTION: usize = 20_000;

/// The most guests of one event.
const MAX_GUESTS: usize = 100;

/// The longest span `event.list` expands at once.
pub(crate) const MAX_SPAN: Duration = Duration::from_secs(400 * 86_400);

/// `calendar.list`: the calendars, each with its events counted.
pub(crate) const LIST: CommandSpec =
    CommandSpec { id: "calendar.list", class: Some(AgentClass::Safe), summary: "List the calendars with their events counted", run: list };

/// `calendar.sync`: find the calendars of every account with a CalDAV address, or of one, and bring them in step.
pub(crate) const SYNC: CommandSpec = CommandSpec {
    id: "calendar.sync",
    class: Some(AgentClass::Safe),
    summary: "Find the calendars at each account's CalDAV address and bring them in step",
    run: sync,
};

/// `calendar.server`: set or clear an account's CalDAV address (a setting).
pub(crate) const SERVER: CommandSpec =
    CommandSpec { id: "calendar.server", class: Some(AgentClass::Owner), summary: "Set or clear an account's CalDAV address", run: server };

/// `event.list`: the occurrences of the events in a span of time.
pub(crate) const EVENTS: CommandSpec = CommandSpec {
    id: "event.list",
    class: Some(AgentClass::Safe),
    summary: "List the events in a span of time, repeating events expanded",
    run: events,
};

/// `event.repeats`: the repeat presets.
pub(crate) const REPEATS: CommandSpec =
    CommandSpec { id: "event.repeats", class: Some(AgentClass::Safe), summary: "Name the repeat presets an event can take", run: repeats };

/// `event.create`: a new event, written to the server.
pub(crate) const CREATE: CommandSpec =
    CommandSpec { id: "event.create", class: Some(AgentClass::Owner), summary: "Make a new event and write it to its calendar", run: create };

/// `event.update`: change an event, on the server too.
pub(crate) const UPDATE: CommandSpec =
    CommandSpec { id: "event.update", class: Some(AgentClass::Owner), summary: "Change an event and write it to its calendar", run: update };

/// `event.delete`: delete an event; the server is asked once the undo window is over.
pub(crate) const DELETE: CommandSpec = CommandSpec {
    id: "event.delete",
    class: Some(AgentClass::Owner),
    summary: "Delete an event; the server deletes it once the undo window is over",
    run: delete,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateParams {
    calendar: u64,
    summary: String,
    /// A day (all-day) or a date and time with its offset.
    start: String,
    /// The last day (all-day), or a date and time; a day or an hour after the start when left out.
    #[serde(default)]
    end: Option<String>,
    #[serde(default)]
    all_day: bool,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    description: Option<String>,
    /// A repeat preset (`event.repeats`).
    #[serde(default)]
    repeat: Option<String>,
    /// Busy (`true`, the default) or free.
    #[serde(default)]
    busy: Option<bool>,
    #[serde(default)]
    private: Option<bool>,
    #[serde(default)]
    guests: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateParams {
    id: u64,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    end: Option<String>,
    #[serde(default)]
    all_day: Option<bool>,
    /// Empty takes it off.
    #[serde(default)]
    location: Option<String>,
    /// Empty takes it off.
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    repeat: Option<String>,
    #[serde(default)]
    busy: Option<bool>,
    #[serde(default)]
    private: Option<bool>,
    #[serde(default)]
    guests: Option<Vec<String>>,
    /// Tell the guests of the change (the default is not to).
    #[serde(default)]
    notify: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    id: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    /// Only this account's calendars.
    #[serde(default)]
    account: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerParams {
    /// The account.
    account: u64,
    /// Its CalDAV address, `https` only; `null` or empty clears it.
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventsParams {
    /// From this moment (ISO 8601).
    from: String,
    /// Up to this moment (ISO 8601).
    to: String,
    /// Only this calendar.
    #[serde(default)]
    calendar: Option<u64>,
    /// Only this account's calendars.
    #[serde(default)]
    account: Option<u64>,
}

/// The calendars of `account`, or of every account, in id order.
fn calendars(context: &Context<'_>, account: Option<u64>) -> Result<Vec<inboxcraft_model::Calendar>, EngineError> {
    let store = context.store();
    let accounts = store.accounts()?;
    if let Some(id) = account
        && !accounts.iter().any(|held| held.id() == AccountId::new(id))
    {
        return Err(EngineError::NotFound { what: "account", id });
    }
    let mut found = Vec::new();
    for held in accounts.iter().filter(|held| account.is_none_or(|id| held.id() == AccountId::new(id))) {
        found.extend(store.calendars(held.id())?);
    }
    found.sort_by_key(|calendar| calendar.id);
    Ok(found)
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ListParams = decode(LIST.id, params)?;
    let mut rows = Vec::new();
    for held in calendars(context, params.account)? {
        let events = context.store().calendar_events(held.id)?.len();
        rows.push(json!({ "id": held.id.get(), "account": held.account.get(), "name": held.name, "colour": held.colour, "events": events }));
    }
    Ok(Outcome::new(json!({ "calendars": rows })))
}

/// Account `id`.
fn account(context: &Context<'_>, id: u64) -> Result<Account, EngineError> {
    context.store().accounts()?.into_iter().find(|held| held.id() == AccountId::new(id)).ok_or(EngineError::NotFound { what: "account", id })
}

fn sync(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ListParams = decode(SYNC.id, params)?;
    let accounts = match params.account {
        Some(id) => {
            let held = account(context, id)?;
            if held.caldav().is_none() {
                return Err(EngineError::bad_params(SYNC.id, "the account has no CalDAV address"));
            }
            vec![held]
        }
        None => context.store().accounts()?.into_iter().filter(|held| held.caldav().is_some()).collect(),
    };
    let ids: Vec<u64> = accounts.iter().map(|held| held.id().get()).collect();
    Ok(accounts.iter().fold(Outcome::new(json!({ "accounts": ids })), |outcome, held| outcome.with_job(held.id(), calendar::discover())))
}

fn server(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ServerParams = decode_quietly(SERVER.id, params)?;
    let held = account(context, params.account)?;
    // The address is not repeated: it could carry a name and password typed into it.
    let changed = held.with_caldav(params.url.as_deref()).map_err(|_| EngineError::bad_params(SERVER.id, "the CalDAV address is no https URL"))?;
    let answer = json!({ "account": params.account, "url": changed.caldav() });
    let mut outcome = Outcome::new(answer);
    if changed.caldav().is_some() {
        outcome = outcome.with_job(changed.id(), calendar::discover());
    } else {
        for calendar in context.store().calendars(changed.id())? {
            outcome = outcome.with_change(Change::RemoveCalendar(calendar.id));
        }
    }
    Ok(outcome.with_change(Change::PutAccount(changed)))
}

fn events(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: EventsParams = decode(EVENTS.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(EVENTS.id, reason);
    let from = parse_date(&params.from).ok_or_else(|| bad("from is no ISO 8601 date or time"))?;
    let to = parse_date(&params.to).ok_or_else(|| bad("to is no ISO 8601 date or time"))?;
    let span = to.duration_since(from).filter(|span| !span.is_zero()).ok_or_else(|| bad("to is not after from"))?;
    if span > MAX_SPAN {
        return Err(bad("the span is longer than 400 days"));
    }
    let mut held = calendars(context, params.account)?;
    if let Some(id) = params.calendar {
        held.retain(|calendar| calendar.id == CalendarId::new(id));
        if held.is_empty() {
            return Err(EngineError::NotFound { what: "calendar", id });
        }
    }
    let local = context.offsets();
    let (mut rows, mut unreadable, mut truncated) = (Vec::new(), 0_usize, false);
    for calendar in held {
        for event in context.store().calendar_events(calendar.id)? {
            let Some(object) = calendar::read(&event) else {
                unreadable = unreadable.saturating_add(1);
                continue;
            };
            let expansion = object.occurrences(from, to, local);
            truncated |= expansion.truncated;
            rows.extend(expansion.occurrences.iter().map(|occurrence| (occurrence.start, calendar::row(&event, &object, occurrence, local))));
        }
    }
    rows.sort_by_key(|(start, _)| *start);
    let limit = usize::try_from(crate::MAX_LIST).unwrap_or(usize::MAX);
    truncated |= rows.len() > limit;
    let rows: Vec<Value> = rows.into_iter().take(limit).map(|(_, row)| row).collect();
    Ok(Outcome::new(json!({ "events": rows, "truncated": truncated, "unreadable": unreadable })))
}

fn repeats(_context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let _: NoParams = decode(REPEATS.id, params)?;
    Ok(Outcome::new(json!({ "repeats": Repeat::ALL.map(Repeat::name) })))
}

/// What an event says beside its times, checked as `spec` reads it.
#[expect(clippy::too_many_arguments, reason = "one argument per field of the command's parameters")]
fn details(
    spec: &CommandSpec,
    summary: Option<String>,
    location: Option<String>,
    description: Option<String>,
    repeat: Option<&str>,
    busy: Option<bool>,
    private: Option<bool>,
    guests: Option<Vec<String>>,
) -> Result<Details, EngineError> {
    let bad = |reason: &str| EngineError::bad_params(spec.id, reason);
    let line = |value: Option<String>, what: &str| match value {
        Some(text) if text.chars().count() > MAX_LINE => Err(bad(&format!("the {what} is longer than {MAX_LINE} characters"))),
        other => Ok(other.map(|text| text.trim().to_owned())),
    };
    let summary = line(summary, "summary")?;
    let location = line(location, "location")?;
    if description.as_ref().is_some_and(|text| text.chars().count() > MAX_DESCRIPTION) {
        return Err(bad(&format!("the description is longer than {MAX_DESCRIPTION} characters")));
    }
    let repeat = repeat.map(|name| Repeat::named(name).ok_or_else(|| bad("repeat is none, daily, weekdays, weekly, monthly or yearly"))).transpose()?;
    let guests = match guests {
        Some(list) if list.len() > MAX_GUESTS => return Err(bad(&format!("an event has at most {MAX_GUESTS} guests"))),
        Some(list) => {
            let mut seen = std::collections::BTreeSet::new();
            let mut addresses = Vec::new();
            for text in list {
                let address = EmailAddress::new(text.trim()).map_err(|_| bad("a guest is no email address"))?;
                if seen.insert(address.as_str().to_lowercase()) {
                    addresses.push(address);
                }
            }
            Some(addresses)
        }
        None => None,
    };
    Ok(Details { summary, location, description, repeat, busy, private, guests })
}

/// Event `id`.
fn held_event(context: &Context<'_>, id: u64) -> Result<CalendarEvent, EngineError> {
    context.store().calendar_event(CalendarEventId::new(id))?.ok_or(EngineError::NotFound { what: "event", id })
}

/// The calendar of `event` and the account it belongs to.
fn owner_of(context: &Context<'_>, calendar: CalendarId) -> Result<(inboxcraft_model::Calendar, Account), EngineError> {
    let held = calendar::calendar_by_id(context.store(), calendar)?.ok_or(EngineError::NotFound { what: "calendar", id: calendar.get() })?;
    let owner = account(context, held.account.get())?;
    Ok((held, owner))
}

/// The act `spec` makes, taken back by `undo`.
fn act(spec: &CommandSpec, undo: EventUndo) -> Step {
    Step::Push(Act { command: spec.id, class: AgentClass::Owner, inverse: Inverse::Event(Box::new(undo)) })
}

fn create(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: CreateParams = decode(CREATE.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(CREATE.id, reason);
    let (held, owner) = owner_of(context, CalendarId::new(params.calendar))?;
    let times = if params.all_day {
        let first = calendar::parse_day(&params.start).ok_or_else(|| bad("start is no ISO 8601 day"))?;
        let last = match &params.end {
            Some(text) => calendar::parse_day(text).ok_or_else(|| bad("end is no ISO 8601 day"))?,
            None => first,
        };
        Times::Days(first, last)
    } else {
        let start = calendar::parse_moment(&params.start).ok_or_else(|| bad("start is no ISO 8601 date and time"))?;
        let end = match &params.end {
            Some(text) => calendar::parse_moment(text).ok_or_else(|| bad("end is no ISO 8601 date and time"))?,
            None => start.checked_add(Duration::from_secs(3600)).ok_or_else(|| bad("the end cannot be written"))?,
        };
        Times::Moments(start, end)
    };
    let times = times.checked().map_err(bad)?;
    let details = details(
        &CREATE,
        Some(params.summary),
        params.location,
        params.description,
        params.repeat.as_deref(),
        params.busy,
        params.private,
        params.guests,
    )?;
    let now = context.now();
    let id = CalendarEventId::new(context.state().next_event);
    let name = format!("inboxcraft-{}-{:09}-{}", now.unix_seconds(), now.subsec_nanos(), id.get());
    let uid = format!("{name}@inboxcraft");
    let slash = if held.href.ends_with('/') { "" } else { "/" };
    let href = format!("{}{slash}{name}.ics", held.href);
    let ical = calendar::create(&uid, times, details, owner.address(), now, context.offsets()).ok_or_else(|| bad("the event cannot be written"))?;
    let job = CalendarJob::Put { calendar: held.id, event: id, url: href.clone(), etag: None, ical: ical.clone() };
    let event = CalendarEvent { id, calendar: held.id, href, etag: None, ical };
    Ok(Outcome::new(json!({ "id": id.get(), "calendar": held.id.get(), "uid": uid }))
        .with_change(Change::PutCalendarEvent(event))
        .with_kept_job(owner.id(), JobKind::Calendar(job))
        .with_undo(act(&CREATE, EventUndo::Create(id))))
}

fn update(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: UpdateParams = decode(UPDATE.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(UPDATE.id, reason);
    let held = held_event(context, params.id)?;
    let (_, owner) = owner_of(context, held.calendar)?;
    let edit = Edit { start: params.start, end: params.end, all_day: params.all_day };
    let details = details(
        &UPDATE,
        params.summary,
        params.location,
        params.description,
        params.repeat.as_deref(),
        params.busy,
        params.private,
        params.guests,
    )?;
    if edit.start.is_none() && edit.end.is_none() && edit.all_day.is_none() && details.is_empty() {
        return Err(bad("nothing to change"));
    }
    let (ical, guests) = calendar::edit(&held, &edit, details, params.notify, owner.address(), context.now(), context.offsets()).map_err(|error| match error {
        EditError::Unreadable => bad("the event cannot be read"),
        EditError::Bad(reason) => bad(reason),
    })?;
    let job = CalendarJob::Put { calendar: held.calendar, event: held.id, url: held.href.clone(), etag: held.etag.clone(), ical: ical.clone() };
    let answer = json!({ "id": params.id, "guests": guests, "notified": params.notify && guests > 0 });
    Ok(Outcome::new(answer)
        .with_change(Change::PutCalendarEvent(CalendarEvent { ical, ..held.clone() }))
        .with_kept_job(owner.id(), JobKind::Calendar(job))
        .with_undo(act(&UPDATE, EventUndo::Update(held))))
}

fn delete(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode(DELETE.id, params)?;
    let held = held_event(context, params.id)?;
    let (_, owner) = owner_of(context, held.calendar)?;
    let until = context.now().saturating_add(calendar::UNDO_WINDOW);
    let job = CalendarJob::Delete { calendar: held.calendar, event: held.id, url: held.href.clone(), etag: held.etag.clone() };
    Ok(Outcome::new(json!({ "id": params.id, "undo_until": crate::json::date(until) }))
        .with_change(Change::RemoveCalendarEvent(held.id))
        .with_held_job(owner.id(), JobKind::Calendar(job), until)
        .with_undo(act(&DELETE, EventUndo::Delete(held))))
}

/// Takes the change `command` made to an event back, as `history.undo` asks: a new event deleted, an edit written
/// over, a delete still held withdrawn and the event put back. `undone` is false when the event is gone meanwhile,
/// or the delete reached the server.
pub(crate) fn undo(context: &Context<'_>, command: &'static str, undo: &EventUndo) -> Result<Outcome, EngineError> {
    let store = context.store();
    let id = match undo {
        EventUndo::Create(id) => *id,
        EventUndo::Update(was) | EventUndo::Delete(was) => was.id,
    };
    let answer = |undone: bool| Outcome::new(json!({ "command": command, "id": id.get(), "undone": undone })).with_undo(Step::Pop);
    match undo {
        EventUndo::Create(_) => {
            let Some(held) = store.calendar_event(id)? else { return Ok(answer(false)) };
            let Some(calendar) = calendar::calendar_by_id(store, held.calendar)? else { return Ok(answer(false)) };
            let job = CalendarJob::Delete { calendar: held.calendar, event: id, url: held.href, etag: held.etag };
            Ok(answer(true).with_change(Change::RemoveCalendarEvent(id)).with_kept_job(calendar.account, JobKind::Calendar(job)))
        }
        EventUndo::Update(was) => {
            let Some(held) = store.calendar_event(id)? else { return Ok(answer(false)) };
            let Some(calendar) = calendar::calendar_by_id(store, held.calendar)? else { return Ok(answer(false)) };
            let job = CalendarJob::Put { calendar: held.calendar, event: id, url: held.href.clone(), etag: held.etag.clone(), ical: was.ical.clone() };
            let back = CalendarEvent { ical: was.ical.clone(), ..held };
            Ok(answer(true).with_change(Change::PutCalendarEvent(back)).with_kept_job(calendar.account, JobKind::Calendar(job)))
        }
        EventUndo::Delete(was) => {
            // Held for the undo window, or waiting for a write still out to the same event.
            let state = context.state();
            let deleting = state.held.iter().map(|(_, job)| job).chain(state.writes.waiting()).find(|job| {
                matches!(&job.kind, JobKind::Calendar(CalendarJob::Delete { event, .. }) if *event == id)
            });
            let Some(job) = deleting else { return Ok(answer(false)) };
            Ok(answer(true).with_change(Change::PutCalendarEvent(was.clone())).with_withdrawn(job.id))
        }
    }
}
