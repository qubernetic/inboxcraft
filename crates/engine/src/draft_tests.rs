// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A draft goes to the server's Drafts folder once it has been still for a while, and each new copy replaces the one
//! before it.

use std::time::Duration;

use inboxcraft_model::{AccountId, Draft, DraftCopy, DraftId, ErrorKind, Event, EventKind, Flag, FolderId, Job, JobId, JobKind, Uid, UidSet};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::FakeClock;
use serde_json::json;

use super::UPLOAD_AFTER;
use crate::cmd::compose_tests::{session_at, store, validity};
use crate::test_support::{NOW, at};
use crate::{Applied, Session};

fn uid(n: u32) -> Uid {
    Uid::new(n).unwrap()
}

fn copy(n: u32) -> DraftCopy {
    DraftCopy { folder: FolderId::new(6), uid_validity: validity(), uid: uid(n) }
}

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    let mut session = session_at(store(), &clock);
    session.start();
    session.take_jobs();
    (session, clock)
}

/// A draft of account 1 to Béla with a subject and text, saved now.
fn typed(session: &mut Session) -> DraftId {
    let id = session.run("compose.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    session
        .run("compose.update", &json!({ "id": id, "to": ["Béla <bela@example.org>", "not an address"], "subject": "Budget", "text": "Hello Béla" }))
        .unwrap();
    DraftId::new(id)
}

fn draft(session: &Session, id: DraftId) -> Draft {
    session.store().draft(id).unwrap().unwrap()
}

fn wait(clock: &FakeClock, seconds: u64) {
    clock.advance(Duration::from_secs(seconds));
}

fn appended(session: &mut Session, job: JobId, uid: Option<u32>) -> Applied {
    let kind = EventKind::Appended { folder: FolderId::new(6), uid: uid.map(|n| (validity(), Uid::new(n).unwrap())) };
    session.apply(&Event { account: AccountId::new(1), job: Some(job), kind }).unwrap()
}

/// The one Append job queued by a tick now, with its raw message as text.
fn upload(session: &mut Session) -> (Job, String) {
    session.tick().unwrap();
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let job = jobs.into_iter().next().unwrap();
    let JobKind::Append { raw, .. } = &job.kind else { panic!("{job:?}") };
    let text = String::from_utf8(raw.clone()).unwrap();
    (job, text)
}

#[test]
fn a_draft_goes_up_once_it_has_been_still_for_a_while() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    let saved = draft(&session, id).saved;
    assert_eq!(session.next_due().unwrap(), Some(saved.saturating_add(UPLOAD_AFTER)));
    wait(&clock, UPLOAD_AFTER.as_secs() - 1);
    assert!(session.tick().unwrap().is_empty());
    assert!(session.take_jobs().is_empty(), "still being written");
    wait(&clock, 1);
    let (job, raw) = upload(&mut session);
    let JobKind::Append { folder, path, flags, draft: of, .. } = &job.kind else { panic!("{job:?}") };
    assert_eq!((*folder, path.name(), *of), (FolderId::new(6), "Drafts", Some(id)));
    assert!(flags.contains(&Flag::Seen) && flags.contains(&Flag::Draft), "{flags:?}");
    assert!(raw.contains("Subject: Budget\r\n"), "{raw}");
    assert!(raw.contains("From: anna@example.org\r\n"), "{raw}");
    assert!(raw.contains("bela@example.org"), "{raw}");
    assert!(!raw.contains("not an address"), "an impossible address stays local: {raw}");
    assert!(raw.contains("Hello B"), "{raw}");
    assert_eq!(draft(&session, id).synced, Some(saved));
    assert_eq!(session.store().queued_jobs().unwrap().len(), 1, "kept until the server answers");
    assert_eq!(session.next_due().unwrap(), None);
    session.tick().unwrap();
    assert!(session.take_jobs().is_empty(), "the same version goes up once");
}

#[test]
fn each_new_copy_replaces_the_one_before() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (first, _) = upload(&mut session);
    assert_eq!(appended(&mut session, first.id, Some(30)), Applied::Changed);
    assert_eq!(draft(&session, id).uploaded, Some(copy(30)));
    assert!(session.take_jobs().is_empty(), "nothing to replace yet");
    assert!(session.store().queued_jobs().unwrap().is_empty());
    session.run("compose.update", &json!({ "id": id.get(), "text": "Hello Béla, the budget." })).unwrap();
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (second, raw) = upload(&mut session);
    assert!(raw.contains("the budget."), "{raw}");
    appended(&mut session, second.id, Some(31));
    assert_eq!(draft(&session, id).uploaded, Some(copy(31)));
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs[0].kind,
        JobKind::Expunge {
            folder: FolderId::new(6),
            path: crate::cmd::act_tests::path("Drafts"),
            uid_validity: validity(),
            uids: UidSet::from_iter([uid(30)])
        }
    );
}

