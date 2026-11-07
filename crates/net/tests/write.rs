// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The write path against the fake IMAP server: flags, move, delete for good, APPEND and CREATE; a message gone
//! meanwhile told as an event; the offline queue run after the reconnect; and each fault script ending in a state a
//! sync confirms.

mod support;

use std::time::Duration;

use inboxcraft_model::{ConnectionState, Copied, ErrorKind, EventKind, Flag, Flags, FolderId, FolderPath, JobId, JobKind, UidSet};
use inboxcraft_net::Timeouts;
use inboxcraft_testkit::Fault;
use inboxcraft_testkit::imap::{Action, Capability, FakeImap, When};
use support::run::{INBOX, Options, Run, added, changed, expunged, path, sync_inbox, uid, validity};
use support::{Result, TIMEOUTS};

const ARCHIVE: FolderId = FolderId::new(11);
const DRAFTS: FolderId = FolderId::new(12);

fn uids(list: &[u32]) -> Result<UidSet> {
    list.iter().map(|value| uid(*value)).collect()
}

fn flags(list: &[Flag]) -> Flags {
    list.iter().cloned().collect()
}

fn store(set: &[u32], add: &[Flag], remove: &[Flag]) -> Result<JobKind> {
    Ok(JobKind::StoreFlags {
        folder: INBOX,
        path: path("INBOX")?,
        uid_validity: validity(1)?,
        uids: uids(set)?,
        add: flags(add),
        remove: flags(remove),
    })
}

fn move_to_archive(set: &[u32]) -> Result<JobKind> {
    Ok(JobKind::Move { folder: INBOX, path: path("INBOX")?, uid_validity: validity(1)?, uids: uids(set)?, to: ARCHIVE, to_path: path("Archive")? })
}

fn expunge(set: &[u32]) -> Result<JobKind> {
    Ok(JobKind::Expunge { folder: INBOX, path: path("INBOX")?, uid_validity: validity(1)?, uids: uids(set)? })
}

fn append(raw: &[u8]) -> Result<JobKind> {
    Ok(JobKind::Append { folder: DRAFTS, path: path("Drafts")?, flags: flags(&[Flag::Seen, Flag::Draft]), raw: raw.to_vec() })
}

fn sync_archive() -> Result<JobKind> {
    Ok(JobKind::SyncFolder { folder: ARCHIVE, path: path("Archive")?, known: None })
}

/// The events of `job`, up to its last one.
fn of_job(run: &Run, job: JobId) -> Result<Vec<EventKind>> {
    let events = run.events_until(|event| event.job == Some(job) && ends(&event.kind))?;
    Ok(events.into_iter().filter(|event| event.job == Some(job)).map(|event| event.kind).collect())
}

fn ends(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::FlagsStored { .. }
            | EventKind::Moved { .. }
            | EventKind::Expunged { .. }
            | EventKind::Appended { .. }
            | EventKind::FolderCreated { .. }
            | EventKind::FolderSynced { .. }
            | EventKind::Failed { .. }
    )
}

/// The flags each message has after a sync of the inbox, by UID.
fn inbox_flags(run: &mut Run) -> Result<Vec<(u32, Flags)>> {
    let job = run.job(sync_inbox(None)?)?;
    let events = of_job(run, job)?;
    Ok(added(&events).iter().map(|summary| (summary.uid().get(), summary.flags().clone())).collect())
}

/// The UIDs of the messages in Archive after a sync.
fn archive_uids(run: &mut Run) -> Result<Vec<u32>> {
    let job = run.job(sync_archive()?)?;
    Ok(added(&of_job(run, job)?).iter().map(|summary| summary.uid().get()).collect())
}

