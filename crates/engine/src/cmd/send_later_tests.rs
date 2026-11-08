// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Undo send and send later, on a fake clock: every send is held before it goes and can be taken back with its draft
//! as it was; a send later goes at its time; a time missed while InboxCraft was off goes at the next start, with a
//! notice.

use std::time::Duration;

use inboxcraft_model::{AccountId, DraftId, Due, JobKind, OutboxState};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use super::send_tests::store;
use crate::notify::Notice;
use crate::send::Hold;
use crate::test_support::{NOW, at};
use crate::{AgentClass, Caller, EngineError, Session};

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (Session::new(Box::new(store()), Box::new(clock.clone())).unwrap(), clock)
}

/// A draft of account 1 to Béla with the subject `subject`; its id.
fn draft(session: &mut Session, subject: &str) -> u64 {
    let id = session.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": id, "to": ["Béla <bela@example.org>"], "subject": subject, "text": "The numbers." })).unwrap();
    session.take_jobs();
    id
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

/// The drafts of the send jobs queued since the last call.
fn sends(session: &mut Session) -> Vec<DraftId> {
    session.take_jobs().into_iter().filter_map(|job| if let JobKind::Send { draft, .. } = job.kind { Some(draft) } else { None }).collect()
}

fn state(session: &Session, id: u64) -> Option<OutboxState> {
    session.store().outbox().unwrap().into_iter().find(|item| item.draft == DraftId::new(id)).map(|item| item.state)
}

fn send_entries(session: &Session) -> Vec<(DraftId, i64)> {
    session
        .store()
        .schedule()
        .unwrap()
        .into_iter()
        .filter_map(|entry| if let Due::Send(draft) = entry.what { Some((draft, entry.due.unix_seconds())) } else { None })
        .collect()
}

#[test]
fn the_commands_have_their_classes() {
    let (session, _) = session();
    let class = |id: &str| session.registry().get(id).map(|spec| spec.class);
    for id in ["mail.send", "mail.send_later", "mail.unsend", "send.set"] {
        assert_eq!(class(id), Some(Some(AgentClass::Owner)), "{id}");
    }
    assert_eq!(class("send.settings"), Some(Some(AgentClass::Safe)));
}

#[test]
fn every_send_is_held_and_goes_when_the_hold_is_over() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    let answer = session.run("mail.send", &json!({ "id": id })).unwrap();
    assert_eq!(answer, json!({ "id": id, "state": "waiting", "at": "2026-11-05T09:00:05Z", "hold_seconds": 5 }));
    // Nothing goes yet: the draft waits in the outbox, its send on the schedule.
    assert_eq!(sends(&mut session), []);
    assert_eq!(state(&session, id), Some(OutboxState::Waiting));
    assert_eq!(send_entries(&session), [(DraftId::new(id), NOW + 5)]);
    assert_eq!(session.next_due().unwrap(), Some(at(NOW + 5)));
    let listed = session.run("outbox.list", &json!({})).unwrap();
    assert_eq!((&listed["items"][0]["state"], &listed["items"][0]["at"]), (&json!("waiting"), &json!("2026-11-05T09:00:05Z")));
    // A second send of the same draft while it waits is refused.
    assert!(bad(session.run("mail.send", &json!({ "id": id }))));
    clock.advance(Duration::from_secs(4));
    assert_eq!(session.tick().unwrap(), []);
    assert_eq!(sends(&mut session), []);
    clock.advance(Duration::from_secs(1));
    let fired = session.tick().unwrap();
    assert!(matches!(fired.as_slice(), [fired] if fired.entry.what == Due::Send(DraftId::new(id)) && !fired.missed), "{fired:?}");
    let jobs = session.take_jobs();
    let [job] = jobs.as_slice() else { panic!("{jobs:?}") };
    let JobKind::Send { draft, raw, .. } = &job.kind else { panic!("{job:?}") };
    assert_eq!((job.account, *draft), (AccountId::new(1), DraftId::new(id)));
    // The message is made when it goes: its date is the moment the hold ended.
    let text = String::from_utf8_lossy(raw);
    assert!(text.contains("Date: Thu, 5 Nov 2026 09:00:05 +0000\r\n"), "{text}");
    let outbox = session.store().outbox().unwrap();
    assert_eq!((&outbox[0].state, &outbox[0].message), (&OutboxState::Sending, raw));
    assert_eq!(send_entries(&session), []);
    assert_eq!(session.take_notices(), [], "a send on time tells nothing");
}

