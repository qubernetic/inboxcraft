// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Invitations: the iTIP messages (RFC 5546) mail carries (iMIP, RFC 6047), read from a message's body once it is
//! kept.
//!
//! [`read`] finds the message's calendar object: the first `text/calendar` part of its own (itself or a part of its
//! multiparts, not inside an attached message), read with `inboxcraft_ical`. [`linked`] marks the message with what
//! the owner needs of it ([`inboxcraft_model::Invitation`]): the `METHOD`, the event by `UID`, `RECURRENCE-ID` and
//! `SEQUENCE`, its `DTSTAMP`, title, times, repeat rule and place; an answer the owner gave to the same version stays.
//! Mail in the Blocked folder is not marked. Two of the organiser's marks (`REQUEST`, `CANCEL`) are versions of one
//! event when they name the same `UID` and `RECURRENCE-ID`; the later one has the higher `SEQUENCE`, then the later
//! `DTSTAMP` (RFC 5546 section 2.1.5). A `REPLY` is no version: the owner's own answer, synced back from Sent, dates
//! nothing.
//!
//! An invitation changes nothing on its own: it is data, never instructions. The calendar and the organiser hear of
//! it only when the owner answers (`invite.answer`) or takes a cancelled event out (`invite.remove`, see
//! `crate::cmd`).

use inboxcraft_ical::{Attendee, Calendar, Event, PartStat, When};
use inboxcraft_mime::{Body, Part};
use inboxcraft_model::{Invitation, ItipMethod, Message, MessageState, Role, Rsvp};
use inboxcraft_store::{Change, Storage};
use inboxcraft_time::{Offsets, Timestamp};

use super::followup::planned;
use crate::EngineError;
use crate::sync::Plan;

/// The most characters of a title or a place kept in a mark.
pub(crate) const MAX_TEXT: usize = 1000;

/// The longest `UID` a mark keeps, in bytes; an invitation with a longer one is not marked.
pub(crate) const MAX_UID: usize = 1000;

/// The calendar object of the message `raw`: its first `text/calendar` part of its own, read; `None` when it has none
/// or it cannot be read.
pub(crate) fn read(raw: &[u8]) -> Option<Calendar> {
    let message = inboxcraft_mime::parse(raw).ok()?;
    let mut parts: Vec<&Part<'_>> = vec![&message];
    while let Some(part) = parts.pop() {
        if part.content_type().is("text", "calendar") {
            let text = part.text().ok()?;
            return Calendar::parse(text.as_bytes()).ok();
        }
        if let Body::Multipart(multipart) = part.body() {
            // Last first, so the parts are looked at in their order.
            parts.extend(multipart.parts().iter().rev());
        }
    }
    None
}

/// What `calendar` does, by its `METHOD`.
pub(crate) fn method(calendar: &Calendar) -> ItipMethod {
    match calendar.method.as_deref().map(str::to_ascii_uppercase).as_deref() {
        Some("REQUEST") => ItipMethod::Request,
        Some("CANCEL") => ItipMethod::Cancel,
        Some("REPLY") => ItipMethod::Reply,
        _ => ItipMethod::Other,
    }
}

/// The event an invitation is about: its first event without a `RECURRENCE-ID`, else its first.
pub(crate) fn event(calendar: &Calendar) -> Option<&Event> {
    crate::calendar::master(calendar).or_else(|| calendar.events.first())
}

/// The mark of the invitation `calendar`, its times on the wall clock `local` where it names none; `None` without an
/// event, or with an empty or too long `UID`.
pub(crate) fn mark(calendar: &Calendar, local: &dyn Offsets) -> Option<Invitation> {
    let event = event(calendar).filter(|event| !event.uid.is_empty() && event.uid.len() <= MAX_UID)?;
    let cut = |text: &Option<String>| text.as_ref().map(|text| text.chars().take(MAX_TEXT).collect::<String>());
    let (start, end) = times(calendar, event, local);
    Some(Invitation {
        method: method(calendar),
        uid: event.uid.clone(),
        recurrence: event.recurrence_id.as_ref().map(written),
        sequence: event.sequence,
        stamp: event.stamp,
        summary: cut(&event.summary),
        start,
        end,
        all_day: event.all_day(),
        repeat: event.rules.first().map(ToString::to_string),
        location: cut(&event.location),
        answer: None,
    })
}