#[test]
fn flags_are_stored_and_told_as_the_server_has_them() -> Result {
    let mut run = Run::sample()?;
    let job = run.job(store(&[2, 3], &[Flag::Seen], &[Flag::Flagged])?)?;
    let events = of_job(&run, job)?;
    let told: Vec<(u32, Flags)> = changed(&events).into_iter().map(|change| (change.uid.get(), change.flags)).collect();
    assert_eq!(told, [(2, flags(&[Flag::Seen])), (3, flags(&[Flag::Seen]))], "{events:?}");
    assert_eq!(events.last(), Some(&EventKind::FlagsStored { folder: INBOX, uid_validity: validity(1)?, uids: uids(&[2, 3])? }));
    assert_eq!(inbox_flags(&mut run)?, [(1, flags(&[Flag::Seen])), (2, flags(&[Flag::Seen])), (3, flags(&[Flag::Seen]))]);
    let lines = run.finish()?;
    assert!(lines.iter().any(|line| line.ends_with("SELECT \"INBOX\" (CONDSTORE)")), "{lines:?}");
    assert!(lines.iter().any(|line| line.ends_with("UID STORE 2:3 +FLAGS.SILENT (\\Seen)")), "{lines:?}");
    assert!(lines.iter().any(|line| line.ends_with("UID STORE 2:3 -FLAGS.SILENT (\\Flagged)")), "{lines:?}");
    Ok(())
}

#[test]
fn a_message_gone_meanwhile_is_told_as_expunged() -> Result {
    let fake = FakeImap::sample().on(When::command("SELECT"), [Action::Expunge { mailbox: "INBOX".to_owned(), uid: 2 }]);
    let mut run = Run::start(fake, Some(inboxcraft_testkit::imap::PASSWORD), Default::default())?;
    let job = run.job(store(&[2, 3], &[Flag::Seen], &[])?)?;
    let events = of_job(&run, job)?;
    assert_eq!(expunged(&events), [2], "{events:?}");
    assert_eq!(changed(&events).iter().map(|change| change.uid.get()).collect::<Vec<_>>(), [3]);
    assert!(matches!(events.last(), Some(EventKind::FlagsStored { .. })), "{events:?}");
    run.finish()?;
    Ok(())
}

#[test]
fn a_changed_uidvalidity_voids_the_job_without_writing() -> Result {
    let mut run = Run::sample()?;
    let stale = validity(5)?;
    let job = run.job(JobKind::StoreFlags {
        folder: INBOX,
        path: path("INBOX")?,
        uid_validity: stale,
        uids: uids(&[1])?,
        add: flags(&[Flag::Flagged]),
        remove: Flags::default(),
    })?;
    let events = of_job(&run, job)?;
    assert_eq!(
        events,
        [
            EventKind::UidValidityReset { folder: INBOX, old: Some(stale), new: validity(1)? },
            EventKind::FlagsStored { folder: INBOX, uid_validity: stale, uids: uids(&[1])? }
        ]
    );
    let lines = run.finish()?;
    assert!(!lines.iter().any(|line| line.contains("UID STORE")), "{lines:?}");
    Ok(())
}

#[test]
fn a_folder_gone_fails_the_job_and_keeps_the_connection() -> Result {
    let mut run = Run::sample()?;
    let job = run.job(JobKind::StoreFlags {
        folder: INBOX,
        path: path("Gone")?,
        uid_validity: validity(1)?,
        uids: uids(&[1])?,
        add: flags(&[Flag::Seen]),
        remove: Flags::default(),
    })?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::FolderGone, folder: Some(INBOX) }]);
    let job = run.job(store(&[1], &[Flag::Flagged], &[])?)?;
    assert!(matches!(of_job(&run, job)?.last(), Some(EventKind::FlagsStored { .. })));
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("AUTHENTICATE")).count(), 1);
    Ok(())
}

#[test]
fn a_move_tells_the_new_uids_and_a_sync_finds_the_message_once() -> Result {
    let mut run = Run::sample()?;
    let job = run.job(move_to_archive(&[1, 3])?)?;
    let copied = Copied { uid_validity: validity(7)?, uids: vec![(uid(1)?, uid(13)?), (uid(3)?, uid(14)?)] };
    assert_eq!(
        of_job(&run, job)?,
        [EventKind::Moved { folder: INBOX, uid_validity: validity(1)?, uids: uids(&[1, 3])?, to: ARCHIVE, copied: Some(copied) }]
    );
    assert_eq!(inbox_flags(&mut run)?.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [2]);
    assert_eq!(archive_uids(&mut run)?, [12, 13, 14]);
    run.finish()?;
    Ok(())
}