#[test]
fn undo_send_reopens_the_draft_intact() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    let before = session.run("compose.show", &json!({ "id": id })).unwrap();
    session.run("mail.send", &json!({ "id": id })).unwrap();
    // While it waits, the draft is neither changed nor discarded.
    assert!(bad(session.run("compose.update", &json!({ "id": id, "text": "Changed." }))));
    assert!(bad(session.run("compose.discard", &json!({ "id": id }))));
    // An agent may not take it back.
    assert!(matches!(session.run_as(Caller::Agent, "mail.unsend", &json!({ "id": id })), Err(EngineError::NotAllowed { .. })));
    clock.advance(Duration::from_secs(4));
    let answer = session.run("mail.unsend", &json!({ "id": id })).unwrap();
    assert_eq!(answer, json!({ "unsent": id, "draft": before }));
    assert_eq!(session.run("compose.show", &json!({ "id": id })).unwrap(), before);
    assert_eq!(state(&session, id), None);
    assert_eq!(send_entries(&session), []);
    clock.advance(Duration::from_secs(60));
    session.tick().unwrap();
    assert_eq!(sends(&mut session), [], "an undone send never goes");
    // It can be changed and sent again, held again.
    session.run("compose.update", &json!({ "id": id, "text": "Changed." })).unwrap();
    assert_eq!(session.run("mail.send", &json!({ "id": id })).unwrap()["at"], json!("2026-11-05T09:01:09Z"));
    clock.advance(Duration::from_secs(5));
    session.tick().unwrap();
    assert_eq!(sends(&mut session), [DraftId::new(id)]);
    // Once it went, it is too late.
    assert!(bad(session.run("mail.unsend", &json!({ "id": id }))));
    assert_eq!(state(&session, id), Some(OutboxState::Sending));
    // Nothing to take back, and junk.
    let other = draft(&mut session, "Other");
    assert!(matches!(session.run("mail.unsend", &json!({ "id": other })), Err(EngineError::NotFound { .. })));
    assert!(matches!(session.run("mail.unsend", &json!({ "id": 999 })), Err(EngineError::NotFound { .. })));
    assert!(bad(session.run("mail.unsend", &json!({ "id": "1" }))));
    assert!(bad(session.run("mail.unsend", &json!({ "id": id, "at": "x" }))));
}

#[test]
fn cancelling_a_send_on_the_schedule_takes_it_back_as_undo_send_does() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    session.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T12:00:00Z" })).unwrap();
    let entries = session.run("schedule.list", &json!({})).unwrap();
    assert_eq!(entries["entries"][0]["what"], json!("send"));
    let entry = entries["entries"][0]["id"].clone();
    session.run("schedule.cancel", &json!({ "id": entry })).unwrap();
    assert_eq!(state(&session, id), None, "no outbox item is left waiting for a time that never comes");
    session.run("compose.update", &json!({ "id": id, "text": "Changed." })).unwrap();
    clock.advance(Duration::from_secs(4 * 3600));
    session.tick().unwrap();
    assert_eq!(sends(&mut session), []);
}

