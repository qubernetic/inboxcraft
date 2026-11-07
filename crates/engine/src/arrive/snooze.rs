// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The snooze stage, and a snoozed message's way there and back.
//!
//! A snoozed message lives in the account's `InboxCraft/Snoozed`, in a copy that carries the time it comes back in
//! InboxCraft's own header field ([`SNOOZED_UNTIL`]), so another InboxCraft on the same account sees it. The stage
//! reads the field of mail that arrives in that folder: a message another InboxCraft snoozed gets its time and its
//! schedule entry here too. At the time ([`back`]) the message comes back to the inbox, unread, marked as returned and
//! listed as if it arrived then.

use inboxcraft_model::{Due, Folder, Message, MessageId, MessageState, Role, SNOOZED_UNTIL, ScheduleId, Scheduled};
use inboxcraft_store::{Change, Storage};
use inboxcraft_time::{MailDate, Offset, Timestamp, Zone};

use super::Arrival;
use crate::EngineError;
use crate::act::{self, Effects, Mover};
use crate::session::State;
use crate::sync::Plan;

/// The stage: a message that arrives in the snoozed folder with a time in [`SNOOZED_UNTIL`] is snoozed until then.
pub(crate) fn stage(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    if arrival.role != Some(Role::Snoozed) {
        return Ok(());
    }
    let Some(until) = message.summary().envelope().snoozed_until() else { return Ok(()) };
    let id = ScheduleId::new(plan.state.next_schedule);
    plan.state.next_schedule = plan.state.next_schedule.saturating_add(1);
    plan.change(Change::SetMessageState(message.id(), MessageState { snoozed_until: Some(until), ..MessageState::default() }));
    plan.change(Change::PutScheduled(Scheduled { id, due: until, what: Due::Unsnooze(message.id()) }));
    Ok(())
}

/// `raw` with [`SNOOZED_UNTIL`] on top, saying `until` in UTC; `None` for a time no RFC 5322 date can say.
pub(crate) fn with_field(raw: &[u8], until: Timestamp) -> Option<Vec<u8>> {
    let date = MailDate::new(until, Zone::Fixed(Offset::UTC))?;
    let mut copy = format!("{SNOOZED_UNTIL}: {date}\r\n").into_bytes();
    copy.extend_from_slice(raw);
    Some(copy)
}

/// The changes that make the schedule bring `message` back at `until` and at no other time: its other entries go,
/// and one for `until` comes under `id` when there is none.
pub(crate) fn reschedule(store: &dyn Storage, message: MessageId, until: Option<Timestamp>, id: ScheduleId) -> Result<Vec<Change>, EngineError> {
    let mut changes = Vec::new();
    let mut kept = false;
    for entry in store.schedule()?.into_iter().filter(|entry| entry.what == Due::Unsnooze(message)) {
        if Some(entry.due) == until && !kept {
            kept = true;
        } else {
            changes.push(Change::RemoveScheduled(entry.id));
        }
    }
    if let Some(due) = until.filter(|_| !kept) {
        changes.push(Change::PutScheduled(Scheduled { id, due, what: Due::Unsnooze(message) }));
    }
    Ok(changes)
}

/// The folder of `message`'s account with `role`, if it has one.
pub(crate) fn role_folder(store: &dyn Storage, state: &State, message: &Message, role: Role) -> Result<Option<Folder>, EngineError> {
    match state.accounts.get(&message.account()).and_then(|held| held.roles.folder(role)) {
        Some(id) => act::folder(store, message.account(), id),
        None => Ok(None),
    }
}

/// Whether `message` is in its account's snoozed folder.
pub(crate) fn is_snoozed(state: &State, message: &Message) -> bool {
    state.accounts.get(&message.account()).and_then(|held| held.roles.role_of(message.folder())) == Some(Role::Snoozed)
}

/// Brings `message` back to its account's inbox, unread, moved by `mover`: the changes and jobs into `effects`, and the
/// inbox it went to back (`None` when the account has none, and it stays where it is, unread).
pub(crate) fn back(store: &dyn Storage, state: &State, message: &Message, mover: &mut Mover, effects: &mut Effects) -> Result<Option<Folder>, EngineError> {
    let message = act::mark_seen(store, state, message.clone(), false, effects)?;
    let Some(inbox) = role_folder(store, state, &message, Role::Inbox)? else { return Ok(None) };
    if let Some(moved) = mover.relocate(store, state, &message, &inbox)? {
        effects.extend(moved);
    }
    Ok(Some(inbox))
}

#[cfg(test)]
#[path = "snooze_tests.rs"]
mod tests;