#[test]
fn a_move_of_messages_gone_meanwhile_names_no_new_uid() -> Result {
    let fake = FakeImap::sample().on(When::command("SELECT"), [Action::Expunge { mailbox: "INBOX".to_owned(), uid: 1 }]);
    let mut run = Run::start(fake, Some(inboxcraft_testkit::imap::PASSWORD), Default::default())?;
    let job = run.job(move_to_archive(&[1])?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Moved { folder: INBOX, uid_validity: validity(1)?, uids: uids(&[1])?, to: ARCHIVE, copied: None }]);
    assert_eq!(archive_uids(&mut run)?, [12]);
    run.finish()?;
    Ok(())
}

#[test]
fn a_move_without_move_copies_marks_and_expunges() -> Result {
    let mut run = Run::start(FakeImap::sample().without(Capability::Move), Some(inboxcraft_testkit::imap::PASSWORD), Default::default())?;
    let job = run.job(move_to_archive(&[2])?)?;
    let copied = Copied { uid_validity: validity(7)?, uids: vec![(uid(2)?, uid(13)?)] };
    assert_eq!(
        of_job(&run, job)?,
        [EventKind::Moved { folder: INBOX, uid_validity: validity(1)?, uids: uids(&[2])?, to: ARCHIVE, copied: Some(copied) }]
    );
    assert_eq!(inbox_flags(&mut run)?.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [1, 3]);
    run.finish()?;
    Ok(())
}

#[test]
fn a_move_to_a_folder_gone_fails_as_folder_gone() -> Result {
    let mut run = Run::sample()?;
    let job = run.job(JobKind::Move {
        folder: INBOX,
        path: path("INBOX")?,
        uid_validity: validity(1)?,
        uids: uids(&[1])?,
        to: ARCHIVE,
        to_path: path("Gone")?,
    })?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::FolderGone, folder: Some(INBOX) }]);
    run.finish()?;
    Ok(())
}

#[test]
fn a_delete_for_good_removes_only_its_messages() -> Result {
    let fake =
        FakeImap::sample().on(When::command("SELECT"), [Action::Flags { mailbox: "INBOX".to_owned(), uid: 1, flags: vec!["\\Deleted".to_owned()] }]);
    let mut run = Run::start(fake, Some(inboxcraft_testkit::imap::PASSWORD), Default::default())?;
    let job = run.job(expunge(&[3])?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Expunged { folder: INBOX, uid_validity: validity(1)?, uids: uids(&[3])? }]);
    // Message 1, marked deleted by someone else, is still there.
    assert_eq!(inbox_flags(&mut run)?.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [1, 2]);
    let lines = run.finish()?;
    assert!(lines.iter().any(|line| line.ends_with("UID STORE 3 +FLAGS.SILENT (\\Deleted)")), "{lines:?}");
    assert!(lines.iter().any(|line| line.ends_with("UID EXPUNGE 3")), "{lines:?}");
    assert!(!lines.iter().any(|line| line.ends_with(" EXPUNGE") || line.ends_with(" CLOSE")), "{lines:?}");
    Ok(())
}

#[test]
fn a_delete_for_good_without_uidplus_is_refused_before_anything_is_sent() -> Result {
    let mut run = Run::start(FakeImap::sample().without(Capability::UidPlus), Some(inboxcraft_testkit::imap::PASSWORD), Default::default())?;
    let job = run.job(expunge(&[3])?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(INBOX) }]);
    let lines = run.finish()?;
    assert!(!lines.iter().any(|line| line.contains("STORE") || line.contains("EXPUNGE")), "{lines:?}");
    Ok(())
}