#[test]
fn the_hold_is_five_to_sixty_seconds() {
    let (mut session, clock) = session();
    let settings = session.run("send.settings", &json!({})).unwrap();
    assert_eq!((&settings["hold_seconds"], &settings["min"], &settings["max"]), (&json!(5), &json!(5), &json!(60)));
    assert!(settings["note"].as_str().unwrap().contains("off"), "{settings}");
    assert_eq!(session.run("send.set", &json!({ "hold_seconds": 30 })).unwrap(), json!({ "hold_seconds": 30 }));
    assert_eq!(session.send_hold().seconds(), 30);
    assert_eq!(session.run("send.settings", &json!({})).unwrap()["hold_seconds"], json!(30));
    let id = draft(&mut session, "Budget");
    assert_eq!(session.run("mail.send", &json!({ "id": id })).unwrap()["hold_seconds"], json!(30));
    clock.advance(Duration::from_secs(29));
    session.tick().unwrap();
    assert_eq!(sends(&mut session), []);
    clock.advance(Duration::from_secs(1));
    session.tick().unwrap();
    assert_eq!(sends(&mut session), [DraftId::new(id)]);
    // Outside the range, junk, and an agent.
    for params in
        [json!({ "hold_seconds": 4 }), json!({ "hold_seconds": 61 }), json!({ "hold_seconds": -5 }), json!({ "hold_seconds": "5" }), json!({})]
    {
        assert!(bad(session.run("send.set", &params)), "{params}");
    }
    assert!(bad(session.run("send.settings", &json!({ "x": 1 }))));
    assert!(matches!(session.run_as(Caller::Agent, "send.set", &json!({ "hold_seconds": 60 })), Err(EngineError::NotAllowed { .. })));
    assert_eq!(session.send_hold().seconds(), 30);
    // The front end hands back what it kept.
    session.set_send_hold(Hold::new(60).unwrap());
    assert_eq!(session.run("send.settings", &json!({})).unwrap()["hold_seconds"], json!(60));
    assert_eq!((Hold::new(4), Hold::new(61), Hold::default().seconds()), (None, None, 5));
    assert_eq!(Hold::new(12).unwrap().duration(), Duration::from_secs(12));
}

#[test]
fn send_later_goes_at_its_time() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    let answer = session.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T12:00:00Z" })).unwrap();
    assert_eq!((&answer["id"], &answer["state"], &answer["at"]), (&json!(id), &json!("waiting"), &json!("2026-11-05T12:00:00Z")));
    // It says honestly that nothing goes while the computer is off.
    let note = answer["note"].as_str().unwrap();
    assert!(note.contains("off") && note.contains("next start"), "{note}");
    assert_eq!(send_entries(&session), [(DraftId::new(id), NOW + 3 * 3600)]);
    clock.advance(Duration::from_secs(3 * 3600 - 1));
    session.tick().unwrap();
    assert_eq!(sends(&mut session), []);
    clock.advance(Duration::from_secs(1));
    let fired = session.tick().unwrap();
    assert!(matches!(fired.as_slice(), [fired] if !fired.missed), "{fired:?}");
    assert_eq!(sends(&mut session), [DraftId::new(id)]);
    assert_eq!(session.take_notices(), []);
}

