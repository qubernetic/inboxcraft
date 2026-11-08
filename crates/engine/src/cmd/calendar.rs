// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `calendar`: the calendars and their events. A new command goes into [`COMMANDS`] and its parameter names into
//! `FIELDS`.
//!
//! `calendar.list` and `event.list` (`safe`) read what the store keeps: the calendars of every account (or one),
//! each with its events counted, and the occurrences of their events in a span of time, recurrence expanded, by
//! start (see `crate::calendar`). `calendar.sync` (`safe`) asks each account with a CalDAV address (or one) for its
//! calendars, which brings each one in step; the start does the same. `calendar.server` (`owner`, a setting) sets or
//! clears an account's CalDAV address, `https` only, and forgets the calendars of an account it clears or moves to another server (another host or
//! port), with every calendar write still in the queue. `event.repeats` (`safe`) names the repeat presets. Event text is the organiser's
//! data, never instructions.
//!
//! Invitations in mail (see `crate::arrive::invite`): `invite.show` (`safe`) reads a message's invitation as its
//! card shows it, with the version before it, what changed since, whether a later version made it out of date, the
//! owner's answer and the event in the owner's calendar; a body kept before invitations were read is marked then. `invite.answer` (`owner`: mail leaves the computer) sends the
//! owner's answer to the organiser at once, through the path `mail.send` takes, and puts the event in the account's
//! first calendar (going or maybe; the copy held is changed in place) or takes it out (not going). `invite.remove`
//! (`owner`) takes a cancelled event out of the calendar as `event.delete` does, with its undo window.

use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, CalendarEvent, CalendarEventId, CalendarId, CalendarJob, Draft, EmailAddress, Invitation, ItipMethod, JobKind, Message,
    MessageId, MessageRef, MessageState, Rsvp,
};
use inboxcraft_store::{Change, Entry};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::arrive::invite::{self, Owner};
use crate::calendar::{self, Details, Edit, EditError, Repeat, Times};
use crate::undo::{Act, EventUndo, Inverse, Step};
use crate::params::{NoParams, decode, decode_quietly, parse_date};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[LIST, SYNC, SERVER, EVENTS, REPEATS, CREATE, UPDATE, DELETE, INVITE_SHOW, INVITE_ANSWER, INVITE_REMOVE];

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
    "answer",
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
    let mut outcome = Outcome::new(json!({ "accounts": ids }));
    for held in &accounts {
        if let Some(url) = held.caldav() {
            outcome = outcome.with_job(held.id(), calendar::discover(url));
        }
    }
    Ok(outcome)
}