#[test]
fn append_tells_the_uid_the_message_got() -> Result {
    let mut run = Run::sample()?;
    let job = run.job(append(b"From: ada@example.org\r\nSubject: Plan\r\n\r\nA draft.\r\n")?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Appended { folder: DRAFTS, uid: Some((validity(1)?, uid(1)?)) }]);
    let job = run.job(JobKind::SyncFolder { folder: DRAFTS, path: path("Drafts")?, known: None })?;
    let drafts = added(&of_job(&run, job)?);
    assert_eq!(drafts.len(), 1);
    assert!(drafts.iter().all(|summary| summary.flags().contains(&Flag::Draft)));
    run.finish()?;
    Ok(())
}

#[test]
fn append_of_what_no_literal_carries_fails_and_keeps_the_connection() -> Result {
    let mut run = Run::sample()?;
    let job = run.job(append(b"Subject: \0\r\n\r\n")?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::CommandRefused, folder: Some(DRAFTS) }]);
    let job = run.job(JobKind::Append { folder: DRAFTS, path: path("Gone")?, flags: Flags::default(), raw: b"Subject: x\r\n\r\n".to_vec() })?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::FolderGone, folder: Some(DRAFTS) }]);
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("AUTHENTICATE")).count(), 1);
    Ok(())
}

#[test]
fn create_makes_a_folder_and_one_that_exists_counts_as_made() -> Result {
    let mut run = Run::sample()?;
    let screener = FolderPath::new("InboxCraft/Screener", Some('/'))?;
    for _ in 0..2 {
        let job = run.job(JobKind::CreateFolder { path: screener.clone() })?;
        assert_eq!(of_job(&run, job)?, [EventKind::FolderCreated { path: screener.clone() }]);
    }
    let job = run.job(JobKind::SyncFolderList)?;
    let events = run.events_until(|event| event.job == Some(job))?;
    let listed = events.iter().any(|event| {
        matches!(&event.kind, EventKind::Folders { listings } if listings.iter().any(|listing| listing.path().name() == "InboxCraft/Screener"))
    });
    assert!(listed, "{events:?}");
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.ends_with("CREATE \"InboxCraft/Screener\"")).count(), 2);
    Ok(())
}

#[test]
fn writes_queued_offline_run_in_order_once_the_worker_is_online() -> Result {
    let fake = FakeImap::sample().on(When::Greeting, [Action::Fault(Fault::Drop)]).on(When::Greeting, [Action::Fault(Fault::Drop)]);
    let mut run = Run::with(fake, Options { connections: 3, ..Options::default() })?;
    let flag = run.job(store(&[2], &[Flag::Flagged], &[])?)?;
    let moved = run.job(move_to_archive(&[2])?)?;
    let events = run.events_until(|event| event.job == Some(moved) && ends(&event.kind))?;
    assert!(!events.iter().any(|event| matches!(event.kind, EventKind::Failed { .. }) && event.job.is_some()), "{events:?}");
    let order: Vec<JobId> = events.iter().filter(|event| ends(&event.kind)).filter_map(|event| event.job).collect();
    assert_eq!(order, [flag, moved]);
    assert!(events.iter().any(|event| matches!(event.kind, EventKind::Connection(ConnectionState::BackingOff { attempt: 2, .. }))));
    // The flag went with the message.
    let job = run.job(sync_archive()?)?;
    let archive = added(&of_job(&run, job)?);
    assert_eq!(archive.iter().map(|summary| (summary.uid().get(), summary.flags().is_flagged())).collect::<Vec<_>>(), [(12, false), (13, true)]);
    run.finish()?;
    Ok(())
}