#[test]
fn a_change_while_a_copy_is_on_its_way_goes_up_after_it() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (first, _) = upload(&mut session);
    session.run("compose.update", &json!({ "id": id.get(), "subject": "Budget 2026" })).unwrap();
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (second, raw) = upload(&mut session);
    assert!(raw.contains("Subject: Budget 2026"), "{raw}");
    appended(&mut session, first.id, Some(30));
    appended(&mut session, second.id, Some(31));
    assert_eq!(draft(&session, id).uploaded, Some(copy(31)));
    let expunged: Vec<UidSet> =
        session.take_jobs().into_iter().filter_map(|job| if let JobKind::Expunge { uids, .. } = job.kind { Some(uids) } else { None }).collect();
    assert_eq!(expunged, [UidSet::from_iter([uid(30)])]);
}

#[test]
fn a_copy_without_a_uid_still_replaces_the_one_before() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    let mut kept = draft(&session, id);
    kept.uploaded = Some(copy(30));
    crate::test_support::change(&mut session, Change::PutDraft(kept));
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (job, _) = upload(&mut session);
    appended(&mut session, job.id, None);
    assert_eq!(draft(&session, id).uploaded, None, "a server without UIDPLUS names no UID");
    assert!(draft(&session, id).unnamed);
    assert_eq!(session.take_jobs().len(), 1, "the copy before goes");
}

#[test]
fn without_a_uid_a_draft_leaves_at_most_one_copy() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (job, _) = upload(&mut session);
    appended(&mut session, job.id, None);
    assert!(draft(&session, id).unnamed, "an unnamed copy is in the Drafts folder");
    session.run("compose.update", &json!({ "id": id.get(), "text": "Hello Béla, the budget." })).unwrap();
    assert_eq!(session.next_due().unwrap(), None);
    wait(&clock, UPLOAD_AFTER.as_secs());
    session.tick().unwrap();
    assert!(session.take_jobs().is_empty(), "no second copy goes up");
}

#[test]
fn a_copy_of_a_discarded_draft_is_taken_back() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (job, _) = upload(&mut session);
    session.run("compose.discard", &json!({ "id": id.get() })).unwrap();
    assert!(session.take_jobs().is_empty(), "no copy known yet");
    appended(&mut session, job.id, Some(30));
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert!(matches!(&jobs[0].kind, JobKind::Expunge { uids, .. } if *uids == UidSet::from_iter([uid(30)])));
    // Without a UID there is nothing to take back.
    let id = typed(&mut session);
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (job, _) = upload(&mut session);
    session.run("compose.discard", &json!({ "id": id.get() })).unwrap();
    appended(&mut session, job.id, None);
    assert!(session.take_jobs().is_empty());
}

#[test]
fn an_append_that_is_not_a_draft_changes_no_draft() {
    let (mut session, _) = session();
    let id = typed(&mut session);
    let kind = JobKind::Append {
        folder: FolderId::new(6),
        path: crate::cmd::act_tests::path("Drafts"),
        flags: inboxcraft_model::Flags::default(),
        raw: b"Subject: x\r\n\r\n".to_vec(),
        draft: None,
        message: None,
    };
    let job = Job::new(JobId::new(77), AccountId::new(1), kind);
    crate::test_support::change(&mut session, Change::QueueJob(job));
    let mut session = session_at(store_of(&session), &FakeClock::new(at(NOW)));
    session.start();
    session.take_jobs();
    assert_eq!(appended(&mut session, JobId::new(77), Some(40)), Applied::Changed);
    assert_eq!(draft(&session, id).uploaded, None);
    assert!(session.take_jobs().is_empty());
    // An answer for a job nobody kept changes nothing either.
    appended(&mut session, JobId::new(78), Some(41));
    assert!(session.take_jobs().is_empty());
}

/// A copy of the session's store, as a restart would find it.
fn store_of(session: &Session) -> inboxcraft_store::Memory {
    let mut store = inboxcraft_store::Memory::new(inboxcraft_store::StoreConfig::default());
    let mut batch = Batch::new();
    for account in session.store().accounts().unwrap() {
        batch.push(Change::PutAccount(account.clone()));
        for folder in session.store().folders(account.id()).unwrap() {
            batch.push(Change::PutFolder(folder.clone()));
            if let Some(state) = session.store().folder_state(folder.id()).unwrap() {
                batch.push(Change::SetFolderState(folder.id(), state));
            }
        }
        for draft in session.store().drafts(account.id()).unwrap() {
            batch.push(Change::PutDraft(draft));
        }
    }
    for job in session.store().queued_jobs().unwrap() {
        batch.push(Change::QueueJob(job));
    }
    store.apply(&batch).unwrap();
    store
}

