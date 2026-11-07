// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The answers to the write jobs and their failures: the store ends as the server has it.

use inboxcraft_model::{
    AccountId, Copied, Envelope, ErrorKind, Event, EventKind, Flag, FlagChange, FolderId, FolderListing, FolderState, Job, JobId, JobKind, MessageId,
    Moving, Summary, UidValidity,
};
use serde_json::json;

use crate::Applied;
use crate::cmd::act_tests::{flags, held, inboxcraft_crate_message, path, put, session, uid, uids, validity};
use crate::session::Session;

fn account() -> AccountId {
    AccountId::new(1)
}

/// Runs `command` and hands out its one job.
fn act(session: &mut Session, command: &str, params: serde_json::Value) -> Job {
    session.run(command, &params).unwrap();
    let mut jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    jobs.remove(0)
}

fn copied(pairs: &[(u32, u32)]) -> Copied {
    Copied { uid_validity: validity(), uids: pairs.iter().map(|(from, to)| (uid(*from), uid(*to))).collect() }
}

/// The answer to `job`, a move of UID `from` out of `folder` to `to`.
fn moved(job: &Job, folder: u64, from: u32, to: u64, copied: Option<Copied>) -> Event {
    let kind = EventKind::Moved { folder: FolderId::new(folder), uid_validity: validity(), uids: uids(&[from]), to: FolderId::new(to), copied };
    Event::new(account(), Some(job.id), kind)
}

fn failed(job: &Job, folder: u64) -> Event {
    Event::new(account(), Some(job.id), EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(FolderId::new(folder)) })
}

fn kinds(session: &mut Session) -> Vec<JobKind> {
    session.take_jobs().into_iter().map(|job| job.kind).collect()
}

fn queued(session: &Session) -> Vec<JobKind> {
    session.store().queued_jobs().unwrap().into_iter().map(|job| job.kind).collect()
}

fn resync(folder: u64, name: &str, full: bool) -> JobKind {
    JobKind::SyncFolder { folder: FolderId::new(folder), path: path(name), known: (!full).then(|| FolderState::new(validity())) }
}

#[test]
fn the_answer_gives_a_moved_message_its_server_uid() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    assert_eq!(session.apply(&moved(&job, 1, 10, 3, Some(copied(&[(10, 55)])))).unwrap(), Applied::Changed);
    let message = held(&session, 1);
    assert_eq!((message.folder(), message.summary().uid(), message.moving()), (FolderId::new(3), uid(55), None));
    assert!(queued(&session).is_empty(), "the move left the queue");
    assert!(session.take_jobs().is_empty());
    let listed = session.run("mail.list", &json!({ "folder": 3 })).unwrap();
    assert_eq!(listed["messages"][0]["uid"], json!(55));
}

#[test]
fn an_undo_before_the_answer_moves_it_back_once_the_answer_comes() {
    let mut session = session();
    let first = act(&mut session, "mail.archive", json!({ "id": 1 }));
    session.run("history.undo", &json!({})).unwrap();
    assert!(session.take_jobs().is_empty(), "no job until the server names its place");

    session.apply(&moved(&first, 1, 10, 3, Some(copied(&[(10, 55)])))).unwrap();
    let message = held(&session, 1);
    assert_eq!(message.folder(), FolderId::new(1), "the owner sees it where the undo put it");
    assert_eq!(message.moving(), Some(&Moving { folder: FolderId::new(3), uid_validity: validity(), uid: uid(55), flags: flags(&[]) }));
    let back = JobKind::Move {
        folder: FolderId::new(3),
        path: path("Archive"),
        uid_validity: validity(),
        uids: uids(&[55]),
        to: FolderId::new(1),
        to_path: path("INBOX"),
    };
    let jobs = session.take_jobs();
    assert_eq!(jobs.iter().map(|job| job.kind.clone()).collect::<Vec<_>>(), std::slice::from_ref(&back));
    assert_eq!(queued(&session), [back]);

    session.apply(&moved(&jobs[0], 3, 55, 1, Some(copied(&[(55, 12)])))).unwrap();
    let message = held(&session, 1);
    assert_eq!((message.folder(), message.summary().uid(), message.moving()), (FolderId::new(1), uid(12), None));
    assert!(queued(&session).is_empty());
}