/// A move whose connection fails at `when` with `fault`, then the state a sync finds: the message moved once, and the
/// fake got UID MOVE `sent` times. The fake serves one connection after the other, so a stalled one holds up those
/// after it: it gets plenty.
fn interrupted_move(when: When, fault: Fault, timeouts: Timeouts, sent: usize) -> Result {
    let fake = FakeImap::sample().on(when, [Action::Fault(fault)]);
    let mut run = Run::with(fake, Options { connections: 20, timeouts, ..Options::default() })?;
    let job = run.job(move_to_archive(&[1])?)?;
    let events = of_job(&run, job)?;
    assert!(matches!(events.last(), Some(EventKind::Moved { copied: Some(_), .. })), "{events:?}");
    assert_eq!(inbox_flags(&mut run)?.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [2, 3]);
    assert_eq!(archive_uids(&mut run)?, [12, 13]);
    run.worker.stop();
    // A connection the worker gave up on fails on the fake's side: only the lines count.
    let (transcript, _ended) = run.fake.finish_with_transcript();
    let lines = transcript.received();
    assert_eq!(lines.iter().filter(|line| line.contains("UID MOVE")).count(), sent, "{lines:?}");
    Ok(())
}

#[test]
fn a_write_whose_connection_drops_runs_again_after_the_reconnect() -> Result {
    interrupted_move(When::command("UID MOVE"), Fault::Drop, TIMEOUTS, 2)
}

#[test]
fn a_write_whose_server_stalls_runs_again_after_the_reconnect() -> Result {
    let timeouts = Timeouts { io: Duration::from_millis(300), ..TIMEOUTS };
    interrupted_move(When::command("SELECT"), Fault::Delay(Duration::from_millis(1_500)), timeouts, 1)
}

#[test]
fn a_move_without_move_cut_off_after_the_copy_fails_and_does_not_copy_again() -> Result {
    let drop = Action::Fault(Fault::Drop);
    let fake = FakeImap::sample().without(Capability::Move).on(When::command("UID STORE"), [drop]);
    let mut run = Run::with(fake, Options { connections: 4, ..Options::default() })?;
    let job = run.job(move_to_archive(&[1])?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::ConnectionDropped, folder: Some(INBOX) }]);
    // Copied once and still in the inbox: the engine takes the move back, and the syncs show the server's state.
    assert_eq!(inbox_flags(&mut run)?.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(archive_uids(&mut run)?, [12, 13]);
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("UID COPY")).count(), 1, "{lines:?}");
    Ok(())
}

#[test]
fn garbage_in_answer_to_a_write_fails_it_and_a_sync_finds_the_server_state() -> Result {
    let garbage = Action::Fault(Fault::Garbage(b"\x00\xff not imap\r\n".to_vec()));
    let fake = FakeImap::sample().on(When::command("UID FETCH"), [garbage]);
    let mut run = Run::with(fake, Options { connections: 2, ..Options::default() })?;
    let job = run.job(store(&[3], &[Flag::Seen], &[])?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::Protocol, folder: Some(INBOX) }]);
    // The store went through before the garbage: the server has the flag, and the sync says so.
    let flagged: Vec<(u32, bool)> = inbox_flags(&mut run)?.into_iter().map(|(uid, flags)| (uid, flags.is_seen())).collect();
    assert_eq!(flagged, [(1, true), (2, false), (3, true)]);
    // The connection the garbage ended fails on the fake's side.
    run.worker.stop();
    Ok(())
}

#[test]
fn a_write_that_drops_the_connection_every_time_fails_after_three_tries() -> Result {
    let drop = Action::Fault(Fault::Drop);
    let fake = (0..3).fold(FakeImap::sample(), |fake, _| fake.on(When::command("UID STORE"), [drop.clone()]));
    let mut run = Run::with(fake, Options { connections: 4, ..Options::default() })?;
    let job = run.job(store(&[1], &[Flag::Flagged], &[])?)?;
    assert_eq!(of_job(&run, job)?, [EventKind::Failed { kind: ErrorKind::ConnectionDropped, folder: Some(INBOX) }]);
    // The queue goes on.
    let next = run.job(store(&[2], &[Flag::Flagged], &[])?)?;
    assert!(matches!(of_job(&run, next)?.last(), Some(EventKind::FlagsStored { .. })));
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("UID STORE")).count(), 4, "{lines:?}");
    Ok(())
}