/// When `event` of `calendar` starts and ends: its end, or its start and duration, or a day after a whole day's start,
/// or its start.
pub(crate) fn times(calendar: &Calendar, event: &Event, local: &dyn Offsets) -> (Option<Timestamp>, Option<Timestamp>) {
    let start = calendar.zones.resolve(&event.start, local);
    let end = match (&event.end, event.duration) {
        (Some(end), _) => calendar.zones.resolve(end, local),
        (None, Some(span)) => start.and_then(|start| {
            let seconds = span.days().checked_mul(86_400)?.checked_add(span.seconds())?;
            Timestamp::from_unix(start.unix_seconds().checked_add(seconds)?, 0)
        }),
        (None, None) if event.all_day() => start.and_then(|start| start.checked_add(std::time::Duration::from_secs(86_400))),
        (None, None) => start,
    };
    (start, end)
}

/// A `RECURRENCE-ID` as iCalendar writes its value.
fn written(when: &When) -> String {
    when.to_property("RECURRENCE-ID").value().to_owned()
}

/// Whether `a` and `b` are versions of one event (or of one occurrence of it).
pub(crate) fn same_event(a: &Invitation, b: &Invitation) -> bool {
    a.uid == b.uid && a.recurrence == b.recurrence
}

/// Whether `mark` is one the organiser sends (`REQUEST`, `CANCEL`): only those are versions of an event a card
/// compares; an attendee's `REPLY` changes no version.
pub(crate) fn from_organiser(mark: &Invitation) -> bool {
    matches!(mark.method, ItipMethod::Request | ItipMethod::Cancel)
}

/// Whether `a` is a later version than `b`: a higher `SEQUENCE`, or the same one stamped later.
pub(crate) fn later(a: &Invitation, b: &Invitation) -> bool {
    (a.sequence, a.stamp) > (b.sequence, b.stamp)
}

/// The body `raw` of `message` is kept: the message is marked with the invitation it carries, or no longer.
pub(crate) fn linked(store: &dyn Storage, plan: &mut Plan, message: &Message, raw: &[u8], local: &dyn Offsets) -> Result<(), EngineError> {
    let roles = plan.state.accounts.get(&message.account()).map(|held| &held.roles);
    if roles.and_then(|roles| roles.role_of(message.folder())) == Some(Role::Blocked) {
        return Ok(());
    }
    let state = planned(plan, message.id()).map_or_else(|| store.message_state(message.id()), Ok)?;
    let invitation = read(raw).and_then(|calendar| mark(&calendar, local)).map(|mark| {
        let was = state.invitation.as_deref().filter(|was| same_event(was, &mark) && !later(was, &mark) && !later(&mark, was));
        Box::new(Invitation { answer: was.and_then(|was| was.answer), ..mark })
    });
    if invitation != state.invitation {
        plan.change(Change::SetMessageState(message.id(), MessageState { invitation, ..state }));
    }
    Ok(())
}

/// The `PARTSTAT` of `answer`.
fn part_stat(answer: Rsvp) -> PartStat {
    match answer {
        Rsvp::Accepted => PartStat::Accepted,
        Rsvp::Tentative => PartStat::Tentative,
        Rsvp::Declined => PartStat::Declined,
    }
}

/// The owner as an account answers for them: the account's address and name, and every address that is theirs.
pub(crate) struct Owner {
    /// The address the answer comes from.
    pub(crate) address: String,
    /// The account's display name.
    pub(crate) name: Option<String>,
    /// The owner's addresses, lower case: the account's, and its login where that is an address.
    pub(crate) own: Vec<String>,
}

