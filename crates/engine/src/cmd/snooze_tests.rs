// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Snooze: `mail.snooze` and `mail.unsnooze`, the copy in `InboxCraft/Snoozed` with the time in InboxCraft's own
//! header field, the return at the due time on a fake clock, and a second session that reads the time from the field.

use std::time::Duration;

use inboxcraft_model::{
    AccountId, Due, ErrorKind, Event, EventKind, Flag, FolderId, FolderState, JobKind, MessageId, MessageState, Moving, Summary, Uid,
};
use inboxcraft_store::{Body, Change, Memory};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use crate::cmd::act_tests::{flags, held, path, store, uid, uids, validity};
use crate::notify::Notice;
use crate::test_support::{NOW, address, at, change, folder};
use crate::{AgentClass, Caller, Registry, Session};

/// 2026-11-05T18:00:00Z, nine hours after [`NOW`].
const EVENING: i64 = NOW + 9 * 3600;

/// The raw message 1 is kept with.
const RAW: &[u8] = b"From: Anna Example <anna@example.org>\r\nSubject: Budget for 2026\r\n\r\nThe numbers.\r\n";

/// The field InboxCraft writes for a snooze until [`EVENING`].
const FIELD: &[u8] = b"InboxCraft-Snoozed-Until: Thu, 5 Nov 2026 18:00:00 +0000\r\n";

/// The snoozed folder made for account 1: past every folder of the test store.
const SNOOZED: u64 = 8;

/// A session on a fake clock over the act tests' store, with message 1 read and its body kept.
fn session_at(clock: &FakeClock) -> Session {
    session_over(store(), clock)
}

fn session_over(store: Memory, clock: &FakeClock) -> Session {
    let mut session = Session::with_registry(Box::new(store), Box::new(clock.clone()), Registry::builtin().unwrap()).unwrap();
    change(&mut session, Change::SetFlags(MessageId::new(1), flags(&[Flag::Seen])));
    change(&mut session, Change::PutBody(MessageId::new(1), Body::new(RAW.to_vec(), "The numbers.").unwrap()));
    session
}

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (session_at(&clock), clock)
}

fn snooze(session: &mut Session, id: u64, until: &str) -> Value {
    session.run("mail.snooze", &json!({ "id": id, "until": until })).unwrap()
}

fn state(session: &Session, id: u64) -> MessageState {
    session.store().message_state(MessageId::new(id)).unwrap()
}

fn schedule(session: &Session) -> Vec<(i64, Due)> {
    session.store().schedule().unwrap().into_iter().map(|entry| (entry.due.unix_seconds(), entry.what)).collect()
}

fn kinds(jobs: &[inboxcraft_model::Job]) -> Vec<JobKind> {
    jobs.iter().map(|job| job.kind.clone()).collect()
}

/// The copy of message 1 with the snooze field on top.
fn copy() -> Vec<u8> {
    [FIELD, RAW].concat()
}

/// Snoozes message 1 until [`EVENING`] and answers its copy with `uid` (`APPENDUID`) or without; the jobs that follow.
fn snoozed_and_copied(session: &mut Session, uid: Option<u32>) -> Vec<inboxcraft_model::Job> {
    snooze(session, 1, "2026-11-05T18:00:00Z");
    let jobs = session.take_jobs();
    let append = jobs.iter().find(|job| matches!(job.kind, JobKind::Append { .. })).unwrap().id;
    let answer = EventKind::Appended { folder: FolderId::new(SNOOZED), uid: uid.map(|n| (validity(), Uid::new(n).unwrap())) };
    session.apply(&Event::new(AccountId::new(1), Some(append), answer)).unwrap();
    session.take_jobs()
}

/// Answers every job of `jobs` the way a server that names new UIDs would, as far as the tests need it.
fn answer(session: &mut Session, jobs: Vec<inboxcraft_model::Job>) {
    for job in jobs {
        let kind = match job.kind {
            JobKind::Move { folder, uid_validity, uids, to, .. } => {
                let pairs = uids.ranges().iter().flat_map(|(low, high)| (low.get()..=high.get()).map(|n| (uid(n), uid(n)))).collect();
                let copied = Some(inboxcraft_model::Copied { uid_validity: validity(), uids: pairs });
                EventKind::Moved { folder, uid_validity, uids, to, copied }
            }
            JobKind::Expunge { folder, uid_validity, uids, .. } => EventKind::Expunged { folder, uid_validity, uids },
            JobKind::StoreFlags { folder, uid_validity, uids, .. } => EventKind::FlagsStored { folder, uid_validity, uids },
            _ => continue,
        };
        session.apply(&Event::new(job.account, Some(job.id), kind)).unwrap();
    }
}