#[test]
fn an_upload_cut_off_by_a_restart_goes_up_again_and_its_answer_still_counts() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (job, _) = upload(&mut session);
    let mut again = session_at(store_of(&session), &clock);
    again.start();
    let replayed: Vec<Job> = again.take_jobs().into_iter().filter(|queued| queued.id == job.id).collect();
    assert_eq!(replayed, std::slice::from_ref(&job));
    again.tick().unwrap();
    assert!(again.take_jobs().is_empty(), "the version on its way is not sent twice");
    appended(&mut again, job.id, Some(30));
    assert_eq!(draft(&again, id).uploaded, Some(copy(30)));
}

#[test]
fn a_draft_with_nothing_typed_or_without_a_drafts_folder_stays_here() {
    let (mut session, clock) = session();
    session.run("compose.new", &json!({})).unwrap();
    let other = session.run("compose.new", &json!({ "account": 2 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": other, "text": "Account 2 has no Drafts folder" })).unwrap();
    assert_eq!(session.next_due().unwrap(), None);
    wait(&clock, UPLOAD_AFTER.as_secs() * 10);
    session.tick().unwrap();
    assert!(session.take_jobs().is_empty());
}

#[test]
fn a_failed_upload_waits_for_the_next_change() {
    let (mut session, clock) = session();
    let id = typed(&mut session);
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (job, _) = upload(&mut session);
    let failed = EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(FolderId::new(6)) };
    session.apply(&Event { account: AccountId::new(1), job: Some(job.id), kind: failed }).unwrap();
    assert!(session.store().queued_jobs().unwrap().is_empty());
    wait(&clock, UPLOAD_AFTER.as_secs());
    session.tick().unwrap();
    assert!(session.take_jobs().is_empty(), "no loop on a folder that refuses");
    session.run("compose.update", &json!({ "id": id.get(), "text": "Once more" })).unwrap();
    wait(&clock, UPLOAD_AFTER.as_secs());
    upload(&mut session);
}

#[test]
fn the_copy_says_what_the_draft_answers_and_keeps_bcc_here() {
    let (mut session, clock) = session();
    let id = session.run("compose.new", &json!({ "answers": 1 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": id, "bcc": ["dora@example.org"], "cc": ["cili@example.net"], "text": "Line\nnext" })).unwrap();
    wait(&clock, UPLOAD_AFTER.as_secs());
    let (_, raw) = upload(&mut session);
    assert!(raw.contains("Cc: cili@example.net"), "{raw}");
    assert!(!raw.contains("dora"), "Bcc stays here: {raw}");
    assert!(raw.contains("Line\r\nnext"), "{raw}");
}

#[test]
fn an_answer_naming_a_job_that_is_no_append_changes_no_draft() {
    let (mut session, _) = session();
    let id = typed(&mut session);
    let kind = JobKind::SyncFolderList;
    crate::test_support::change(&mut session, Change::QueueJob(Job::new(JobId::new(77), AccountId::new(1), kind)));
    let mut session = session_at(store_of(&session), &FakeClock::new(at(NOW)));
    appended(&mut session, JobId::new(77), Some(40));
    assert_eq!(draft(&session, id).uploaded, None);
    assert!(session.take_jobs().is_empty());
}

/// An answer to an invitation carries its iTIP object as a `text/calendar` part with the object's own `METHOD`; an
/// object that names none cannot be sent.
#[test]
fn a_draft_with_a_calendar_object_carries_it_with_its_method() {
    let account = crate::test_support::account(1, "anna@example.org");
    let reply = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n";
    let draft = Draft {
        to: vec!["odile@example.org".to_owned()],
        text: "Going.".to_owned(),
        calendar: Some(reply.to_owned()),
        ..Draft::new(DraftId::new(3), AccountId::new(1), at(NOW))
    };
    let raw = super::built(&account, &draft, None, None, &[]).unwrap();
    let message = inboxcraft_mime::parse(&raw).unwrap();
    assert!(message.content_type().is("multipart", "alternative"));
    let inboxcraft_mime::Body::Multipart(parts) = message.body() else { panic!("not a multipart") };
    let calendar = parts.parts().iter().find(|part| part.content_type().is("text", "calendar")).unwrap();
    assert_eq!(calendar.content_type().param("method"), Some("REPLY"));
    assert_eq!(calendar.text().unwrap(), reply);
    let nameless = Draft { calendar: Some("BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".to_owned()), ..draft.clone() };
    assert!(super::built(&account, &nameless, None, None, &[]).is_err());
    let broken = Draft { calendar: Some("not a calendar".to_owned()), ..draft };
    assert!(super::built(&account, &broken, None, None, &[]).is_err());
}