impl Owner {
    /// The owner of `account`.
    pub(crate) fn of(account: &inboxcraft_model::Account) -> Self {
        let address = account.address().as_str().to_owned();
        let mut own = vec![address.to_lowercase()];
        if inboxcraft_model::EmailAddress::new(account.username()).is_ok() {
            own.push(account.username().to_lowercase());
        }
        Self { address, name: account.display_name().map(str::to_owned), own }
    }

    /// Whether `attendee` is the owner.
    pub(crate) fn is(&self, attendee: &Attendee) -> bool {
        self.own.contains(&crate::calendar::address_of(attendee).to_lowercase())
    }
}

/// The owner as an attendee who gave `answer`: as `event` names them among its attendees (name and parameters kept),
/// or as the account's address and name.
fn answering(event: &Event, owner: &Owner, answer: Rsvp) -> Attendee {
    let mut attendee = event
        .attendees
        .iter()
        .find(|attendee| owner.is(attendee))
        .cloned()
        .unwrap_or_else(|| Attendee { name: owner.name.clone(), ..Attendee::new(&format!("mailto:{}", owner.address)) });
    attendee.status = part_stat(answer);
    attendee.rsvp = false;
    attendee
}

/// The owner's iTIP answer (RFC 5546 section 3.2.3) to `event` of the invitation `calendar`: `METHOD:REPLY` with the
/// event's `UID`, `SEQUENCE`, `RECURRENCE-ID`, organiser, start (a moment in UTC, or a day) and title, stamped
/// `now`, and the owner alone among the attendees with `answer`. `None` when it cannot be written.
pub(crate) fn reply(calendar: &Calendar, event: &Event, owner: &Owner, answer: Rsvp, now: Timestamp, local: &dyn Offsets) -> Option<String> {
    let start = match &event.start {
        When::Date(_) => event.start.clone(),
        other => calendar.zones.resolve(other, local).map_or_else(|| other.clone(), When::Utc),
    };
    let recurrence_id = event.recurrence_id.as_ref().map(|when| match when {
        When::Date(_) => when.clone(),
        other => calendar.zones.resolve(other, local).map_or_else(|| other.clone(), When::Utc),
    });
    let mut answered = Event::new(&event.uid, start);
    answered.sequence = event.sequence;
    answered.stamp = Some(now);
    answered.recurrence_id = recurrence_id;
    answered.summary.clone_from(&event.summary);
    answered.organizer.clone_from(&event.organizer);
    answered.attendees = vec![answering(event, owner, answer)];
    let mut object = Calendar::new(crate::calendar::PRODUCT);
    object.method = Some("REPLY".to_owned());
    object.events.push(answered);
    object.write().ok()
}

/// The invitation `calendar` as it goes into the owner's calendar after `answer`: no `METHOD`, the events of its
/// `UID` with the owner's answer, the reminders the owner set on the copy held (`held`) kept and the organiser's left
/// out, and nobody mailed by a server that schedules on its own (`SCHEDULE-AGENT=CLIENT`). `None` when it cannot be
/// written.
pub(crate) fn copy(calendar: &Calendar, uid: &str, held: Option<&Calendar>, owner: &Owner, answer: Rsvp) -> Option<String> {
    let mut object = calendar.clone();
    object.method = None;
    object.product = crate::calendar::PRODUCT.to_owned();
    object.events.retain(|event| event.uid == uid);
    for event in &mut object.events {
        let answered = answering(event, owner, answer);
        match event.attendees.iter_mut().find(|attendee| owner.is(attendee)) {
            Some(listed) => *listed = answered,
            None => event.attendees.push(answered),
        }
        // The reminders are the owner's: those on the copy held, else none.
        let was = held.and_then(|held| held.events.iter().find(|was| was.uid == event.uid && was.recurrence_id == event.recurrence_id));
        event.alarms = was.map(|was| was.alarms.clone()).unwrap_or_default();
        crate::calendar::unscheduled(event);
    }
    object.write().ok()
}

#[cfg(test)]
#[path = "invite_tests.rs"]
pub(crate) mod tests;