#[test]
fn snooze_is_safe_for_an_agent() {
    let registry = Registry::builtin().unwrap();
    for id in ["mail.snooze", "mail.unsnooze"] {
        assert_eq!(registry.lookup(id).map(|(_, class)| class), Some(AgentClass::Safe), "{id}");
    }
    let (mut session, _) = session();
    let answer = session.run_as(Caller::Agent, "mail.snooze", &json!({ "id": 2, "until": "2026-11-05T18:00:00Z" })).unwrap();
    assert_eq!(answer["changed"], json!(true));
}

#[test]
fn snooze_writes_the_time_into_a_copy_in_the_snoozed_folder() {
    let (mut session, _) = session();
    let answer = snooze(&mut session, 1, "2026-11-05T18:00:00Z");
    assert_eq!(answer, json!({ "id": 1, "until": "2026-11-05T18:00:00Z", "folder": SNOOZED, "was": 1, "changed": true, "written": true }));
    let snoozed = path("InboxCraft/Snoozed");
    let append = JobKind::Append {
        folder: FolderId::new(SNOOZED),
        path: snoozed.clone(),
        flags: flags(&[Flag::Seen]),
        raw: copy(),
        draft: None,
        message: Some(MessageId::new(1)),
    };
    assert_eq!(kinds(&session.take_jobs()), [JobKind::CreateFolder { path: snoozed }, append], "the old copy stays until the new one is there");
    let message = held(&session, 1);
    assert_eq!(message.folder(), FolderId::new(SNOOZED));
    let place = Moving { folder: FolderId::new(1), uid_validity: validity(), uid: uid(10), flags: flags(&[Flag::Seen]) };
    assert_eq!(message.moving(), Some(&place), "the server keeps it in the inbox until the copy is there");
    assert_eq!(state(&session, 1), MessageState { snoozed_until: Some(at(EVENING)), ..MessageState::default() });
    assert_eq!(schedule(&session), [(EVENING, Due::Unsnooze(MessageId::new(1)))]);
    let listed = session.run("mail.list", &json!({ "role": "snoozed" })).unwrap();
    assert_eq!(listed["messages"][0]["snoozed_until"], json!("2026-11-05T18:00:00Z"));
    assert_eq!(listed["messages"][0]["returned"], Value::Null);
}

#[test]
fn the_answer_to_the_copy_takes_the_old_one_away() {
    let (mut session, _) = session();
    let jobs = snoozed_and_copied(&mut session, Some(4));
    let expunge = JobKind::Expunge { folder: FolderId::new(1), path: path("INBOX"), uid_validity: validity(), uids: uids(&[10]) };
    assert_eq!(kinds(&jobs), [expunge]);
    let message = held(&session, 1);
    assert_eq!((message.folder(), message.summary().uid(), message.moving()), (FolderId::new(SNOOZED), uid(4), None));
    assert_eq!(state(&session, 1).snoozed_until, Some(at(EVENING)), "the message keeps its id and its state");
    answer(&mut session, jobs);
    assert_eq!(held(&session, 1).folder(), FolderId::new(SNOOZED), "the old copy's expunge is no news");
}

