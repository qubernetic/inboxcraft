// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Done, park and pin: `mail.done`, `mail.park`, `mail.pin`, the park pile and the pins view, "show done" in
//! `mail.list`, the setting that keeps a pin through done, and the undo of each.

use inboxcraft_model::{AccountId, Copied, ErrorKind, Event, EventKind, FolderId, JobKind, MessageId, MessageState};
use inboxcraft_store::Change;
use serde_json::{Value, json};

use crate::arrive::screener::tests::settle;
use crate::cmd::act_tests::{held, path, session, uid, uids, validity};
use crate::test_support::change;
use crate::{AgentClass, Caller, Registry, Session};

fn state(session: &Session, id: u64) -> MessageState {
    session.store().message_state(MessageId::new(id)).unwrap()
}

fn undo(session: &mut Session) -> Value {
    session.run("history.undo", &json!({})).unwrap()
}

/// The ids `answer` lists, in order.
fn ids(answer: &Value) -> Vec<u64> {
    answer["messages"].as_array().unwrap().iter().map(|shown| shown["id"].as_u64().unwrap()).collect()
}

/// The row of message `id` in `answer`.
fn row(answer: &Value, id: u64) -> Value {
    answer["messages"].as_array().unwrap().iter().find(|shown| shown["id"] == json!(id)).cloned().unwrap()
}

fn done(session: &mut Session, id: u64, done: bool) -> Value {
    session.run("mail.done", &json!({ "id": id, "done": done })).unwrap()
}

#[test]
fn done_park_and_pin_have_their_agent_classes() {
    let registry = Registry::builtin().unwrap();
    let class = |id: &str| registry.lookup(id).map(|(_, class)| class);
    for id in ["mail.park", "mail.pin", "park.list", "pin.list"] {
        assert_eq!(class(id), Some(AgentClass::Safe), "{id}");
    }
    for id in ["mail.done", "pin.set"] {
        assert_eq!(class(id), Some(AgentClass::Owner), "{id}");
    }
    let mut session = session();
    assert_eq!(session.run_as(Caller::Agent, "mail.done", &json!({ "id": 1, "done": true })).unwrap_err().code(), "not_allowed");
    assert_eq!(session.run_as(Caller::Agent, "pin.set", &json!({ "keep_on_done": true })).unwrap_err().code(), "not_allowed");
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
}