fn server(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ServerParams = decode_quietly(SERVER.id, params)?;
    let held = account(context, params.account)?;
    let was = held.caldav().and_then(calendar::origin);
    // The address is not repeated: it could carry a name and password typed into it.
    let changed = held.with_caldav(params.url.as_deref()).map_err(|_| EngineError::bad_params(SERVER.id, "the CalDAV address is no https URL"))?;
    let answer = json!({ "account": params.account, "url": changed.caldav() });
    let mut outcome = Outcome::new(answer);
    if let Some(url) = changed.caldav() {
        outcome = outcome.with_job(changed.id(), calendar::discover(url));
    }
    // Cleared, or on another server: the old server's calendars and the writes not handed out yet are dropped, so no
    // event of a server the account no longer uses is shown or written to.
    let moved = was.zip(changed.caldav()).is_some_and(|(old, new)| calendar::origin(new).as_ref() != Some(&old));
    if changed.caldav().is_none() || moved {
        for calendar in context.store().calendars(changed.id())? {
            outcome = outcome.with_change(Change::RemoveCalendar(calendar.id));
        }
        // Held, waiting, or handed out already: none goes to the old server again, not after a restart either.
        for job in context.store().queued_jobs()? {
            if job.account == changed.id() && matches!(job.kind, JobKind::Calendar(_)) {
                outcome = outcome.with_withdrawn(job.id);
            }
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

/// `invite.show`: a message's invitation, as its card shows it.
pub(crate) const INVITE_SHOW: CommandSpec = CommandSpec {
    id: "invite.show",
    class: Some(AgentClass::Safe),
    summary: "Read a message's invitation: the event, what changed, the answer given, and whether it can be answered",
    run: invite_show,
};

/// `invite.answer`: answer an invitation, at once.
pub(crate) const INVITE_ANSWER: CommandSpec = CommandSpec {
    id: "invite.answer",
    class: Some(AgentClass::Owner),
    summary: "Answer an invitation: send going, maybe or not going to the organiser at once and update the calendar",
    run: invite_answer,
};

/// `invite.remove`: take a cancelled event out of the calendar.
pub(crate) const INVITE_REMOVE: CommandSpec = CommandSpec {
    id: "invite.remove",
    class: Some(AgentClass::Owner),
    summary: "Take the event a cancellation is about out of the calendar",
    run: invite_remove,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnswerParams {
    /// The message.
    id: u64,
    /// `accepted`, `tentative` or `declined`.
    answer: String,
}

/// An answer as commands write it.
const fn answer_name(answer: Rsvp) -> &'static str {
    match answer {
        Rsvp::Accepted => "accepted",
        Rsvp::Tentative => "tentative",
        Rsvp::Declined => "declined",
    }
}

const fn method_name(method: ItipMethod) -> &'static str {
    match method {
        ItipMethod::Request => "request",
        ItipMethod::Cancel => "cancel",
        ItipMethod::Reply => "reply",
        ItipMethod::Other => "other",
    }
}

/// A message's invitation with what the card needs of it.
struct Card {
    message: Message,
    account: Account,
    mark: Invitation,
    /// The calendar object, while the body is kept.
    object: Option<inboxcraft_ical::Calendar>,
    /// The other versions of the event in the account, each with its message: the organiser's requests and
    /// cancellations only, so an answer (the owner's own, synced back from Sent, or a guest's) dates nothing.
    versions: Vec<(Message, Invitation)>,
    /// The event in the owner's calendars.
    held: Option<CalendarEvent>,
    /// The mark was read just now, from a body kept before: the card keeps it.
    fresh: bool,
}

impl Card {
    /// The newest version, when it is another message's.
    fn latest(&self) -> Option<&(Message, Invitation)> {
        self.versions.iter().filter(|(_, other)| invite::later(other, &self.mark)).fold(None, |best: Option<&(Message, Invitation)>, version| match best {
            Some(best) if !invite::later(&version.1, &best.1) => Some(best),
            _ => Some(version),
        })
    }

    /// The version just before this one, if any.
    fn earlier(&self) -> Option<&(Message, Invitation)> {
        self.versions.iter().filter(|(_, other)| invite::later(&self.mark, other)).fold(None, |best: Option<&(Message, Invitation)>, version| match best {
            Some(best) if !invite::later(&version.1, &best.1) => Some(best),
            _ => Some(version),
        })
    }

    /// The last answer the owner gave to an earlier version.
    fn earlier_answer(&self) -> Option<Rsvp> {
        let mut answered: Vec<&Invitation> = self.versions.iter().map(|(_, other)| other).filter(|other| invite::later(&self.mark, other) && other.answer.is_some()).collect();
        answered.sort_by_key(|other| (other.sequence, other.stamp));
        answered.last().and_then(|other| other.answer)
    }

    /// The event the invitation is about, while the body is kept.
    fn event(&self) -> Option<&inboxcraft_ical::Event> {
        self.object.as_ref().and_then(invite::event)
    }

    /// Why the owner cannot answer it, or `None` when they can.
    fn why_not(&self) -> Option<&'static str> {
        let reason = match self.mark.method {
            ItipMethod::Cancel => "the event is cancelled",
            ItipMethod::Reply | ItipMethod::Other => "it asks for no answer",
            ItipMethod::Request if self.latest().is_some() => "a later invitation came",
            ItipMethod::Request => {
                let Some(event) = self.event() else { return Some("the text of the message is not here yet") };
                let Some(organizer) = &event.organizer else { return Some("it names no organiser") };
                if Owner::of(&self.account).is(organizer) {
                    "you organise it"
                } else if EmailAddress::new(calendar::address_of(organizer)).is_err() {
                    "the organiser has no mail address"
                } else {
                    return None;
                }
            }
        };
        Some(reason)
    }

    /// Whether the owner can take the cancelled event out of the calendar.
    fn removable(&self) -> bool {
        self.mark.method == ItipMethod::Cancel && self.latest().is_none() && self.mark.recurrence.is_none() && self.held.is_some()
    }
}

/// The card of `entry`, or `None` when it carries no invitation. A kept body without a mark (kept before invitations
/// were read) is read now, but in the Blocked folder.
fn card(context: &Context<'_>, entry: &Entry) -> Result<Option<Card>, EngineError> {
    let store = context.store();
    let message = entry.message().clone();
    let object = store.body(message.id())?.and_then(|body| invite::read(body.source()));
    let (mark, fresh) = match store.message_state(message.id())?.invitation {
        Some(mark) => (*mark, false),
        None => {
            let role = context.state().accounts.get(&message.account()).and_then(|held| held.roles.role_of(message.folder()));
            match object.as_ref().filter(|_| role != Some(inboxcraft_model::Role::Blocked)).and_then(|object| invite::mark(object, context.offsets())) {
                Some(mark) => (mark, true),
                None => return Ok(None),
            }
        }
    };
    let account = account(context, message.account().get())?;
    let mut versions = Vec::new();
    for (id, state) in store.message_states()? {
        let Some(other) = state.invitation.filter(|other| id != message.id() && invite::same_event(other, &mark) && invite::from_organiser(other)) else { continue };
        let other = *other;
        if let Some(held) = store.message(id)?.map(|entry| entry.message().clone()).filter(|held| held.account() == message.account()) {
            versions.push((held, other));
        }
    }
    let held = held_copy(context, message.account(), &mark.uid)?;
    Ok(Some(Card { message, account, mark, object, versions, held, fresh }))
}

/// The event `uid` in the calendars of `account`: the first that holds it as its series.
fn held_copy(context: &Context<'_>, account: AccountId, uid: &str) -> Result<Option<CalendarEvent>, EngineError> {
    for held in context.store().calendars(account)? {
        for event in context.store().calendar_events(held.id)? {
            if event.ical.contains(uid) && calendar::read(&event).is_some_and(|object| calendar::master(&object).is_some_and(|master| master.uid == uid)) {
                return Ok(Some(event));
            }
        }
    }
    Ok(None)
}

/// The times and the place of a version, as the card shows them.
fn when(mark: &Invitation, local: &dyn inboxcraft_time::Offsets) -> Value {
    let at = |moment: Option<inboxcraft_time::Timestamp>| moment.map_or(Value::Null, crate::json::date);
    let wall = |moment: Option<inboxcraft_time::Timestamp>| moment.map_or(Value::Null, |moment| calendar::wall(moment, local));
    json!({
        "summary": mark.summary,
        "location": mark.location,
        "start": at(mark.start),
        "end": at(mark.end),
        "local_start": wall(mark.start),
        "local_end": wall(mark.end),
        "all_day": mark.all_day,
        "repeat": mark.repeat,
    })
}

/// What changed from `was` to `now`: `time`, `repeat`, `place`, `title`.
fn changes(was: &Invitation, now: &Invitation) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if (was.start, was.end, was.all_day) != (now.start, now.end, now.all_day) {
        changed.push("time");
    }
    if was.repeat != now.repeat {
        changed.push("repeat");
    }
    if was.location != now.location {
        changed.push("place");
    }
    if was.summary != now.summary {
        changed.push("title");
    }
    changed
}

