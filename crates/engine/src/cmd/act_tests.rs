// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The acts on messages (`mail.flag`, `mail.seen`, `mail.open`, `mail.move`, `mail.archive`, `mail.delete`): the
//! change at once, a kept job for the server, and an undo for each.

use inboxcraft_model::{
    AccountId, Flag, Flags, FolderId, FolderPath, FolderState, Job, JobKind, Message, MessageId, Moving, SpecialUse, Uid, UidSet, UidValidity,
};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use serde_json::{Value, json};

use crate::test_support::{folder, mailbox, sample_session, session_with};
use crate::{AgentClass, Caller, Registry, Session};

pub(crate) fn validity() -> UidValidity {
    UidValidity::new(7).unwrap()
}

pub(crate) fn uid(n: u32) -> Uid {
    Uid::new(n).unwrap()
}

pub(crate) fn uids(list: &[u32]) -> UidSet {
    list.iter().map(|n| uid(*n)).collect()
}

pub(crate) fn path(name: &str) -> FolderPath {
    FolderPath::new(name, Some('/')).unwrap()
}

pub(crate) fn flags(list: &[Flag]) -> Flags {
    list.iter().cloned().collect()
}

/// The test mailbox with a Trash (6) and a folder that holds no mail (7) for account 1, every folder of account 1
/// synced at UIDVALIDITY 7.
pub(crate) fn store() -> Memory {
    let mut store = mailbox();
    let mut noselect = folder(7, 1, "Projects", vec![]);
    noselect = inboxcraft_model::Folder::new(
        noselect.id(),
        noselect.account(),
        inboxcraft_model::FolderListing::new(noselect.listing().path().clone(), vec![], false),
    );
    let mut batch = Batch::new().with(Change::PutFolder(folder(6, 1, "Trash", vec![SpecialUse::Trash]))).with(Change::PutFolder(noselect));
    for id in [1, 2, 3, 6] {
        batch.push(Change::SetFolderState(FolderId::new(id), FolderState::new(validity())));
    }
    store.apply(&batch).unwrap();
    store
}

pub(crate) fn session() -> Session {
    session_with(store(), Registry::builtin().unwrap())
}

pub(crate) fn held(session: &Session, id: u64) -> Message {
    session.store().message(MessageId::new(id)).unwrap().unwrap().message().clone()
}

fn kinds(session: &mut Session) -> Vec<JobKind> {
    session.take_jobs().into_iter().map(|job| job.kind).collect()
}

fn queued(session: &Session) -> Vec<JobKind> {
    session.store().queued_jobs().unwrap().into_iter().map(|job: Job| job.kind).collect()
}

fn undo(session: &mut Session) -> Value {
    session.run("history.undo", &json!({})).unwrap()
}

fn store_flags(add: &[Flag], remove: &[Flag]) -> JobKind {
    JobKind::StoreFlags {
        folder: FolderId::new(1),
        path: path("INBOX"),
        uid_validity: validity(),
        uids: uids(&[10]),
        add: flags(add),
        remove: flags(remove),
    }
}

/// Where the server keeps message 1 before it moves: INBOX, UID 10.
fn in_inbox(with: &[Flag]) -> Moving {
    Moving { folder: FolderId::new(1), uid_validity: validity(), uid: uid(10), flags: flags(with) }
}

#[test]
fn the_acts_have_their_agent_classes() {
    let registry = Registry::builtin().unwrap();
    let class = |id: &str| registry.lookup(id).map(|(_, class)| class);
    for id in ["mail.flag", "mail.seen", "mail.open", "history.undo"] {
        assert_eq!(class(id), Some(AgentClass::Safe), "{id}");
    }
    for id in ["mail.move", "mail.archive", "mail.delete"] {
        assert_eq!(class(id), Some(AgentClass::Owner), "{id}");
    }
    let mut session = session();
    for (id, params) in [("mail.move", json!({ "id": 1, "folder": 3 })), ("mail.archive", json!({ "id": 1 })), ("mail.delete", json!({ "id": 1 }))] {
        assert_eq!(session.run_as(Caller::Agent, id, &params).unwrap_err().code(), "not_allowed", "{id}");
    }
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
}

