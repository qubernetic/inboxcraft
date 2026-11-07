// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The screener's commands: its mode, the senders waiting, accept and block, and their undo.

use inboxcraft_model::{AccountId, Flag, FolderId, Group, JobKind, MessageState, Screening};
use inboxcraft_store::{Body, Change, Storage};
use serde_json::{Value, json};

use crate::ScreenerMode;
use crate::arrive::screener::tests::{arrive, arrived, decided, from, known, mode, path_of, sender, settle};
use crate::cmd::act_tests::session;
use crate::test_support::change;
use crate::{Caller, EngineError, Session};

/// A session in `mode` with mail from `new@example.com` (UIDs 20 and 21) and `other@example.com` (22) arrived.
fn screened(name: &str) -> Session {
    screened_in(session(), name)
}

fn screened_in(mut session: Session, name: &str) -> Session {
    mode(&mut session, name);
    arrive(&mut session, 1, vec![from(20, "new@example.com"), from(21, "new@example.com"), from(22, "other@example.com")]);
    settle(&mut session);
    session
}

fn undo(session: &mut Session) -> Value {
    session.run("history.undo", &json!({})).unwrap()
}

fn moves_to(session: &mut Session, to: FolderId) -> usize {
    session.take_jobs().into_iter().filter(|job| matches!(&job.kind, JobKind::Move { to: target, .. } if *target == to)).count()
}

#[test]
fn the_mode_is_the_owners_and_the_session_keeps_it() {
    let mut session = session();
    assert_eq!(session.screener(), ScreenerMode::Off);
    assert_eq!(session.run("screener.mode", &json!({ "mode": "inside" })).unwrap(), json!({ "mode": "inside", "learned": 0 }));
    assert_eq!(session.screener(), ScreenerMode::Inside);
    let refused = session.run_as(Caller::Agent, "screener.mode", &json!({ "mode": "off" }));
    assert!(matches!(refused, Err(EngineError::NotAllowed { .. })));
    assert_eq!(session.run("screener.mode", &json!({ "mode": "sometimes" })).unwrap_err().code(), "bad_params");
    assert_eq!(session.run("screener.mode", &json!({ "mode": "before" })).unwrap(), json!({ "mode": "before", "learned": 0 }));
    session.set_screener(ScreenerMode::Off);
    assert_eq!(session.screener(), ScreenerMode::Off);
    assert_eq!(ScreenerMode::ALL.map(ScreenerMode::name), ["before", "inside", "off"]);
}

#[test]
fn turning_the_screener_on_learns_the_senders_the_owner_has_mail_from_or_wrote_to() {
    let mut session = session();
    let junk = crate::test_support::message(9, 1, 6, 40, "Win a prize", None);
    change(&mut session, Change::PutMessage(junk));
    change(&mut session, decided("anna@example.org", Screening::Blocked));
    session.run("screener.mode", &json!({ "mode": "before" })).unwrap();
    // The test mailbox: every message is from anna@example.org, to bob@example.net; both are the owner's own.
    assert!(sender(&session, "bob@example.net").is_none());
    let anna = sender(&session, "anna@example.org").unwrap();
    assert_eq!((anna.known, anna.screening), (false, Some(Screening::Blocked)), "own addresses are not learned");
    let mut session = crate::cmd::act_tests::session();
    let stranger = inboxcraft_model::Message::new(
        inboxcraft_model::MessageId::new(9),
        AccountId::new(1),
        FolderId::new(3),
        from(41, "kept@example.com").with_received(None),
    );
    let trash =
        inboxcraft_model::Message::new(inboxcraft_model::MessageId::new(10), AccountId::new(1), FolderId::new(6), from(42, "gone@example.com"));
    let sent = inboxcraft_model::Message::new(inboxcraft_model::MessageId::new(11), AccountId::new(1), FolderId::new(2), from(43, "x@example.com"));
    for message in [stranger, trash, sent] {
        change(&mut session, Change::PutMessage(message));
    }
    assert_eq!(session.run("screener.mode", &json!({ "mode": "inside" })).unwrap()["learned"], json!(1));
    assert!(sender(&session, "kept@example.com").unwrap().known);
    assert!(sender(&session, "gone@example.com").is_none(), "not from the trash");
    assert!(sender(&session, "x@example.com").is_none(), "a sent message teaches its recipients, here the owner");
}