#[test]
fn without_a_uid_for_the_copy_the_snoozed_folder_brings_it_with_its_time() {
    let (mut session, _) = session();
    let jobs = snoozed_and_copied(&mut session, None);
    assert!(
        matches!(kinds(&jobs).as_slice(), [JobKind::Expunge { .. }, JobKind::SyncFolder { folder, .. }] if *folder == FolderId::new(SNOOZED)),
        "{jobs:?}"
    );
    assert!(session.store().message(MessageId::new(1)).unwrap().is_none());
    let copy = Summary::new(uid(4), inboxcraft_model::Envelope::new("Budget for 2026").with_snoozed_until(Some(at(EVENING))));
    let added = EventKind::MessagesAdded { folder: FolderId::new(SNOOZED), uid_validity: validity(), messages: vec![copy] };
    session.apply(&Event::new(AccountId::new(1), None, added)).unwrap();
    let back = session.store().messages(FolderId::new(SNOOZED)).unwrap();
    let id = back.first().unwrap().message().id();
    assert_eq!(session.store().message_state(id).unwrap().snoozed_until, Some(at(EVENING)));
    assert_eq!(schedule(&session), [(EVENING, Due::Unsnooze(id))]);
}

#[test]
fn a_failed_copy_leaves_the_message_where_the_server_keeps_it() {
    let (mut session, _) = session();
    snooze(&mut session, 1, "2026-11-05T18:00:00Z");
    let jobs = session.take_jobs();
    let append = jobs.iter().find(|job| matches!(job.kind, JobKind::Append { .. })).unwrap().id;
    let failed = EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(FolderId::new(SNOOZED)) };
    session.apply(&Event::new(AccountId::new(1), Some(append), failed)).unwrap();
    let message = held(&session, 1);
    assert_eq!((message.folder(), message.summary().uid(), message.moving()), (FolderId::new(1), uid(10), None));
    assert!(!kinds(&session.take_jobs()).iter().any(|kind| matches!(kind, JobKind::Expunge { .. })), "nothing is deleted");
}

#[test]
fn a_message_without_a_kept_body_is_moved_without_the_field() {
    let (mut session, _) = session();
    let answer = snooze(&mut session, 2, "2026-11-05T18:00:00Z");
    assert_eq!(answer["written"], json!(false));
    let jobs = kinds(&session.take_jobs());
    assert!(matches!(jobs.as_slice(), [JobKind::CreateFolder { .. }, JobKind::Move { to, .. }] if *to == FolderId::new(SNOOZED)), "{jobs:?}");
    assert_eq!(held(&session, 2).folder(), FolderId::new(SNOOZED));
    assert_eq!(schedule(&session), [(EVENING, Due::Unsnooze(MessageId::new(2)))]);
}

#[test]
fn at_the_due_time_it_comes_back_unread_at_the_top() {
    let (mut session, clock) = session();
    let jobs = snoozed_and_copied(&mut session, Some(4));
    answer(&mut session, jobs);
    clock.advance(Duration::from_secs(9 * 3600 - 1));
    assert_eq!(session.tick().unwrap(), []);
    assert_eq!(held(&session, 1).folder(), FolderId::new(SNOOZED));
    clock.advance(Duration::from_secs(1));
    let fired = session.tick().unwrap();
    assert_eq!(fired.len(), 1);
    assert!(!fired.first().unwrap().missed);
    let message = held(&session, 1);
    assert_eq!(message.folder(), FolderId::new(1), "back in the inbox");
    assert!(!message.summary().flags().contains(&Flag::Seen), "unread");
    assert_eq!(state(&session, 1), MessageState { returned: Some(at(EVENING)), ..MessageState::default() });
    assert_eq!(schedule(&session), []);
    let jobs = kinds(&session.take_jobs());
    let unread = JobKind::StoreFlags {
        folder: FolderId::new(SNOOZED),
        path: path("InboxCraft/Snoozed"),
        uid_validity: validity(),
        uids: uids(&[4]),
        add: flags(&[]),
        remove: flags(&[Flag::Seen]),
    };
    assert_eq!(jobs.first(), Some(&unread), "{jobs:?}");
    assert!(matches!(jobs.get(1), Some(JobKind::Move { to, .. }) if *to == FolderId::new(1)), "{jobs:?}");
    let inbox = session.run("mail.list", &json!({ "account": 1 })).unwrap();
    assert_eq!(inbox["messages"][0]["id"], json!(1), "at the top, though message 2 is newer");
    assert_eq!(inbox["messages"][0]["returned"], json!("2026-11-05T18:00:00Z"));
    assert!(matches!(session.take_notices().as_slice(), [Notice::Returned { missed: false, .. }]));
}