#[test]
fn send_later_takes_a_preset_and_refuses_a_time_gone() {
    let (mut session, _) = session();
    let id = draft(&mut session, "Budget");
    // 09:00 UTC: this evening is 20:00.
    let answer = session.run("mail.send_later", &json!({ "id": id, "preset": "this_evening" })).unwrap();
    assert_eq!(answer["at"], json!("2026-11-05T20:00:00Z"));
    // Taken back, then given another time.
    session.run("mail.unsend", &json!({ "id": id })).unwrap();
    for params in [
        json!({ "id": id, "at": "2026-11-05T08:00:00Z" }),
        json!({ "id": id, "at": "2026-11-05T09:00:00Z" }),
        json!({ "id": id, "at": "soon" }),
        json!({ "id": id, "preset": "next_year" }),
        json!({ "id": id, "at": "2026-11-05T12:00:00Z", "preset": "this_evening" }),
        json!({ "id": id }),
        json!({ "id": id, "at": 5 }),
    ] {
        assert!(bad(session.run("mail.send_later", &params)), "{params}");
    }
    assert!(matches!(session.run("mail.send_later", &json!({ "id": 999, "at": "2026-11-05T12:00:00Z" })), Err(EngineError::NotFound { .. })));
    assert!(matches!(
        session.run_as(Caller::Agent, "mail.send_later", &json!({ "id": id, "at": "2026-11-05T12:00:00Z" })),
        Err(EngineError::NotAllowed { .. })
    ));
    assert_eq!(state(&session, id), None);
    // A draft that cannot go is refused now, not at its time.
    let empty = session.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
    assert!(bad(session.run("mail.send_later", &json!({ "id": empty, "at": "2026-11-05T12:00:00Z" }))));
    // So is one already waiting.
    session.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T12:00:00Z" })).unwrap();
    assert!(bad(session.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T13:00:00Z" }))));
    assert!(bad(session.run("mail.send", &json!({ "id": id }))));
    assert_eq!(send_entries(&session), [(DraftId::new(id), NOW + 3 * 3600)]);
}

/// The store `session` leaves behind when the program stops: all a new start has.
fn left(session: &Session) -> Memory {
    let mut store = store();
    let mut batch = Batch::new();
    for draft in session.store().drafts(AccountId::new(1)).unwrap() {
        batch.push(Change::PutDraft(draft));
    }
    for item in session.store().outbox().unwrap() {
        batch.push(Change::PutOutgoing(item));
    }
    for entry in session.store().schedule().unwrap() {
        batch.push(Change::PutScheduled(entry));
    }
    store.apply(&batch).unwrap();
    store
}

#[test]
fn a_time_missed_while_inboxcraft_was_off_is_sent_at_the_next_start_with_a_notice() {
    let (mut session, _) = session();
    let id = draft(&mut session, "Budget");
    session.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T12:00:00Z" })).unwrap();
    let held = draft(&mut session, "Held");
    session.run("mail.send", &json!({ "id": held })).unwrap();
    // The program stops before either time; it starts again the next morning.
    let clock = FakeClock::new(at(NOW + 24 * 3600));
    let mut again = Session::new(Box::new(left(&session)), Box::new(clock.clone())).unwrap();
    assert_eq!(state(&again, id), Some(OutboxState::Waiting), "a send waiting for its time is not cut off");
    again.start();
    again.take_jobs();
    let fired = again.tick().unwrap();
    assert_eq!(fired.len(), 2);
    assert!(fired.iter().all(|fired| fired.missed), "{fired:?}");
    let mut sent = sends(&mut again);
    sent.sort();
    assert_eq!(sent, [DraftId::new(id), DraftId::new(held)]);
    let notices = again.take_notices();
    assert_eq!(
        notices,
        [
            Notice::SentLate { account: AccountId::new(1), draft: DraftId::new(held), subject: "Held".to_owned() },
            Notice::SentLate { account: AccountId::new(1), draft: DraftId::new(id), subject: "Budget".to_owned() },
        ],
        "in time order"
    );
    assert_eq!(notices[0].account(), AccountId::new(1));
    assert_eq!(notices[0].message(), None);
}

#[test]
fn a_send_handled_late_while_the_computer_slept_tells_too() {
    let (mut session, clock) = session();
    session.start();
    let id = draft(&mut session, "Budget");
    session.run("mail.send", &json!({ "id": id })).unwrap();
    // A minute after its time is on time still; more is late.
    clock.advance(Duration::from_secs(65));
    assert!(session.tick().unwrap().iter().all(|fired| !fired.missed));
    assert_eq!(session.take_notices(), []);
    let id = draft(&mut session, "Asleep");
    session.run("mail.send", &json!({ "id": id })).unwrap();
    clock.advance(Duration::from_secs(3600));
    assert!(session.tick().unwrap().iter().all(|fired| fired.missed));
    assert!(matches!(session.take_notices().as_slice(), [Notice::SentLate { subject, .. }] if subject == "Asleep"));
}

#[test]
fn a_send_that_can_no_longer_go_at_its_time_is_refused_with_why() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    session.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T12:00:00Z" })).unwrap();
    // Meanwhile the account lost its submission server.
    let account = session.store().accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    crate::test_support::change(&mut session, Change::PutAccount(account.with_submission(None)));
    clock.advance(Duration::from_secs(4 * 3600));
    let fired = session.tick().unwrap();
    assert_eq!(fired.len(), 1);
    assert_eq!(sends(&mut session), []);
    assert_eq!(state(&session, id), Some(OutboxState::Refused { reply: "the account has no submission server".to_owned() }));
    assert_eq!(session.take_notices(), [], "it is in the outbox, refused, not sent late");
    // Refused, it can be changed again.
    session.run("compose.update", &json!({ "id": id, "text": "Changed." })).unwrap();
}

#[test]
fn a_send_whose_outbox_item_is_gone_fires_nothing() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    session.run("mail.send", &json!({ "id": id })).unwrap();
    crate::test_support::change(&mut session, Change::RemoveOutgoing(DraftId::new(id)));
    clock.advance(Duration::from_secs(5));
    assert_eq!(session.tick().unwrap().len(), 1);
    assert_eq!(sends(&mut session), []);
    assert_eq!(state(&session, id), None);
}