#[test]
fn the_list_shows_each_sender_waiting_with_the_mail_held() {
    let mut session = screened("before");
    let answer = session.run_as(Caller::Agent, "screener.list", &json!({})).unwrap();
    assert_eq!(answer["mode"], json!("before"));
    let senders = answer["senders"].as_array().unwrap();
    assert_eq!(senders.len(), 2);
    let new = senders.iter().find(|entry| entry["address"] == json!("new@example.com")).unwrap();
    assert_eq!(new["name"], json!("Someone"));
    assert_eq!(new["messages"].as_array().unwrap().len(), 2);
    assert_eq!(new["accounts"], json!([1]));
    // Reading one is a preview: it accepts nothing.
    let id = new["messages"][0].as_u64().unwrap();
    session.run("mail.read", &json!({ "id": id })).unwrap();
    assert_eq!(sender(&session, "new@example.com"), None);
    assert_eq!(session.run("screener.list", &json!({})).unwrap()["senders"].as_array().unwrap().len(), 2);
}

#[test]
fn inside_the_inbox_the_list_shows_the_marked_mail() {
    let mut session = screened("inside");
    let answer = session.run("screener.list", &json!({})).unwrap();
    assert_eq!(answer["senders"].as_array().unwrap().len(), 2, "{answer}");
}

#[test]
fn accept_lets_the_held_mail_and_the_future_mail_in_and_undo_takes_it_back() {
    let mut session = screened("before");
    let screener = arrived(&session, 20).folder();
    let answer = session.run("screener.accept", &json!({ "address": " New@Example.com " })).unwrap();
    assert_eq!(answer["address"], json!("new@example.com"));
    assert_eq!(answer["moved"], json!(2));
    for n in [20, 21] {
        assert_eq!(arrived(&session, n).folder(), FolderId::new(1));
    }
    assert_eq!(arrived(&session, 22).folder(), screener, "another sender's mail stays");
    assert_eq!(moves_to(&mut session, FolderId::new(1)), 2);
    let accepted = sender(&session, "new@example.com").unwrap();
    assert!(accepted.known && accepted.screening == Some(Screening::Accepted) && accepted.group.is_none());
    arrive(&mut session, 1, vec![from(23, "new@example.com")]);
    assert_eq!(arrived(&session, 23).folder(), FolderId::new(1));

    let undone = undo(&mut session);
    assert_eq!(undone, json!({ "command": "screener.accept", "address": "new@example.com", "undone": true }));
    for n in [20, 21] {
        assert_eq!(arrived(&session, n).folder(), screener, "{n}");
    }
    assert_eq!(arrived(&session, 23).folder(), FolderId::new(1), "mail that came in meanwhile stays");
    assert_eq!(sender(&session, "new@example.com"), None);
}

#[test]
fn accept_with_a_group_keeps_the_group() {
    let mut session = screened("before");
    session.run("screener.accept", &json!({ "address": "new@example.com", "group": "newsletters" })).unwrap();
    assert_eq!(sender(&session, "new@example.com").unwrap().group, Some(Group::Newsletters));
    assert_eq!(session.run("screener.accept", &json!({ "address": "new@example.com", "group": "friends" })).unwrap_err().code(), "bad_params");
}

#[test]
fn undo_puts_back_what_was_known_before() {
    let mut session = screened("before");
    let record = inboxcraft_model::Sender {
        known: false,
        screening: Some(Screening::Blocked),
        group: Some(Group::Updates),
        ..inboxcraft_model::Sender::new(inboxcraft_model::EmailAddress::new("new@example.com").unwrap())
    };
    change(&mut session, Change::PutSender(record.clone()));
    session.run("screener.accept", &json!({ "address": "new@example.com", "group": "people" })).unwrap();
    undo(&mut session);
    assert_eq!(sender(&session, "new@example.com"), Some(record));
}