#[test]
fn a_snooze_due_while_inboxcraft_was_off_returns_at_the_start_as_missed() {
    let (mut session, clock) = session();
    snooze(&mut session, 2, "2026-11-05T18:00:00Z");
    session.take_jobs();
    clock.set(at(EVENING + 3600));
    session.start();
    session.take_jobs();
    let fired = session.tick().unwrap();
    assert!(fired.first().unwrap().missed);
    assert_eq!(held(&session, 2).folder(), FolderId::new(1));
}

#[test]
fn a_message_moved_on_from_the_snoozed_folder_stays_where_the_owner_put_it() {
    let (mut session, clock) = session();
    snooze(&mut session, 2, "2026-11-05T18:00:00Z");
    session.run("mail.archive", &json!({ "id": 2 })).unwrap();
    session.take_jobs();
    clock.set(at(EVENING));
    session.tick().unwrap();
    assert_eq!(held(&session, 2).folder(), FolderId::new(3));
    assert!(state(&session, 2).is_empty());
}

/// The test store with `InboxCraft/Snoozed` (8) synced, as another InboxCraft left it.
fn second_store() -> Memory {
    let mut store = store();
    let batch = inboxcraft_store::Batch::new()
        .with(Change::PutFolder(folder(SNOOZED, 1, "InboxCraft/Snoozed", vec![])))
        .with(Change::SetFolderState(FolderId::new(SNOOZED), FolderState::new(validity())));
    inboxcraft_store::Storage::apply(&mut store, &batch).unwrap();
    store
}

#[test]
fn a_second_session_reads_the_due_time_from_the_header() {
    let clock = FakeClock::new(at(NOW));
    let mut session = session_over(second_store(), &clock);
    let envelope =
        inboxcraft_model::Envelope::new("Later, please").with_from(vec![address("Cecil", "cecil@example.org")]).with_snoozed_until(Some(at(EVENING)));
    let not_ours = inboxcraft_model::Envelope::new("Filed by hand").with_from(vec![address("Dora", "dora@example.org")]);
    let added = EventKind::MessagesAdded {
        folder: FolderId::new(SNOOZED),
        uid_validity: validity(),
        messages: vec![Summary::new(uid(1), envelope), Summary::new(uid(2), not_ours)],
    };
    session.apply(&Event::new(AccountId::new(1), None, added)).unwrap();
    let id = |n: u32| {
        session
            .store()
            .messages(FolderId::new(SNOOZED))
            .unwrap()
            .into_iter()
            .find(|entry| entry.message().summary().uid() == uid(n))
            .unwrap()
            .message()
            .id()
    };
    let (later, by_hand) = (id(1), id(2));
    assert_eq!(session.store().message_state(later).unwrap().snoozed_until, Some(at(EVENING)));
    assert!(session.store().message_state(by_hand).unwrap().is_empty(), "no field, no time");
    assert_eq!(schedule(&session), [(EVENING, Due::Unsnooze(later))]);
    clock.set(at(EVENING));
    session.tick().unwrap();
    assert_eq!(session.store().message(later).unwrap().unwrap().message().folder(), FolderId::new(1));
    let elsewhere = EventKind::MessagesAdded {
        folder: FolderId::new(1),
        uid_validity: validity(),
        messages: vec![Summary::new(uid(30), inboxcraft_model::Envelope::new("Old field").with_snoozed_until(Some(at(EVENING + 60))))],
    };
    session.apply(&Event::new(AccountId::new(1), None, elsewhere)).unwrap();
    assert_eq!(schedule(&session), [], "the field counts in the snoozed folder only");
}

#[test]
fn undo_takes_a_snooze_back() {
    let (mut session, _) = session();
    snooze(&mut session, 2, "2026-11-05T18:00:00Z");
    session.take_jobs();
    assert_eq!(session.run("history.undo", &json!({})).unwrap(), json!({ "command": "mail.snooze", "id": 2, "undone": true }));
    assert_eq!(held(&session, 2).folder(), FolderId::new(1));
    assert!(state(&session, 2).is_empty());
    assert_eq!(schedule(&session), []);
}

