// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `schedule.presets`, `schedule.list` and `schedule.cancel`.

use inboxcraft_model::{AccountId, Draft, DraftId, Due, MessageId, ScheduleId, Scheduled};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{FakeClock, Offset, OffsetTable};
use serde_json::{Value, json};

use crate::test_support::{NOW, at, mailbox};
use crate::{AgentClass, Caller, EngineError, Session};

fn session() -> Session {
    let mut store = mailbox();
    let batch = Batch::new()
        .with(Change::PutDraft(Draft::new(DraftId::new(3), AccountId::new(1), at(NOW))))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(2), due: at(NOW + 3600), what: Due::Send(DraftId::new(3)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(1), due: at(NOW - 60), what: Due::Unsnooze(MessageId::new(2)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(4), due: at(NOW + 60), what: Due::FollowUp(MessageId::new(5)) }));
    store.apply(&batch).unwrap();
    Session::new(Box::new(store), Box::new(FakeClock::new(at(NOW)))).unwrap()
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

#[test]
fn the_classes_are_safe_for_reading_and_owner_for_cancelling() {
    let session = session();
    for (id, class) in [("schedule.presets", AgentClass::Safe), ("schedule.list", AgentClass::Safe), ("schedule.cancel", AgentClass::Owner)] {
        assert_eq!(session.registry().get(id).map(|spec| spec.class), Some(Some(class)), "{id}");
    }
}

#[test]
fn presets_answers_the_presets_ahead_in_utc_until_the_offsets_are_given() {
    // NOW is 09:00 UTC on 5 November 2026.
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "schedule.presets", &json!({})).unwrap();
    assert_eq!(
        answer,
        json!({ "presets": [
            { "preset": "later_today", "at": "2026-11-05T12:00:00Z" },
            { "preset": "this_evening", "at": "2026-11-05T20:00:00Z" },
            { "preset": "tomorrow_morning", "at": "2026-11-06T08:00:00Z" },
        ] })
    );
    session.set_offsets(Box::new(OffsetTable::new(Offset::from_minutes(60).unwrap())));
    let answer = session.run("schedule.presets", &json!({})).unwrap();
    assert_eq!(answer["presets"][1], json!({ "preset": "this_evening", "at": "2026-11-05T19:00:00Z" }));
    assert!(bad(session.run("schedule.presets", &json!({ "at": "now" }))));
}

#[test]
fn list_answers_every_entry_by_its_time_with_what_it_does() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "schedule.list", &json!({})).unwrap();
    assert_eq!(
        answer,
        json!({ "entries": [
            { "id": 1, "due": "2026-11-05T08:59:00Z", "what": "unsnooze", "message": 2 },
            { "id": 4, "due": "2026-11-05T09:01:00Z", "what": "follow_up", "message": 5 },
            { "id": 2, "due": "2026-11-05T10:00:00Z", "what": "send", "draft": 3 },
        ] })
    );
    assert!(bad(session.run("schedule.list", &json!({ "id": 1 }))));
}

#[test]
fn list_names_an_alert_by_its_id() {
    assert_eq!(super::schedule::what(&Due::Alert(inboxcraft_model::AlertId::new(9))), json!({ "what": "alert", "alert": 9 }));
}

#[test]
fn cancel_takes_one_entry_off_and_only_for_the_owner() {
    let mut session = session();
    let refused = session.run_as(Caller::Agent, "schedule.cancel", &json!({ "id": 2 }));
    assert!(matches!(refused, Err(EngineError::NotAllowed { .. })));
    assert_eq!(session.run("schedule.cancel", &json!({ "id": 2 })).unwrap(), json!({ "cancelled": 2 }));
    let left: Vec<u64> = session.store().schedule().unwrap().into_iter().map(|entry| entry.id.get()).collect();
    assert_eq!(left, vec![1, 4]);
    assert!(session.store().draft(DraftId::new(3)).unwrap().is_some(), "the draft stays");
}

#[test]
fn cancel_refuses_an_entry_that_is_not_there_and_junk() {
    let mut session = session();
    let missing = session.run("schedule.cancel", &json!({ "id": 99 })).unwrap_err();
    assert_eq!(missing.code(), "not_found");
    for junk in [json!({}), json!({ "id": "2" }), json!({ "id": -1 }), json!({ "id": 2, "more": true })] {
        assert!(bad(session.run("schedule.cancel", &junk)), "{junk}");
    }
    assert_eq!(session.store().schedule().unwrap().len(), 3);
}