#[test]
fn flags_set_on_the_way_follow_once_the_answer_comes() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.run("mail.seen", &json!({ "id": 1, "seen": true })).unwrap();
    session.run("mail.seen", &json!({ "id": 1, "seen": false })).unwrap();
    assert!(session.take_jobs().is_empty());
    session.apply(&moved(&job, 1, 10, 3, Some(copied(&[(10, 55)])))).unwrap();
    let store_flags = JobKind::StoreFlags {
        folder: FolderId::new(3),
        path: path("Archive"),
        uid_validity: validity(),
        uids: uids(&[55]),
        add: flags(&[Flag::Flagged]),
        remove: flags(&[]),
    };
    assert_eq!(kinds(&mut session), [store_flags]);
    assert!(held(&session, 1).summary().flags().is_flagged());
}

#[test]
fn a_server_without_new_uids_brings_the_message_with_the_next_sync() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    session.apply(&moved(&job, 1, 10, 3, None)).unwrap();
    assert_eq!(session.store().message(MessageId::new(1)).unwrap(), None);
    assert_eq!(kinds(&mut session), [resync(3, "Archive", false)]);
}

#[test]
fn a_message_the_answer_does_not_name_was_gone_already() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    session.apply(&moved(&job, 1, 10, 3, Some(copied(&[(11, 56)])))).unwrap();
    assert_eq!(session.store().message(MessageId::new(1)).unwrap(), None);
    assert_eq!(kinds(&mut session), [resync(3, "Archive", false)]);
}

#[test]
fn new_uids_of_another_uidvalidity_are_not_taken() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    let other = Copied { uid_validity: UidValidity::new(8).unwrap(), uids: vec![(uid(10), uid(55))] };
    session.apply(&moved(&job, 1, 10, 3, Some(other))).unwrap();
    assert_eq!(session.store().message(MessageId::new(1)).unwrap(), None);
    assert_eq!(kinds(&mut session), [resync(3, "Archive", false)]);
}

#[test]
fn a_copy_the_sync_brought_first_gives_way_to_the_moved_message() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    put(&mut session, inboxcraft_crate_message(9, 3, 55));
    session.apply(&moved(&job, 1, 10, 3, Some(copied(&[(10, 55)])))).unwrap();
    assert_eq!(session.store().message(MessageId::new(9)).unwrap(), None);
    assert_eq!(held(&session, 1).summary().uid(), uid(55));
}

#[test]
fn a_move_to_a_folder_gone_meanwhile_drops_the_message() {
    let mut session = session();
    let job = act(&mut session, "mail.move", json!({ "id": 1, "folder": 6 }));
    session.run("history.undo", &json!({})).unwrap();
    crate::cmd::act_tests::put_change(&mut session, inboxcraft_store::Change::RemoveFolder(FolderId::new(6)));
    session.apply(&moved(&job, 1, 10, 6, Some(copied(&[(10, 55)])))).unwrap();
    assert_eq!(session.store().message(MessageId::new(1)).unwrap(), None);
    assert!(session.take_jobs().is_empty(), "no folder to sync");
}

#[test]
fn an_answer_of_an_older_uidvalidity_is_dropped_but_ends_the_job() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    let stale = EventKind::Moved {
        folder: FolderId::new(1),
        uid_validity: UidValidity::new(8).unwrap(),
        uids: uids(&[10]),
        to: FolderId::new(3),
        copied: None,
    };
    assert_eq!(session.apply(&Event::new(account(), Some(job.id), stale)).unwrap(), Applied::Dropped);
    assert!(held(&session, 1).moving().is_some());
    assert!(queued(&session).is_empty());
}

#[test]
fn a_failed_move_puts_the_message_back_where_the_server_has_it() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    assert_eq!(session.apply(&failed(&job, 1)).unwrap(), Applied::Changed);
    let message = held(&session, 1);
    assert_eq!((message.folder(), message.summary().uid(), message.moving()), (FolderId::new(1), uid(10), None));
    assert_eq!(kinds(&mut session), [resync(1, "INBOX", false), resync(3, "Archive", false)]);
    assert!(queued(&session).is_empty());
}

