// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The follow-up stage, and which messages the owner sent.
//!
//! A follow-up reminder ("remind me if no reply by ...") waits on a message the owner sent: its time is in the
//! message's state (`follow_up`) and its schedule entry is a [`Due::FollowUp`]. A reply that arrives while the reminder
//! waits cancels it: a message whose `In-Reply-To` or `References` names the sent message's `Message-ID`, not from one
//! of the owner's own addresses and not in a Sent or Drafts folder (the owner's own answer in the thread is no reply).
//! Without a `Message-ID` on the sent message no reply can be told, and the reminder fires at its time. A reminder
//! that is not cancelled fires at its time (`crate::schedule`) and tells the owner (`crate::notify`), the notice
//! opening the sent message.

use inboxcraft_model::{Due, Envelope, Message, MessageId, MessageRef, MessageState, Role, ScheduleId};
use inboxcraft_store::{Change, Storage};

use super::Arrival;
use super::screener::sender_of;
use crate::EngineError;
use crate::schedule::reschedule;
use crate::session::State;
use crate::sync::Plan;

/// The stage: a reply to a sent message with a reminder waiting cancels the reminder.
pub(crate) fn stage(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    let envelope = message.summary().envelope();
    if envelope.in_reply_to().is_empty() && envelope.references().is_empty() {
        return Ok(());
    }
    if matches!(arrival.role, Some(Role::Sent | Role::Drafts)) || own(arrival.store, message)? {
        return Ok(());
    }
    let store = arrival.store;
    for entry in store.schedule()? {
        let Due::FollowUp(sent) = entry.what else { continue };
        let answered = store.message(sent)?.is_some_and(|held| held.message().summary().envelope().message_id().is_some_and(|id| answers(envelope, id)));
        if !answered {
            continue;
        }
        let state = planned(plan, sent).map_or_else(|| store.message_state(sent), Ok)?;
        if state.follow_up.is_some() {
            plan.change(Change::SetMessageState(sent, MessageState { follow_up: None, ..state }));
            for change in reschedule(store, Due::FollowUp(sent), None, ScheduleId::new(plan.state.next_schedule))? {
                plan.change(change);
            }
        }
    }
    Ok(())
}

/// Whether `envelope` names `id` as the message it answers or one before it in the thread.
fn answers(envelope: &Envelope, id: &MessageRef) -> bool {
    envelope.in_reply_to().iter().chain(envelope.references()).any(|named| named == id)
}

/// The state an earlier change of `plan` gives `message`, if one does.
fn planned(plan: &Plan, message: MessageId) -> Option<MessageState> {
    plan.batch.changes().iter().rev().find_map(|change| match change {
        Change::SetMessageState(id, state) if *id == message => Some(state.clone()),
        _ => None,
    })
}

/// Whether `message` is from one of the owner's own addresses.
fn own(store: &dyn Storage, message: &Message) -> Result<bool, EngineError> {
    let Some(from) = sender_of(message) else { return Ok(false) };
    Ok(store.accounts()?.iter().any(|account| account.address().as_str().eq_ignore_ascii_case(from.as_str())))
}

/// Whether the owner sent `message`: it is in its account's Sent folder, or from one of the owner's own addresses.
pub(crate) fn is_sent(store: &dyn Storage, state: &State, message: &Message) -> Result<bool, EngineError> {
    let role = state.accounts.get(&message.account()).and_then(|held| held.roles.role_of(message.folder()));
    Ok(role == Some(Role::Sent) || own(store, message)?)
}