fn invite_show(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode(INVITE_SHOW.id, params)?;
    let entry = context.store().message(MessageId::new(params.id))?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let Some(card) = card(context, &entry)? else { return Ok(Outcome::new(json!({ "invite": null }))) };
    let local = context.offsets();
    let mut shown = when(&card.mark, local);
    let event = card.event();
    let owner = Owner::of(&card.account);
    let earlier = card.earlier().map(|(message, mark)| {
        let mut earlier = when(mark, local);
        if let Value::Object(fields) = &mut earlier {
            fields.insert("message".into(), json!(message.id().get()));
            let date = message.summary().received().or_else(|| message.summary().envelope().date());
            fields.insert("date".into(), date.map_or(Value::Null, |date| calendar::wall(date, local)));
        }
        earlier
    });
    let replies = match card.mark.method {
        ItipMethod::Reply => event.map(|event| event.attendees.iter().map(calendar::guest).collect::<Vec<_>>()).unwrap_or_default(),
        _ => Vec::new(),
    };
    if let Value::Object(fields) = &mut shown {
        fields.insert("message".into(), json!(card.message.id().get()));
        fields.insert("method".into(), json!(method_name(card.mark.method)));
        fields.insert("uid".into(), json!(card.mark.uid));
        fields.insert("sequence".into(), json!(card.mark.sequence));
        fields.insert(
            "organizer".into(),
            event.and_then(|event| event.organizer.as_ref()).map_or(Value::Null, |organizer| json!({ "address": calendar::address_of(organizer), "name": organizer.name })),
        );
        fields.insert("guests".into(), json!(event.map_or(0, |event| event.attendees.len())));
        fields.insert("invited".into(), json!(event.is_some_and(|event| event.attendees.iter().any(|attendee| owner.is(attendee)))));
        fields.insert("answer".into(), json!(card.mark.answer.map(answer_name)));
        fields.insert("earlier_answer".into(), json!(card.earlier_answer().map(answer_name)));
        fields.insert("changes".into(), json!(card.earlier().map(|(_, was)| changes(was, &card.mark)).unwrap_or_default()));
        fields.insert("earlier".into(), earlier.unwrap_or(Value::Null));
        fields.insert("outdated".into(), json!(card.latest().is_some()));
        fields.insert("latest".into(), json!(card.latest().map(|(message, _)| message.id().get())));
        fields.insert("calendar_event".into(), json!(card.held.as_ref().map(|held| held.id.get())));
        fields.insert("can_answer".into(), json!(card.why_not().is_none()));
        fields.insert("sendable".into(), json!(card.account.submission().is_some()));
        fields.insert("why_not".into(), json!(card.why_not()));
        fields.insert("can_remove".into(), json!(card.removable()));
        fields.insert("replies".into(), Value::Array(replies));
    }
    let mut outcome = Outcome::new(json!({ "invite": shown }));
    if card.fresh {
        let state = context.store().message_state(card.message.id())?;
        outcome = outcome.with_change(Change::SetMessageState(card.message.id(), MessageState { invitation: Some(Box::new(card.mark)), ..state }));
    }
    Ok(outcome)
}