#[test]
fn a_failed_move_keeps_the_flags_set_on_the_way() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.apply(&failed(&job, 1)).unwrap();
    let store_flags = JobKind::StoreFlags {
        folder: FolderId::new(1),
        path: path("INBOX"),
        uid_validity: validity(),
        uids: uids(&[10]),
        add: flags(&[Flag::Flagged]),
        remove: flags(&[]),
    };
    assert_eq!(kinds(&mut session)[0], store_flags);
    assert_eq!(held(&session, 1).folder(), FolderId::new(1));
}

#[test]
fn a_failed_move_out_of_a_folder_that_is_gone_drops_the_message() {
    let mut session = session();
    let job = act(&mut session, "mail.archive", json!({ "id": 1 }));
    let without_inbox = ["Sent", "Archive", "Trash"]
        .iter()
        .map(|name| FolderListing::new(path(name), if *name == "Trash" { vec![inboxcraft_model::SpecialUse::Trash] } else { vec![] }, true))
        .collect();
    session.apply(&Event::new(account(), None, EventKind::Folders { listings: without_inbox })).unwrap();
    session.take_jobs();
    session.apply(&Event::new(account(), Some(job.id), EventKind::Failed { kind: ErrorKind::FolderGone, folder: None })).unwrap();
    assert_eq!(session.store().message(MessageId::new(1)).unwrap(), None);
    assert_eq!(kinds(&mut session), [resync(3, "Archive", false)]);
}

#[test]
fn a_reset_folder_voids_the_moves_out_of_it() {
    let mut session = session();
    act(&mut session, "mail.archive", json!({ "id": 1 }));
    let reset = EventKind::UidValidityReset { folder: FolderId::new(1), old: Some(validity()), new: UidValidity::new(8).unwrap() };
    session.apply(&Event::new(account(), None, reset)).unwrap();
    assert_eq!(session.store().message(MessageId::new(1)).unwrap(), None, "its place on the server is void");
    assert_eq!(session.store().message(MessageId::new(2)).unwrap(), None);
}

#[test]
fn a_failed_flag_is_taken_back() {
    let mut session = session();
    let job = act(&mut session, "mail.flag", json!({ "id": 1, "flagged": true }));
    session.apply(&failed(&job, 1)).unwrap();
    assert!(!held(&session, 1).summary().flags().is_flagged());
    assert!(queued(&session).is_empty());
}

#[test]
fn a_failed_unflag_is_taken_back_and_a_flag_changed_since_is_left() {
    let mut session = session();
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.take_jobs();
    let job = act(&mut session, "mail.flag", json!({ "id": 1, "flagged": false }));
    session.apply(&failed(&job, 1)).unwrap();
    assert!(held(&session, 1).summary().flags().is_flagged());

    let job = act(&mut session, "mail.seen", json!({ "id": 2, "seen": true }));
    let changed = FlagChange { uid: uid(11), flags: flags(&[]), modseq: None };
    let changes = EventKind::MessagesChanged { folder: FolderId::new(1), uid_validity: validity(), changes: vec![changed] };
    session.apply(&Event::new(account(), None, changes)).unwrap();
    session.apply(&failed(&job, 1)).unwrap();
    assert!(held(&session, 2).summary().flags().is_empty(), "nothing left to take back");
}

#[test]
fn a_failed_delete_for_good_syncs_the_whole_folder_again() {
    let mut session = session();
    put(&mut session, inboxcraft_crate_message(9, 6, 40));
    let job = act(&mut session, "mail.delete", json!({ "id": 9 }));
    session.apply(&failed(&job, 6)).unwrap();
    assert_eq!(kinds(&mut session), [resync(6, "Trash", true)]);
}

#[test]
fn a_failure_of_a_job_not_kept_takes_nothing_back() {
    let mut session = session();
    session.run("mail.flag", &json!({ "id": 1, "flagged": true })).unwrap();
    session.take_jobs();
    let stray = Event::new(account(), Some(JobId::new(999)), EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(FolderId::new(1)) });
    session.apply(&stray).unwrap();
    assert!(held(&session, 1).summary().flags().is_flagged());
    assert!(session.take_jobs().is_empty());
}