#[test]
fn accept_and_open_marks_the_newest_read_and_names_it() {
    let mut session = screened("before");
    let answer = session.run("screener.accept", &json!({ "address": "new@example.com", "then": "open" })).unwrap();
    let newest = arrived(&session, 21);
    assert_eq!(answer["open"], json!(newest.id().get()));
    assert!(newest.summary().flags().contains(&Flag::Seen));
    assert!(!arrived(&session, 20).summary().flags().contains(&Flag::Seen));
    let jobs: Vec<JobKind> = session.take_jobs().into_iter().map(|job| job.kind).collect();
    assert!(matches!(&jobs[0], JobKind::StoreFlags { add, .. } if add.contains(&Flag::Seen)), "the flag first, then the move: {jobs:?}");
    undo(&mut session);
    let back = arrived(&session, 21);
    assert!(!back.summary().flags().contains(&Flag::Seen));
    assert_eq!(path_of(&session, back.folder()), "InboxCraft/Screener");
}

#[test]
fn accept_and_done_files_the_held_mail_as_done() {
    let mut session = screened("before");
    let answer = session.run("screener.accept", &json!({ "address": "new@example.com", "then": "done" })).unwrap();
    assert_eq!(answer["moved"], json!(2));
    for n in [20, 21] {
        let message = arrived(&session, n);
        assert_eq!(message.folder(), FolderId::new(3), "the archive");
        assert_eq!(session.store().message_state(message.id()).unwrap().done, Some(FolderId::new(1)), "done from the inbox");
    }
    undo(&mut session);
    for n in [20, 21] {
        let message = arrived(&session, n);
        assert_eq!(path_of(&session, message.folder()), "InboxCraft/Screener");
        assert_eq!(session.store().message_state(message.id()).unwrap(), MessageState::default());
    }
}

#[test]
fn accept_and_done_needs_an_archive() {
    let mut store = crate::cmd::act_tests::store();
    store.apply(&inboxcraft_store::Batch::new().with(Change::RemoveFolder(FolderId::new(3)))).unwrap();
    let mut session = screened_in(crate::test_support::session_with(store, crate::Registry::builtin().unwrap()), "before");
    let error = session.run("screener.accept", &json!({ "address": "new@example.com", "then": "done" })).unwrap_err();
    assert_eq!(error.code(), "not_found");
    assert_eq!(sender(&session, "new@example.com"), None, "nothing changed");
    assert_eq!(session.run("screener.accept", &json!({ "address": "new@example.com", "then": "later" })).unwrap_err().code(), "bad_params");
}

#[test]
fn block_files_the_held_and_the_future_mail_away_and_undo_takes_it_back() {
    let mut session = screened("before");
    let screener = arrived(&session, 20).folder();
    let answer = session.run("screener.block", &json!({ "address": "new@example.com" })).unwrap();
    assert_eq!(answer["moved"], json!(2));
    let blocked = arrived(&session, 20).folder();
    assert_eq!(path_of(&session, blocked), "InboxCraft/Blocked");
    assert_eq!(arrived(&session, 21).folder(), blocked);
    assert_eq!(sender(&session, "new@example.com").unwrap().screening, Some(Screening::Blocked));
    let jobs: Vec<JobKind> = session.take_jobs().into_iter().map(|job| job.kind).collect();
    assert!(matches!(&jobs[0], JobKind::CreateFolder { path } if path.name() == "InboxCraft/Blocked"), "{jobs:?}");
    arrive(&mut session, 1, vec![from(23, "new@example.com")]);
    assert_eq!(arrived(&session, 23).folder(), blocked);
    undo(&mut session);
    for n in [20, 21] {
        assert_eq!(arrived(&session, n).folder(), screener, "{n}");
    }
    assert_eq!(sender(&session, "new@example.com"), None);
}