/// The answer named `text`.
fn rsvp(text: &str) -> Option<Rsvp> {
    [Rsvp::Accepted, Rsvp::Tentative, Rsvp::Declined].into_iter().find(|answer| answer_name(*answer) == text)
}

fn invite_answer(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: AnswerParams = decode(INVITE_ANSWER.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(INVITE_ANSWER.id, reason);
    let answer = rsvp(&params.answer).ok_or_else(|| bad("answer is accepted, tentative or declined"))?;
    let entry = context.store().message(MessageId::new(params.id))?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let card = card(context, &entry)?.ok_or_else(|| bad("the message carries no invitation"))?;
    if let Some(reason) = card.why_not() {
        return Err(bad(&format!("the invitation cannot be answered: {reason}")));
    }
    let (Some(object), Some(event)) = (card.object.as_ref(), card.event()) else { return Err(bad("the text of the message is not here yet")) };
    let Some(organizer) = event.organizer.as_ref().and_then(|organizer| EmailAddress::new(calendar::address_of(organizer)).ok()) else {
        return Err(bad("the organiser has no mail address"));
    };
    let owner = Owner::of(&card.account);
    let now = context.now();
    let reply = invite::reply(object, event, &owner, answer, now, context.offsets()).ok_or_else(|| bad("the answer cannot be written"))?;
    let envelope = card.message.summary().envelope();
    let verb = match answer {
        Rsvp::Accepted => "Accepted",
        Rsvp::Tentative => "Tentative",
        Rsvp::Declined => "Declined",
    };
    let title = event.summary.clone().unwrap_or_default();
    let who = card.account.display_name().unwrap_or(card.account.address().as_str()).to_owned();
    let text = match answer {
        Rsvp::Accepted => format!("{who} is going: {title}"),
        Rsvp::Tentative => format!("{who} might go: {title}"),
        Rsvp::Declined => format!("{who} is not going: {title}"),
    };
    let mut references: Vec<MessageRef> = envelope.references().to_vec();
    references.extend(envelope.message_id().cloned());
    let draft = Draft {
        to: vec![organizer.as_str().to_owned()],
        subject: format!("{verb}: {title}"),
        text,
        answers: Some(card.message.id()),
        in_reply_to: envelope.message_id().cloned(),
        references,
        // Sent at once: no copy goes to the Drafts folder.
        synced: Some(now),
        calendar: Some(reply),
        ..Draft::new(crate::draft::next_id(context.state()), card.account.id(), now)
    };
    let sending = super::send::sending(context, INVITE_ANSWER.id, &draft)?;
    let state = context.store().message_state(card.message.id())?;
    let answered = MessageState { invitation: Some(Box::new(Invitation { answer: Some(answer), ..card.mark.clone() })), ..state };
    let mut outcome = Outcome::new(Value::Null)
        .with_change(Change::PutDraft(draft.clone()))
        .with_change(sending.outgoing)
        .with_job(sending.account, sending.job)
        .with_change(Change::SetMessageState(card.message.id(), answered));
    let in_calendar = match (answer, &card.held) {
        (Rsvp::Declined, Some(held)) => {
            let job = CalendarJob::Delete { calendar: held.calendar, event: held.id, url: held.href.clone(), etag: held.etag.clone() };
            outcome = outcome.with_change(Change::RemoveCalendarEvent(held.id)).with_kept_job(card.account.id(), JobKind::Calendar(job));
            None
        }
        (Rsvp::Declined, None) => None,
        // One occurrence: its series is not ours to change from here.
        _ if card.mark.recurrence.is_some() => card.held.as_ref().map(|held| held.id),
        (_, Some(held)) => {
            let ical = invite::copy(object, &card.mark.uid, calendar::read(held).as_ref(), &owner, answer).ok_or_else(|| bad("the event cannot be written"))?;
            let job = CalendarJob::Put { calendar: held.calendar, event: held.id, url: held.href.clone(), etag: held.etag.clone(), ical: ical.clone() };
            outcome = outcome.with_change(Change::PutCalendarEvent(CalendarEvent { ical, ..held.clone() })).with_kept_job(card.account.id(), JobKind::Calendar(job));
            Some(held.id)
        }
        (_, None) => match context.store().calendars(card.account.id())?.into_iter().min_by_key(|held| held.id) {
            Some(into) => {
                let ical = invite::copy(object, &card.mark.uid, None, &owner, answer).ok_or_else(|| bad("the event cannot be written"))?;
                let id = CalendarEventId::new(context.state().next_event);
                let slash = if into.href.ends_with('/') { "" } else { "/" };
                let href = format!("{}{slash}inboxcraft-{}-{:09}-{}.ics", into.href, now.unix_seconds(), now.subsec_nanos(), id.get());
                let job = CalendarJob::Put { calendar: into.id, event: id, url: href.clone(), etag: None, ical: ical.clone() };
                let event = CalendarEvent { id, calendar: into.id, href, etag: None, ical };
                outcome = outcome.with_change(Change::PutCalendarEvent(event)).with_kept_job(card.account.id(), JobKind::Calendar(job));
                Some(id)
            }
            None => None,
        },
    };
    let mut answer_value = sending.answer;
    if let Value::Object(fields) = &mut answer_value {
        fields.insert("draft".into(), json!(draft.id.get()));
        fields.insert("id".into(), json!(params.id));
        fields.insert("answer".into(), json!(answer_name(answer)));
        fields.insert("calendar_event".into(), json!(in_calendar.map(CalendarEventId::get)));
    }
    *outcome.value_mut() = answer_value;
    Ok(outcome)
}

fn invite_remove(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode(INVITE_REMOVE.id, params)?;
    let bad = |reason: &str| EngineError::bad_params(INVITE_REMOVE.id, reason);
    let entry = context.store().message(MessageId::new(params.id))?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let card = card(context, &entry)?.ok_or_else(|| bad("the message carries no invitation"))?;
    let held = card.held.as_ref().filter(|_| card.removable()).ok_or_else(|| bad("the message cancels no event in the calendar"))?;
    let mut outcome = (DELETE.run)(context, &json!({ "id": held.id.get() }))?;
    if let Value::Object(fields) = outcome.value_mut() {
        fields.insert("event".into(), json!(held.id.get()));
        fields.insert("id".into(), json!(params.id));
    }
    Ok(outcome)
}