#[test]
fn the_last_events_of_the_other_writes_end_their_jobs() {
    let mut session = session();
    let job = act(&mut session, "mail.flag", json!({ "id": 1, "flagged": true }));
    let stored = EventKind::FlagsStored { folder: FolderId::new(1), uid_validity: validity(), uids: uids(&[10]) };
    assert_eq!(session.apply(&Event::new(account(), Some(job.id), stored)).unwrap(), Applied::Changed);
    assert!(queued(&session).is_empty());
    assert!(held(&session, 1).summary().flags().is_flagged());

    put(&mut session, inboxcraft_crate_message(9, 6, 40));
    let job = act(&mut session, "mail.delete", json!({ "id": 9 }));
    let gone = EventKind::Expunged { folder: FolderId::new(6), uid_validity: validity(), uids: uids(&[40]) };
    assert_eq!(session.apply(&Event::new(account(), Some(job.id), gone)).unwrap(), Applied::Changed);
    assert!(queued(&session).is_empty());

    let appended = EventKind::Appended { folder: FolderId::new(2), uid: Some((validity(), uid(4))) };
    assert_eq!(session.apply(&Event::new(account(), None, appended)).unwrap(), Applied::Changed);
    let created = EventKind::FolderCreated { path: path("InboxCraft/Screener") };
    assert_eq!(session.apply(&Event::new(account(), None, created)).unwrap(), Applied::Changed);
    assert_eq!(kinds(&mut session), [JobKind::SyncFolderList], "the new folder comes with the folder list");
}

#[test]
fn a_server_uid_that_takes_one_of_ours_moves_ours_aside() {
    let mut session = session();
    act(&mut session, "mail.archive", json!({ "id": 1 }));
    assert_eq!(held(&session, 1).summary().uid(), uid(u32::MAX));
    let arrived = Summary::new(uid(u32::MAX), Envelope::new("A very high UID"));
    let added = EventKind::MessagesAdded { folder: FolderId::new(3), uid_validity: validity(), messages: vec![arrived] };
    session.apply(&Event::new(account(), None, added)).unwrap();
    let ours = held(&session, 1);
    assert_eq!((ours.folder(), ours.summary().uid()), (FolderId::new(3), uid(u32::MAX - 1)));
    assert!(ours.moving().is_some());
    let theirs = session.run("mail.list", &json!({ "folder": 3 })).unwrap();
    assert_eq!(theirs["total"], json!(2));
}

#[test]
fn server_events_do_not_touch_a_uid_of_ours() {
    let mut session = session();
    act(&mut session, "mail.archive", json!({ "id": 1 }));
    let changed = FlagChange { uid: uid(u32::MAX), flags: flags(&[Flag::Deleted]), modseq: None };
    let events = [
        EventKind::MessagesChanged { folder: FolderId::new(3), uid_validity: validity(), changes: vec![changed] },
        EventKind::MessagesExpunged { folder: FolderId::new(3), uid_validity: validity(), uids: uids(&[u32::MAX]) },
        EventKind::BodyFetched { folder: FolderId::new(3), uid_validity: validity(), uid: uid(u32::MAX), raw: b"Subject: x\r\n\r\ny".to_vec() },
    ];
    for kind in events {
        session.apply(&Event::new(account(), None, kind)).unwrap();
    }
    let ours = held(&session, 1);
    assert!(ours.summary().flags().is_empty());
    assert_eq!(session.store().body(MessageId::new(1)).unwrap(), None);
}

#[test]
fn a_message_on_its_way_is_read_without_a_fetch() {
    let mut session = session();
    act(&mut session, "mail.archive", json!({ "id": 1 }));
    let answer = session.run("mail.read", &json!({ "id": 1 })).unwrap();
    assert_eq!(answer["fetching"], json!(false));
    assert_eq!(answer["message"]["moving"], json!(true));
    assert!(session.take_jobs().is_empty());
}