#[test]
fn a_retry_is_held_as_a_send_is() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    session.run("mail.send", &json!({ "id": id })).unwrap();
    clock.advance(Duration::from_secs(5));
    session.tick().unwrap();
    session.take_jobs();
    let refused = inboxcraft_model::EventKind::SendFailed {
        draft: DraftId::new(id),
        kind: inboxcraft_model::ErrorKind::MessageRefused,
        reply: Some("550 5.1.1 No such user".to_owned()),
    };
    session.apply(&inboxcraft_model::Event::new(AccountId::new(1), None, refused)).unwrap();
    let answer = session.run("outbox.retry", &json!({ "id": id })).unwrap();
    assert_eq!((&answer["state"], &answer["at"]), (&json!("waiting"), &json!("2026-11-05T09:00:10Z")));
    assert_eq!(sends(&mut session), []);
    session.run("mail.unsend", &json!({ "id": id })).unwrap();
    assert_eq!(state(&session, id), None);
}

#[test]
fn a_draft_on_its_way_is_not_copied_to_the_drafts_folder_and_one_taken_back_is() {
    let (mut session, clock) = session();
    let id = draft(&mut session, "Budget");
    let uploads = |session: &mut Session| {
        session
            .take_jobs()
            .into_iter()
            .filter(|job| matches!(job.kind, JobKind::Append { draft: Some(draft), .. } if draft == DraftId::new(id)))
            .count()
    };
    session.run("send.set", &json!({ "hold_seconds": 60 })).unwrap();
    session.run("mail.send", &json!({ "id": id })).unwrap();
    clock.advance(Duration::from_secs(30));
    session.tick().unwrap();
    assert_eq!(uploads(&mut session), 0);
    session.run("mail.unsend", &json!({ "id": id })).unwrap();
    session.tick().unwrap();
    assert_eq!(uploads(&mut session), 1);
}

#[test]
fn a_front_end_asking_every_frame_ticks_only_when_something_is_due() {
    let (mut session, clock) = session();
    assert_eq!(session.tick_if_due().unwrap(), None, "nothing is due");
    let id = draft(&mut session, "Budget");
    // The draft's copy is due ten seconds after its last change; the send, sent now, five seconds from now.
    session.run("mail.send", &json!({ "id": id })).unwrap();
    assert_eq!(session.tick_if_due().unwrap(), None);
    clock.advance(Duration::from_secs(4));
    assert_eq!(session.tick_if_due().unwrap(), None, "the time it said is kept");
    clock.advance(Duration::from_secs(1));
    let fired = session.tick_if_due().unwrap().unwrap();
    assert!(matches!(fired.as_slice(), [fired] if fired.entry.what == Due::Send(DraftId::new(id))), "{fired:?}");
    assert_eq!(sends(&mut session), [DraftId::new(id)]);
    assert_eq!(session.tick_if_due().unwrap(), None);
}