#[test]
fn snoozing_again_moves_the_time() {
    let (mut session, _) = session();
    let jobs = snoozed_and_copied(&mut session, Some(4));
    answer(&mut session, jobs);
    let again = snooze(&mut session, 1, "2026-11-06T08:00:00Z");
    assert_eq!(again["changed"], json!(true));
    let later = NOW + 23 * 3600;
    assert_eq!(schedule(&session), [(later, Due::Unsnooze(MessageId::new(1)))]);
    let jobs = kinds(&session.take_jobs());
    let [JobKind::Append { folder, raw, .. }] = jobs.as_slice() else { panic!("{jobs:?}") };
    assert_eq!(*folder, FolderId::new(SNOOZED), "a new copy with the new time, in the same folder");
    assert!(raw.starts_with(b"InboxCraft-Snoozed-Until: Fri, 6 Nov 2026 08:00:00 +0000\r\n"), "{:?}", String::from_utf8_lossy(raw));
    session.run("history.undo", &json!({})).unwrap();
    assert_eq!(state(&session, 1).snoozed_until, Some(at(EVENING)));
    assert_eq!(schedule(&session), [(EVENING, Due::Unsnooze(MessageId::new(1)))], "the undo brings the first time back");
}

#[test]
fn unsnooze_brings_it_back_now() {
    let (mut session, clock) = session();
    snooze(&mut session, 2, "2026-11-05T18:00:00Z");
    session.take_jobs();
    clock.advance(Duration::from_secs(60));
    let answer = session.run("mail.unsnooze", &json!({ "id": 2 })).unwrap();
    assert_eq!(answer, json!({ "id": 2, "folder": 1, "changed": true }));
    assert_eq!(held(&session, 2).folder(), FolderId::new(1));
    assert_eq!(state(&session, 2), MessageState { returned: Some(at(NOW + 60)), ..MessageState::default() });
    assert_eq!(schedule(&session), []);
    assert_eq!(session.run("mail.unsnooze", &json!({ "id": 2 })).unwrap()["changed"], json!(false));
}

#[test]
fn a_preset_names_the_time() {
    let (mut session, _) = session();
    let answer = session.run("mail.snooze", &json!({ "id": 2, "preset": "this_evening" })).unwrap();
    assert_eq!(answer["until"], json!("2026-11-05T20:00:00Z"));
    let answer = session.run("mail.snooze", &json!({ "id": 2, "preset": "tomorrow_morning" })).unwrap();
    assert_eq!(answer["until"], json!("2026-11-06T08:00:00Z"));
}