#[test]
fn done_files_a_message_in_the_archive_and_keeps_where_it_was() {
    let mut session = session();
    let answer = done(&mut session, 1, true);
    assert_eq!(answer, json!({ "id": 1, "done": true, "folder": 3, "was": 1, "changed": true, "pinned": false }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(3));
    assert_eq!(state(&session, 1), MessageState { done: Some(FolderId::new(1)), ..MessageState::default() });
    let jobs: Vec<JobKind> = session.take_jobs().into_iter().map(|job| job.kind).collect();
    let moved = JobKind::Move {
        folder: FolderId::new(1),
        path: path("INBOX"),
        uid_validity: validity(),
        uids: uids(&[10]),
        to: FolderId::new(3),
        to_path: path("Archive"),
    };
    assert_eq!(jobs, [moved]);
    let archive = session.run("mail.list", &json!({ "role": "archive" })).unwrap();
    assert_eq!(row(&archive, 1)["done"], json!(true));
}

#[test]
fn show_done_lists_done_mail_in_its_old_place() {
    let mut session = session();
    done(&mut session, 1, true);
    let inbox = session.run("mail.list", &json!({ "account": 1 })).unwrap();
    assert_eq!(ids(&inbox), [2], "done mail leaves the inbox");
    assert_eq!(row(&inbox, 2)["done"], json!(false));
    let shown = session.run("mail.list", &json!({ "account": 1, "done": true })).unwrap();
    assert_eq!(ids(&shown), [2, 1], "newest first, the done among them");
    assert_eq!(shown["total"], json!(2));
    assert_eq!(row(&shown, 1)["done"], json!(true));
    assert_eq!(row(&shown, 1)["folder"], json!(3), "it is in the archive");
    let folder = session.run("mail.list", &json!({ "folder": 1, "done": true })).unwrap();
    assert_eq!(ids(&folder), [2, 1]);
    let elsewhere = session.run("mail.list", &json!({ "role": "sent", "done": true })).unwrap();
    assert_eq!(ids(&elsewhere), [5]);
    let archive = session.run("mail.list", &json!({ "role": "archive", "done": true })).unwrap();
    assert_eq!(ids(&archive), [1], "listed once where it is");
}

#[test]
fn undo_takes_done_back_to_its_place() {
    let mut session = session();
    done(&mut session, 1, true);
    settle(&mut session);
    assert_eq!(session.run_as(Caller::Agent, "history.undo", &json!({})).unwrap_err().code(), "not_allowed");
    assert_eq!(undo(&mut session), json!({ "command": "mail.done", "id": 1, "undone": true }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
    assert!(state(&session, 1).is_empty());
    assert_eq!(session.take_jobs().len(), 1, "the move back goes to the server");
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn done_again_changes_nothing_and_leaves_nothing_to_undo() {
    let mut session = session();
    done(&mut session, 1, true);
    session.run("mail.flag", &json!({ "id": 2, "flagged": true })).unwrap();
    undo(&mut session);
    let again = done(&mut session, 1, true);
    assert_eq!(again["changed"], json!(false));
    assert_eq!(undo(&mut session)["command"], json!("mail.done"), "the first done is still on top");
    assert_eq!(done(&mut session, 2, false), json!({ "id": 2, "done": false, "folder": 1, "changed": false }));
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

/// Files message 1 as done and answers its move out of the inbox with `kind`.
fn done_and_answer(session: &mut Session, kind: impl FnOnce(inboxcraft_model::JobId) -> EventKind) {
    done(session, 1, true);
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let job = jobs.first().unwrap().id;
    session.apply(&Event::new(AccountId::new(1), Some(job), kind(job))).unwrap();
    session.take_jobs();
}

#[test]
fn done_whose_move_failed_files_it_again() {
    let mut session = session();
    done_and_answer(&mut session, |_| EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(FolderId::new(1)) });
    assert_eq!(held(&session, 1).folder(), FolderId::new(1), "the server kept it in the inbox");
    let again = done(&mut session, 1, true);
    assert_eq!(again, json!({ "id": 1, "done": true, "folder": 3, "was": 1, "changed": true, "pinned": false }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(3));
    assert_eq!(state(&session, 1).done, Some(FolderId::new(1)));
    let jobs: Vec<JobKind> = session.take_jobs().into_iter().map(|job| job.kind).collect();
    assert!(matches!(jobs.as_slice(), [JobKind::Move { to, .. }] if *to == FolderId::new(3)), "{jobs:?}");
}

#[test]
fn done_keeps_its_state_once_the_server_names_the_new_uid() {
    let mut session = session();
    session.set_pins_outlast_done(true);
    session.run("mail.pin", &json!({ "id": 1, "pinned": true })).unwrap();
    let copied = Copied { uid_validity: validity(), uids: [(uid(10), uid(55))].into_iter().collect() };
    done_and_answer(&mut session, |_| EventKind::Moved {
        folder: FolderId::new(1),
        uid_validity: validity(),
        uids: uids(&[10]),
        to: FolderId::new(3),
        copied: Some(copied),
    });
    let message = held(&session, 1);
    assert_eq!((message.folder(), message.summary().uid(), message.moving()), (FolderId::new(3), uid(55), None));
    assert_eq!(state(&session, 1), MessageState { done: Some(FolderId::new(1)), pinned: true, ..MessageState::default() });
    let shown = session.run("mail.list", &json!({ "account": 1, "done": true })).unwrap();
    assert_eq!(ids(&shown), [2, 1]);
    assert_eq!(done(&mut session, 1, false)["folder"], json!(1), "the way back is still there");
}

#[test]
fn not_done_brings_a_message_back_and_undo_files_it_again() {
    let mut session = session();
    done(&mut session, 1, true);
    session.take_jobs();
    assert_eq!(done(&mut session, 1, false), json!({ "id": 1, "done": false, "folder": 1, "changed": true }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
    assert!(state(&session, 1).is_empty());
    assert_eq!(undo(&mut session), json!({ "command": "mail.done", "id": 1, "undone": true }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(3));
    assert_eq!(state(&session, 1).done, Some(FolderId::new(1)));
}

#[test]
fn done_in_the_archive_moves_nothing() {
    let mut session = session();
    let archived = crate::test_support::message(9, 1, 3, 30, "Old news", Some(crate::test_support::NOW));
    change(&mut session, Change::PutMessage(archived));
    assert_eq!(done(&mut session, 9, true), json!({ "id": 9, "done": true, "folder": 3, "was": 3, "changed": true, "pinned": false }));
    assert!(session.take_jobs().is_empty());
    assert_eq!(state(&session, 9).done, Some(FolderId::new(3)));
    assert_eq!(undo(&mut session)["undone"], json!(true));
    assert!(state(&session, 9).is_empty());
    assert_eq!(held(&session, 9).folder(), FolderId::new(3));
}

#[test]
fn not_done_whose_old_place_is_gone_stays_where_it_is() {
    let mut session = session();
    let state = MessageState { done: Some(FolderId::new(99)), parked: true, ..MessageState::default() };
    change(&mut session, Change::SetMessageState(MessageId::new(1), state));
    assert_eq!(done(&mut session, 1, false), json!({ "id": 1, "done": false, "folder": 1, "changed": true }));
    assert_eq!(self::state(&session, 1), MessageState { parked: true, ..MessageState::default() }, "the rest of its state stays");
    assert!(session.take_jobs().is_empty());
}

#[test]
fn done_refuses_what_does_not_fit() {
    let mut session = session();
    let error = session.run("mail.done", &json!({ "id": 3, "done": true })).unwrap_err();
    assert_eq!(error.code(), "not_found", "account 2 has no archive: {error}");
    assert_eq!(session.run("mail.done", &json!({ "id": 99, "done": true })).unwrap_err().code(), "not_found");
    assert_eq!(session.run("mail.done", &json!({ "id": 1 })).unwrap_err().code(), "bad_params");
    for id in ["mail.park", "mail.pin"] {
        assert_eq!(session.run(id, &json!({ "id": 99, "parked": true, "pinned": true })).unwrap_err().code(), "bad_params", "{id}");
    }
    assert_eq!(session.run("mail.park", &json!({ "id": 99, "parked": true })).unwrap_err().code(), "not_found");
    assert_eq!(session.run("mail.pin", &json!({ "id": 99, "pinned": true })).unwrap_err().code(), "not_found");
    assert!(state(&session, 3).is_empty());
}

#[test]
fn the_park_pile_leaves_the_list_and_undo_puts_it_back() {
    let mut session = session();
    assert_eq!(
        session.run_as(Caller::Agent, "mail.park", &json!({ "id": 2, "parked": true })).unwrap(),
        json!({ "id": 2, "parked": true, "changed": true })
    );
    assert!(state(&session, 2).parked);
    assert!(session.take_jobs().is_empty(), "nothing goes to the server");
    let inbox = session.run("mail.list", &json!({ "account": 1 })).unwrap();
    assert_eq!(ids(&inbox), [1]);
    assert_eq!(inbox["parked"], json!(1), "for the pill");
    let all = session.run("mail.list", &json!({ "account": 1, "parked": true })).unwrap();
    assert_eq!(ids(&all), [2, 1]);
    assert_eq!(row(&all, 2)["parked"], json!(true));
    let pile = session.run("park.list", &json!({})).unwrap();
    assert_eq!(ids(&pile), [2]);
    assert_eq!(pile["total"], json!(1));

    assert_eq!(session.run_as(Caller::Agent, "history.undo", &json!({})).unwrap(), json!({ "command": "mail.park", "id": 2, "undone": true }));
    assert!(state(&session, 2).is_empty());
    assert_eq!(ids(&session.run("park.list", &json!({})).unwrap()), Vec::<u64>::new());
    assert_eq!(session.run("mail.park", &json!({ "id": 2, "parked": false })).unwrap()["changed"], json!(false));
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn pins_have_their_view_across_accounts() {
    let mut session = session();
    assert_eq!(
        session.run_as(Caller::Agent, "mail.pin", &json!({ "id": 3, "pinned": true })).unwrap(),
        json!({ "id": 3, "pinned": true, "changed": true })
    );
    session.run("mail.pin", &json!({ "id": 1, "pinned": true })).unwrap();
    let pins = session.run("pin.list", &json!({})).unwrap();
    assert_eq!(ids(&pins), [3, 1], "newest first");
    assert_eq!(pins["keep_on_done"], json!(false));
    assert_eq!(row(&pins, 1)["pinned"], json!(true));
    assert_eq!(ids(&session.run("pin.list", &json!({ "account": 2 })).unwrap()), [3]);
    let page = session.run("pin.list", &json!({ "offset": 1, "limit": 1 })).unwrap();
    assert_eq!((ids(&page), page["total"].clone()), (vec![1], json!(2)));
    assert_eq!(session.run("pin.list", &json!({ "account": 9 })).unwrap_err().code(), "not_found");
    let inbox = session.run("mail.list", &json!({ "account": 1 })).unwrap();
    assert_eq!(row(&inbox, 1)["pinned"], json!(true), "a pin stays in its list too");

    assert_eq!(undo(&mut session), json!({ "command": "mail.pin", "id": 1, "undone": true }));
    assert_eq!(ids(&session.run("pin.list", &json!({})).unwrap()), [3]);
    session.run("mail.pin", &json!({ "id": 3, "pinned": false })).unwrap();
    assert!(state(&session, 3).is_empty());
}

#[test]
fn done_takes_a_pin_off_unless_the_setting_keeps_it() {
    let mut session = session();
    session.run("mail.pin", &json!({ "id": 1, "pinned": true })).unwrap();
    assert_eq!(done(&mut session, 1, true)["pinned"], json!(false));
    assert!(!state(&session, 1).pinned);
    undo(&mut session);
    assert!(state(&session, 1).pinned, "the undo gives the pin back");

    assert!(!session.pins_outlast_done());
    assert_eq!(session.run("pin.set", &json!({ "keep_on_done": true })).unwrap(), json!({ "keep_on_done": true }));
    assert!(session.pins_outlast_done());
    assert_eq!(done(&mut session, 1, true)["pinned"], json!(true));
    assert_eq!(state(&session, 1), MessageState { done: Some(FolderId::new(1)), pinned: true, ..MessageState::default() });
    assert_eq!(session.run("pin.list", &json!({})).unwrap()["keep_on_done"], json!(true));
    session.set_pins_outlast_done(false);
    assert!(!session.pins_outlast_done());
    assert_eq!(session.run("pin.set", &json!({})).unwrap_err().code(), "bad_params");
}

#[test]
fn an_undo_whose_message_is_gone_undoes_nothing() {
    let mut session = session();
    session.run("mail.park", &json!({ "id": 2, "parked": true })).unwrap();
    change(&mut session, Change::RemoveMessage(MessageId::new(2)));
    assert_eq!(undo(&mut session), json!({ "command": "mail.park", "id": 2, "undone": false }));
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn an_undo_whose_old_folder_is_gone_restores_the_state_only() {
    let mut session = session();
    let archived = crate::test_support::message(9, 1, 6, 30, "In the trash", None);
    change(&mut session, Change::PutMessage(archived));
    done(&mut session, 9, true);
    change(&mut session, Change::RemoveFolder(FolderId::new(6)));
    assert_eq!(undo(&mut session)["undone"], json!(true));
    assert_eq!(held(&session, 9).folder(), FolderId::new(3));
    assert!(state(&session, 9).is_empty());
}