#[test]
fn block_inside_the_inbox_takes_the_marked_mail_out() {
    let mut session = screened("inside");
    session.run("screener.block", &json!({ "address": "new@example.com" })).unwrap();
    assert_eq!(path_of(&session, arrived(&session, 20).folder()), "InboxCraft/Blocked");
    assert_eq!(arrived(&session, 22).folder(), FolderId::new(1));
    undo(&mut session);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(1));
    // A known sender's mail stays where it is; only the future mail is blocked.
    change(&mut session, known("other@example.com"));
    assert_eq!(session.run("screener.block", &json!({ "address": "other@example.com" })).unwrap()["moved"], json!(0));
    assert_eq!(arrived(&session, 22).folder(), FolderId::new(1));
}

#[test]
fn accept_takes_blocked_mail_back_in() {
    let mut session = screened("before");
    session.run("screener.block", &json!({ "address": "new@example.com" })).unwrap();
    assert_eq!(session.run("screener.accept", &json!({ "address": "new@example.com" })).unwrap()["moved"], json!(2));
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(1));
}

#[test]
fn a_blocked_message_is_never_fetched_and_shows_only_what_is_kept() {
    let mut session = screened("before");
    session.run("screener.block", &json!({ "address": "new@example.com" })).unwrap();
    let id = arrived(&session, 20).id().get();
    session.run("remote.allow", &json!({ "address": "new@example.com" })).unwrap();
    session.take_jobs();
    let answer = session.run("mail.read", &json!({ "id": id })).unwrap();
    assert_eq!(answer["fetching"], json!(false));
    assert_eq!(answer["blocked"], json!(true));
    assert_eq!(answer["body_error"], json!("not_downloaded"));
    assert_eq!(answer["remote_allowed"], json!(false), "remote content never loads for a blocked sender");
    assert!(!session.take_jobs().iter().any(|job| matches!(job.kind, JobKind::FetchBody { .. })));
    let body = Body::new(b"Subject: Hello\r\n\r\nKept text\r\n".to_vec(), "Kept text").unwrap();
    change(&mut session, Change::PutBody(inboxcraft_model::MessageId::new(id), body));
    let answer = session.run("mail.read", &json!({ "id": id })).unwrap();
    assert_eq!(answer["body_error"], Value::Null);
    assert!(answer["message"]["body"].is_object());
    // An ordinary message is still fetched.
    let answer = session.run("mail.read", &json!({ "id": arrived(&session, 22).id().get() })).unwrap();
    assert_eq!(answer["blocked"], json!(false));
}

#[test]
fn only_the_owner_accepts_or_blocks_and_an_agent_cannot_undo_it() {
    let mut session = screened("before");
    for command in ["screener.accept", "screener.block"] {
        assert!(matches!(session.run_as(Caller::Agent, command, &json!({ "address": "new@example.com" })), Err(EngineError::NotAllowed { .. })));
    }
    session.run("screener.accept", &json!({ "address": "new@example.com" })).unwrap();
    assert!(matches!(session.run_as(Caller::Agent, "history.undo", &json!({})), Err(EngineError::NotAllowed { .. })));
}

#[test]
fn an_address_that_is_not_one_is_refused_and_one_with_nothing_held_is_still_decided() {
    let mut session = screened("before");
    for bad in ["", "not an address", "a@b\r\nBcc: x@y"] {
        assert_eq!(session.run("screener.block", &json!({ "address": bad })).unwrap_err().code(), "bad_params", "{bad:?}");
    }
    assert_eq!(session.run("screener.accept", &json!({ "address": "nobody@example.com" })).unwrap()["moved"], json!(0));
    assert_eq!(sender(&session, "nobody@example.com").unwrap().screening, Some(Screening::Accepted));
    undo(&mut session);
    assert_eq!(sender(&session, "nobody@example.com"), None);
}

#[test]
fn undo_after_the_messages_went_skips_them() {
    let mut session = screened("before");
    session.run("screener.accept", &json!({ "address": "new@example.com" })).unwrap();
    let id = arrived(&session, 20).id();
    change(&mut session, Change::RemoveMessage(id));
    assert_eq!(undo(&mut session)["undone"], json!(true));
    assert_eq!(path_of(&session, arrived(&session, 21).folder()), "InboxCraft/Screener");
}