#[test]
fn a_snooze_needs_one_time_ahead() {
    let (mut session, _) = session();
    for params in [
        json!({ "id": 2 }),
        json!({ "id": 2, "until": "2026-11-05T18:00:00Z", "preset": "this_evening" }),
        json!({ "id": 2, "until": "tonight" }),
        json!({ "id": 2, "until": "2026-11-05T09:00:00Z" }),
        json!({ "id": 2, "until": "2026-11-04" }),
        json!({ "id": 2, "preset": "next_year" }),
    ] {
        assert_eq!(session.run("mail.snooze", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert_eq!(session.run("mail.snooze", &json!({ "id": 99, "until": "2026-11-05T18:00:00Z" })).unwrap_err().code(), "not_found");
    assert_eq!(session.run("mail.unsnooze", &json!({ "id": 99 })).unwrap_err().code(), "not_found");
    assert!(session.take_jobs().is_empty());
    assert_eq!(schedule(&session), []);
}

#[test]
fn an_account_whose_folders_have_no_hierarchy_cannot_snooze() {
    let mut store = store();
    let flat = inboxcraft_model::Folder::new(
        FolderId::new(4),
        AccountId::new(2),
        inboxcraft_model::FolderListing::new(inboxcraft_model::FolderPath::new("INBOX", None).unwrap(), vec![], true),
    );
    inboxcraft_store::Storage::apply(&mut store, &inboxcraft_store::Batch::new().with(Change::PutFolder(flat))).unwrap();
    let clock = FakeClock::new(at(NOW));
    let mut session = session_over(store, &clock);
    assert_eq!(session.run("mail.snooze", &json!({ "id": 3, "until": "2026-11-05T18:00:00Z" })).unwrap_err().code(), "not_found");
}

#[test]
fn the_sample_mailbox_snoozes_without_a_server() {
    let mut session = crate::test_support::sample_session();
    session.run("mail.snooze", &json!({ "id": 1, "until": "2026-11-05T18:00:00Z" })).unwrap();
    assert!(session.take_jobs().is_empty());
    let listed = session.run("mail.list", &json!({ "role": "snoozed" })).unwrap();
    assert_eq!(listed["messages"][0]["id"], json!(1));
}

#[test]
fn a_message_whose_move_is_on_its_way_is_snoozed_without_a_copy() {
    let (mut session, _) = session();
    session.run("mail.archive", &json!({ "id": 1 })).unwrap();
    session.take_jobs();
    let answer = snooze(&mut session, 1, "2026-11-05T18:00:00Z");
    assert_eq!(answer["written"], json!(false), "the copy waits for nothing: the server follows once the move is answered");
    let message = held(&session, 1);
    assert_eq!(message.folder(), FolderId::new(SNOOZED));
    assert_eq!(message.moving().map(|place| place.uid), Some(uid(10)));
    assert!(kinds(&session.take_jobs()).iter().all(|kind| matches!(kind, JobKind::CreateFolder { .. })));
}

#[test]
fn undoing_something_else_keeps_the_snooze_entry() {
    let (mut session, _) = session();
    snooze(&mut session, 2, "2026-11-05T18:00:00Z");
    let before = session.store().schedule().unwrap();
    session.run("mail.park", &json!({ "id": 2, "parked": true })).unwrap();
    session.run("history.undo", &json!({})).unwrap();
    assert_eq!(session.store().schedule().unwrap(), before, "the same entry, not a new one");
    assert_eq!(state(&session, 2).snoozed_until, Some(at(EVENING)));
}

#[test]
fn a_failed_copy_whose_old_folder_is_gone_drops_the_message() {
    let (mut session, _) = session();
    snooze(&mut session, 1, "2026-11-05T18:00:00Z");
    let jobs = session.take_jobs();
    let append = jobs.iter().find(|job| matches!(job.kind, JobKind::Append { .. })).unwrap().id;
    let listing = |name: &str| inboxcraft_model::FolderListing::new(path(name), vec![], true);
    let listings = vec![listing("Sent"), listing("Archive"), listing("Trash"), listing("InboxCraft/Snoozed")];
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Folders { listings })).unwrap();
    assert_eq!(held(&session, 1).folder(), FolderId::new(SNOOZED), "the inbox went, the copy on its way stays");
    let failed = EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(FolderId::new(SNOOZED)) };
    session.apply(&Event::new(AccountId::new(1), Some(append), failed)).unwrap();
    assert!(session.store().message(MessageId::new(1)).unwrap().is_none(), "the server keeps it nowhere we know");
}

#[test]
fn an_account_that_lists_no_inbox_keeps_a_returned_message_where_it_is() {
    let mut store = Memory::new(inboxcraft_store::StoreConfig::default());
    let batch = inboxcraft_store::Batch::new()
        .with(Change::PutAccount(crate::test_support::account(1, "anna@example.org")))
        .with(Change::PutFolder(folder(SNOOZED, 1, "InboxCraft/Snoozed", vec![])))
        .with(Change::SetFolderState(FolderId::new(SNOOZED), FolderState::new(validity())));
    inboxcraft_store::Storage::apply(&mut store, &batch).unwrap();
    let clock = FakeClock::new(at(NOW));
    let mut session = Session::with_registry(Box::new(store), Box::new(clock.clone()), Registry::builtin().unwrap()).unwrap();
    let envelope = inboxcraft_model::Envelope::new("Later").with_snoozed_until(Some(at(EVENING)));
    let added = EventKind::MessagesAdded { folder: FolderId::new(SNOOZED), uid_validity: validity(), messages: vec![Summary::new(uid(1), envelope)] };
    session.apply(&Event::new(AccountId::new(1), None, added)).unwrap();
    let id = session.store().messages(FolderId::new(SNOOZED)).unwrap().first().unwrap().message().id();
    assert_eq!(session.run("mail.unsnooze", &json!({ "id": id.get() })).unwrap(), json!({ "id": id.get(), "folder": SNOOZED, "changed": true }));
}
