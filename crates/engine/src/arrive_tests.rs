// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The arriving-mail pipeline: each new message through each stage, in order.

use inboxcraft_model::{AccountId, FolderId, JobKind, Message, MessageId, Role};
use inboxcraft_store::{Batch, Change};

use super::{Arrival, STAGES, Stage, arrived};
use crate::EngineError;
use crate::cmd::act_tests::{inboxcraft_crate_message, session};
use crate::sync::Plan;
use crate::test_support::at;

/// A plan over a state that knows the inbox's UIDVALIDITY.
fn plan() -> Plan {
    let mut state = crate::session::State::default();
    state.validity.insert(FolderId::new(1), crate::cmd::act_tests::validity());
    Plan { state, batch: Batch::new(), jobs: Vec::new() }
}

/// Notes that it saw the message, by marking it opened.
fn first(_: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    plan.change(Change::MarkOpened(message.id()));
    Ok(())
}

/// Notes that it saw the message after `first`, by taking its body off.
fn second(_: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    plan.change(Change::RemoveBody(message.id()));
    Ok(())
}

/// Files mail arriving in the inbox's first sync into the archive, as a screener would.
fn file_away(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    if arrival.role == Some(Role::Inbox) && arrival.first_sync && arrival.now == at(crate::test_support::NOW) && arrival.folder == message.folder() {
        let archive = crate::act::folder(arrival.store, arrival.account, FolderId::new(3))?.ok_or(EngineError::NothingToUndo)?;
        let state = plan.state.clone();
        if let Some(effects) = crate::act::relocate(arrival.store, &state, message, &archive)? {
            plan.effects(effects);
        }
    }
    Ok(())
}

fn refuses(_: &Arrival<'_>, _: &Message, _: &mut Plan) -> Result<(), EngineError> {
    Err(EngineError::NothingToUndo)
}

fn arrival(session: &crate::Session, first_sync: bool) -> Arrival<'_> {
    Arrival {
        store: session.store(),
        now: at(crate::test_support::NOW),
        account: AccountId::new(1),
        folder: FolderId::new(1),
        role: Some(Role::Inbox),
        first_sync,
    }
}

#[test]
fn the_screener_comes_first_then_snooze_then_follow_up_and_the_notify_stage_last() {
    assert_eq!(STAGES.len(), 4, "a stage is a file of its own in arrive/ and a name in the stages! line");
}

#[test]
fn each_message_passes_every_stage_in_order() {
    let session = session();
    let mut plan = plan();
    let messages = [inboxcraft_crate_message(1, 1, 10), inboxcraft_crate_message(2, 1, 11)];
    arrived(&[first as Stage, second], &arrival(&session, false), &messages, &mut plan).unwrap();
    let (one, two) = (MessageId::new(1), MessageId::new(2));
    assert_eq!(plan.batch.changes(), [Change::MarkOpened(one), Change::RemoveBody(one), Change::MarkOpened(two), Change::RemoveBody(two)]);
}

#[test]
fn a_stage_may_act_on_the_message() {
    let session = session();
    let mut plan = plan();
    let message = inboxcraft_crate_message(1, 1, 10);
    arrived(&[file_away as Stage], &arrival(&session, false), std::slice::from_ref(&message), &mut plan).unwrap();
    assert!(plan.batch.is_empty(), "only on the first sync");
    arrived(&[file_away as Stage], &arrival(&session, true), &[message], &mut plan).unwrap();
    assert!(matches!(plan.batch.changes(), [Change::PutMessage(moved)] if moved.folder() == FolderId::new(3)));
    assert!(plan.jobs.iter().all(|job| job.keep && matches!(job.kind, JobKind::Move { .. })));
    assert_eq!(plan.jobs.len(), 1);
}

#[test]
fn a_stage_that_fails_stops_the_pipeline() {
    let session = session();
    let mut plan = plan();
    let messages = [inboxcraft_crate_message(1, 1, 10)];
    assert_eq!(arrived(&[refuses as Stage, first], &arrival(&session, false), &messages, &mut plan), Err(EngineError::NothingToUndo));
    assert!(plan.batch.is_empty());
}