#[test]
fn a_flag_shows_at_once_goes_to_the_server_and_is_undone() {
    let mut session = session();
    assert_eq!(session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap(), json!({ "id": 1, "flagged": true, "changed": true }));
    assert!(held(&session, 1).summary().flags().is_flagged());
    assert_eq!(kinds(&mut session), [store_flags(&[Flag::Flagged], &[])]);
    assert_eq!(queued(&session), [store_flags(&[Flag::Flagged], &[])], "kept until the server answers");

    assert_eq!(undo(&mut session), json!({ "command": "mail.flag", "id": 1, "undone": true }));
    assert!(!held(&session, 1).summary().flags().is_flagged());
    assert_eq!(kinds(&mut session), [store_flags(&[], &[Flag::Flagged])]);
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn a_flag_already_so_changes_nothing_and_leaves_nothing_to_undo() {
    let mut session = session();
    assert_eq!(session.run("mail.flag", &json!({ "id": 1, "flagged": false })).unwrap(), json!({ "id": 1, "flagged": false, "changed": false }));
    assert!(session.take_jobs().is_empty());
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn read_and_unread_go_to_the_server_and_are_undone() {
    let mut session = session();
    assert_eq!(session.run("mail.seen", &json!({ "id": 1, "seen": true })).unwrap(), json!({ "id": 1, "seen": true, "changed": true }));
    assert!(held(&session, 1).summary().flags().is_seen());
    assert_eq!(kinds(&mut session), [store_flags(&[Flag::Seen], &[])]);
    assert_eq!(session.run("mail.seen", &json!({ "id": 1, "seen": false })).unwrap()["changed"], json!(true));
    assert_eq!(kinds(&mut session), [store_flags(&[], &[Flag::Seen])]);

    assert_eq!(undo(&mut session), json!({ "command": "mail.seen", "id": 1, "undone": true }));
    assert!(held(&session, 1).summary().flags().is_seen(), "the unread is undone");
    assert_eq!(kinds(&mut session), [store_flags(&[Flag::Seen], &[])]);
    undo(&mut session);
    assert!(!held(&session, 1).summary().flags().is_seen(), "and then the read");
}

#[test]
fn opening_marks_read_and_the_undo_marks_it_unread_again() {
    let mut session = session();
    let answer = session.run("mail.open", &json!({ "id": 1 })).unwrap();
    assert_eq!(answer["marked_seen"], json!(true));
    assert_eq!(answer["message"]["seen"], json!(true));
    assert_eq!(answer["message"]["flags"], json!(["\\Seen"]));
    assert_eq!(answer["message"]["subject"], json!("Budget for 2026"));
    assert!(held(&session, 1).summary().flags().is_seen());
    assert!(session.store().message(MessageId::new(1)).unwrap().unwrap().opened());
    let jobs = kinds(&mut session);
    assert!(jobs.contains(&store_flags(&[Flag::Seen], &[])), "{jobs:?}");
    assert!(jobs.iter().any(|job| matches!(job, JobKind::FetchBody { .. })), "the body is fetched as by mail.read: {jobs:?}");

    assert_eq!(undo(&mut session), json!({ "command": "mail.open", "id": 1, "undone": true }));
    assert!(!held(&session, 1).summary().flags().is_seen());

    session.run("mail.seen", &json!({ "id": 2, "seen": true })).unwrap();
    let answer = session.run("mail.open", &json!({ "id": 2 })).unwrap();
    assert_eq!(answer["marked_seen"], json!(false));
    assert_eq!(answer["message"]["seen"], json!(true));
    assert_eq!(undo(&mut session)["command"], json!("mail.seen"), "opening a read message is no act of its own");
}

#[test]
fn opening_refuses_what_does_not_fit() {
    let mut session = session();
    assert_eq!(session.run("mail.open", &json!({ "id": 99 })).unwrap_err().code(), "not_found");
    assert_eq!(session.run("mail.open", &json!({})).unwrap_err().code(), "bad_params");
}

fn moved_to(folder: u64) -> JobKind {
    let to_path = match folder {
        3 => path("Archive"),
        6 => path("Trash"),
        _ => path("Sent"),
    };
    JobKind::Move { folder: FolderId::new(1), path: path("INBOX"), uid_validity: validity(), uids: uids(&[10]), to: FolderId::new(folder), to_path }
}

#[test]
fn a_moved_message_keeps_its_id_and_waits_for_its_server_uid() {
    let mut session = session();
    assert_eq!(session.run("mail.move", &json!({ "id": 1, "folder": 2 })).unwrap(), json!({ "id": 1, "folder": 2, "was": 1, "changed": true }));
    let message = held(&session, 1);
    assert_eq!(message.folder(), FolderId::new(2));
    assert_eq!(message.summary().uid(), uid(u32::MAX), "a UID of ours, from the top down");
    assert_eq!(message.moving(), Some(&in_inbox(&[])));
    assert_eq!(message.summary().envelope().subject(), "Budget for 2026");
    assert_eq!(kinds(&mut session), [moved_to(2)]);
    assert_eq!(queued(&session), [moved_to(2)]);
    let listed = session.run("mail.list", &json!({ "folder": 2 })).unwrap();
    let shown = listed["messages"].as_array().unwrap().iter().find(|shown| shown["id"] == json!(1)).cloned().unwrap();
    assert_eq!((shown["uid"].clone(), shown["moving"].clone()), (Value::Null, json!(true)));

    assert_eq!(undo(&mut session), json!({ "command": "mail.move", "id": 1, "undone": true }));
    let back = held(&session, 1);
    assert_eq!(back.folder(), FolderId::new(1));
    assert_eq!(back.moving(), Some(&in_inbox(&[])), "the server still has it where it was; its answer decides the next job");
    assert!(session.take_jobs().is_empty());
}

#[test]
fn a_second_move_takes_the_next_uid_of_ours() {
    let mut session = session();
    session.run("mail.move", &json!({ "id": 1, "folder": 2 })).unwrap();
    session.run("mail.move", &json!({ "id": 2, "folder": 2 })).unwrap();
    assert_eq!(held(&session, 2).summary().uid(), uid(u32::MAX - 1));
}

#[test]
fn moving_to_where_it_is_changes_nothing() {
    let mut session = session();
    assert_eq!(session.run("mail.move", &json!({ "id": 1, "folder": 1 })).unwrap()["changed"], json!(false));
    assert!(session.take_jobs().is_empty());
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn a_move_refuses_what_does_not_fit() {
    let mut session = session();
    for (params, code) in [
        (json!({ "id": 1, "folder": 4 }), "bad_params"),
        (json!({ "id": 1, "folder": 99 }), "not_found"),
        (json!({ "id": 1, "folder": 7 }), "bad_params"),
        (json!({ "id": 99, "folder": 2 }), "not_found"),
        (json!({ "id": 1 }), "bad_params"),
    ] {
        assert_eq!(session.run("mail.move", &params).unwrap_err().code(), code, "{params}");
    }
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
}

#[test]
fn archive_moves_to_the_archive_folder_and_is_undone() {
    let mut session = session();
    assert_eq!(session.run("mail.archive", &json!({ "id": 1 })).unwrap(), json!({ "id": 1, "folder": 3, "was": 1, "changed": true }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(3));
    assert_eq!(kinds(&mut session), [moved_to(3)]);
    assert_eq!(undo(&mut session), json!({ "command": "mail.archive", "id": 1, "undone": true }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
}

#[test]
fn archive_needs_an_archive_folder() {
    let mut session = session();
    assert_eq!(session.run("mail.archive", &json!({ "id": 3 })).unwrap_err().code(), "not_found", "account 2 has no archive");
    session.run("mail.archive", &json!({ "id": 1 })).unwrap();
    assert_eq!(session.run("mail.archive", &json!({ "id": 1 })).unwrap()["changed"], json!(false), "archived already");
}

#[test]
fn delete_moves_to_trash_and_is_undone() {
    let mut session = session();
    assert_eq!(
        session.run("mail.delete", &json!({ "id": 1 })).unwrap(),
        json!({ "id": 1, "permanent": false, "folder": 6, "was": 1, "changed": true })
    );
    assert_eq!(held(&session, 1).folder(), FolderId::new(6));
    assert_eq!(kinds(&mut session), [moved_to(6)]);
    assert_eq!(undo(&mut session), json!({ "command": "mail.delete", "id": 1, "undone": true }));
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
}

#[test]
fn delete_in_trash_is_for_good_and_cannot_be_undone() {
    let mut session = session();
    let trashed = inboxcraft_crate_message(9, 6, 40);
    session.run("mail.flag", &json!({ "id": 2, "flagged": true })).unwrap();
    put(&mut session, trashed);
    session.take_jobs();
    assert_eq!(session.run("mail.delete", &json!({ "id": 9 })).unwrap(), json!({ "id": 9, "permanent": true }));
    assert_eq!(session.store().message(MessageId::new(9)).unwrap(), None);
    let expunge = JobKind::Expunge { folder: FolderId::new(6), path: path("Trash"), uid_validity: validity(), uids: uids(&[40]) };
    assert_eq!(kinds(&mut session), std::slice::from_ref(&expunge));
    assert!(queued(&session).contains(&expunge));
    assert_eq!(undo(&mut session)["command"], json!("mail.flag"), "the delete for good is not in the history");
}

#[test]
fn a_message_on_its_way_to_trash_is_not_deleted_for_good_yet() {
    let mut session = session();
    session.run("mail.delete", &json!({ "id": 1 })).unwrap();
    assert_eq!(session.run("mail.delete", &json!({ "id": 1 })).unwrap_err().code(), "still_moving");
    assert_eq!(held(&session, 1).folder(), FolderId::new(6));
}

#[test]
fn delete_needs_a_trash_folder() {
    let mut session = session();
    assert_eq!(session.run("mail.delete", &json!({ "id": 3 })).unwrap_err().code(), "not_found");
    assert_eq!(session.run("mail.delete", &json!({ "id": 99 })).unwrap_err().code(), "not_found");
}

#[test]
fn undo_walks_back_through_more_than_one_act() {
    let mut session = session();
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.run("mail.archive", &json!({ "id": 1 })).unwrap();
    session.run("mail.seen", &json!({ "id": 2, "seen": true })).unwrap();
    assert_eq!(undo(&mut session)["command"], json!("mail.seen"));
    assert!(!held(&session, 2).summary().flags().is_seen());
    assert_eq!(undo(&mut session)["command"], json!("mail.archive"));
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
    assert_eq!(undo(&mut session)["command"], json!("mail.flag"));
    assert!(!held(&session, 1).summary().flags().is_flagged());
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn an_agent_undoes_only_what_an_agent_may_do() {
    let mut session = session();
    session.run_as(Caller::Agent, "mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.run("mail.archive", &json!({ "id": 2 })).unwrap();
    let refused = session.run_as(Caller::Agent, "history.undo", &json!({})).unwrap_err();
    assert_eq!(refused.code(), "not_allowed");
    assert_eq!(refused.to_string(), "the command mail.archive is owner: an agent may not run it");
    assert_eq!(held(&session, 2).folder(), FolderId::new(3), "nothing was undone");
    assert_eq!(undo(&mut session)["command"], json!("mail.archive"));
    assert_eq!(session.run_as(Caller::Agent, "history.undo", &json!({})).unwrap()["command"], json!("mail.flag"));
}

#[test]
fn the_history_is_capped() {
    let mut session = session();
    for round in 0..crate::undo::MAX_HISTORY + 5 {
        session.run("mail.flag", &json!({ "id": 1, "flagged": round % 2 == 0 })).unwrap();
    }
    for _ in 0..crate::undo::MAX_HISTORY {
        undo(&mut session);
    }
    assert_eq!(session.run("history.undo", &json!({})).unwrap_err().code(), "nothing_to_undo");
}

#[test]
fn an_act_whose_message_is_gone_is_passed_over() {
    let mut session = session();
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.run("mail.archive", &json!({ "id": 2 })).unwrap();
    session.run("mail.seen", &json!({ "id": 2, "seen": true })).unwrap();
    put_change(&mut session, Change::RemoveMessage(MessageId::new(2)));
    assert_eq!(undo(&mut session), json!({ "command": "mail.seen", "id": 2, "undone": false }));
    assert_eq!(undo(&mut session), json!({ "command": "mail.archive", "id": 2, "undone": false }));
    assert_eq!(undo(&mut session)["undone"], json!(true));
}

#[test]
fn a_move_back_to_a_folder_that_is_gone_is_passed_over() {
    let mut session = session();
    session.run("mail.move", &json!({ "id": 5, "folder": 6 })).unwrap();
    put_change(&mut session, Change::RemoveFolder(FolderId::new(2)));
    assert_eq!(undo(&mut session), json!({ "command": "mail.move", "id": 5, "undone": false }));
    assert_eq!(held(&session, 5).folder(), FolderId::new(6));
}

#[test]
fn hostile_params_are_refused() {
    let mut session = session();
    for (id, params) in [
        ("mail.flag", json!({ "id": 1 })),
        ("mail.flag", json!({ "id": 1, "flagged": "yes" })),
        ("mail.flag", json!({ "id": 99, "flagged": true })),
        ("mail.seen", json!({ "id": 99, "seen": true })),
        ("mail.seen", json!({ "id": 1, "seen": true, "extra": 1 })),
        ("mail.archive", json!({ "id": 99 })),
        ("history.undo", json!({ "id": 1 })),
    ] {
        assert!(session.run(id, &params).is_err(), "{id} {params}");
    }
}

#[test]
fn the_sample_mailbox_acts_without_a_server() {
    let mut session = sample_session();
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.run("mail.archive", &json!({ "id": 1 })).unwrap();
    let message = held(&session, 1);
    assert_eq!(message.folder(), FolderId::new(3));
    assert_eq!(message.moving(), None, "nothing waits for a server");
    assert!(session.take_jobs().is_empty());
    assert!(session.store().queued_jobs().unwrap().is_empty());
    undo(&mut session);
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
}

#[test]
fn a_folder_not_synced_yet_gets_no_job() {
    let mut session = session_with(mailbox(), Registry::builtin().unwrap());
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    assert!(session.take_jobs().is_empty(), "no UIDVALIDITY to name the message by");
    assert!(held(&session, 1).summary().flags().is_flagged());
}

/// A message `id` in `folder` of account 1 with UID `uid`.
pub(crate) fn inboxcraft_crate_message(id: u64, folder: u64, uid: u32) -> Message {
    crate::test_support::message(id, 1, folder, uid, "Old news", Some(crate::test_support::NOW - 86_400))
}

/// Puts `message` into the session's store, as a sync would.
pub(crate) fn put(session: &mut Session, message: Message) {
    put_change(session, Change::PutMessage(message));
}

/// Makes `change` in the session's store through a test command, as an event would.
pub(crate) fn put_change(session: &mut Session, change: Change) {
    crate::test_support::change(session, change);
}

#[test]
fn every_act_is_kept_for_a_restart_with_the_account_it_belongs_to() {
    let mut session = session();
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    let jobs = session.take_jobs();
    assert_eq!(jobs.iter().map(|job| job.account).collect::<Vec<_>>(), [AccountId::new(1)]);
    assert_eq!(session.store().queued_jobs().unwrap(), jobs);
}
