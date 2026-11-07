// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Live updates: a watched folder in IDLE, the resync each change brings, IDLE renewed and ended by a job, polling
//! where the server has no IDLE, and the resync after a connection lost while watching.

mod support;

use std::time::Duration;

use inboxcraft_model::{ConnectionState, ErrorKind, EventKind, FolderId, JobId, JobKind};
use inboxcraft_net::WatchConfig;
use inboxcraft_testkit::Fault;
use inboxcraft_testkit::imap::{Action, Capability, FakeImap, Message, When};
use support::Result;
use support::run::{INBOX, Options, Run, added, changed, expunged, path, state, sync_inbox, validity};

fn idle_inbox() -> Result<JobKind> {
    Ok(JobKind::Idle { folder: INBOX, path: path("INBOX")? })
}

fn deliver() -> Action {
    Action::Deliver { mailbox: "INBOX".to_owned(), message: Message::new("From: dora@example.org\nSubject: Just in\n\nHello.\n") }
}

fn watching(kind: &EventKind) -> bool {
    *kind == EventKind::Connection(ConnectionState::Watching)
}

/// Starts a worker on `fake`, syncs INBOX and watches it; the Idle job's id once the worker is watching.
fn watch_inbox(fake: FakeImap, options: Options) -> Result<(Run, JobId)> {
    let mut run = Run::with(fake, options)?;
    run.job(sync_inbox(None)?)?;
    run.synced()?;
    let idle = run.job(idle_inbox()?)?;
    run.until(watching)?;
    Ok((run, idle))
}

/// The events up to the next FolderSynced.
fn next_resync(run: &Run) -> Result<Vec<EventKind>> {
    run.until(|kind| matches!(kind, EventKind::FolderSynced { .. } | EventKind::Failed { .. }))
}

fn idles(lines: &[String]) -> usize {
    lines.iter().filter(|line| line.ends_with(" IDLE")).count()
}

#[test]
fn new_mail_while_idling_is_brought_in_by_a_resync() -> Result {
    let (run, idle) = watch_inbox(FakeImap::sample().on(When::Idle, [deliver()]), Options::default())?;
    let events = run.events_until(|event| matches!(event.kind, EventKind::FolderSynced { .. }))?;
    assert!(events.iter().all(|event| event.job == Some(idle)), "{events:?}");
    let kinds: Vec<_> = events.into_iter().map(|event| event.kind).collect();
    assert_eq!(added(&kinds).iter().map(|summary| summary.envelope().subject().to_owned()).collect::<Vec<_>>(), ["Just in"]);
    assert_eq!(state(&kinds).and_then(|state| state.uid_next()).map(|uid| uid.get()), Some(5));
    let lines = run.finish()?;
    assert_eq!(idles(&lines), 2, "{lines:?}");
    // Stopping ends the second IDLE and signs out.
    assert_eq!(lines.iter().rev().take(2).map(|line| line.rsplit(' ').next().unwrap_or_default()).collect::<Vec<_>>(), ["LOGOUT", "DONE"]);
    Ok(())
}

#[test]
fn changed_and_removed_mail_while_idling_is_reported() -> Result {
    let flags = Action::Flags { mailbox: "INBOX".to_owned(), uid: 2, flags: vec!["\\Seen".to_owned()] };
    let gone = Action::Expunge { mailbox: "INBOX".to_owned(), uid: 3 };
    let (run, _idle) = watch_inbox(FakeImap::sample().on(When::Idle, [flags, gone]), Options::default())?;
    let events = next_resync(&run)?;
    assert_eq!(expunged(&events), [3]);
    assert_eq!(changed(&events).iter().map(|change| change.uid.get()).collect::<Vec<_>>(), [2]);
    run.finish()?;
    Ok(())
}

#[test]
fn a_new_uidvalidity_while_idling_reloads_the_folder() -> Result {
    let renumbered = Action::UidValidity { mailbox: "INBOX".to_owned(), value: 77 };
    let (run, _idle) = watch_inbox(FakeImap::sample().on(When::Idle, [renumbered]), Options::default())?;
    let events = next_resync(&run)?;
    assert!(events.contains(&EventKind::UidValidityReset { folder: INBOX, old: Some(validity(1)?), new: validity(77)? }), "{events:?}");
    assert_eq!(added(&events).len(), 3);
    assert_eq!(state(&events).map(|state| state.uid_validity()), Some(validity(77)?));
    run.finish()?;
    Ok(())
}

#[test]
fn a_job_ends_the_idle_and_the_watch_resumes_after_it() -> Result {
    let (mut run, idle) = watch_inbox(FakeImap::sample(), Options::default())?;
    let list = run.job(JobKind::SyncFolderList)?;
    let events = run.events_until(|event| event.job == Some(list))?;
    let kinds: Vec<_> = events.iter().map(|event| event.kind.clone()).collect();
    assert_eq!(kinds[0], EventKind::Connection(ConnectionState::Online), "{kinds:?}");
    assert!(matches!(kinds.last(), Some(EventKind::Folders { .. })));
    // The list may have opened nothing, yet the watched folder is opened again before IDLE.
    let events = run.events_until(|event| watching(&event.kind))?;
    assert!(events.iter().any(|event| event.job == Some(idle) && matches!(event.kind, EventKind::FolderSynced { .. })), "{events:?}");
    let lines = run.finish()?;
    let list_at = lines.iter().position(|line| line.contains(" LIST ")).ok_or("no LIST")?;
    assert_eq!(lines.get(list_at - 1).map(String::as_str), Some("DONE"), "{lines:?}");
    assert_eq!(idles(&lines), 2, "{lines:?}");
    Ok(())
}

#[test]
fn idle_is_renewed_before_the_server_ends_it() -> Result {
    let watch = WatchConfig { tick: Duration::from_millis(10), renew: Duration::from_millis(60), ..WatchConfig::default() };
    let (run, _idle) = watch_inbox(FakeImap::sample(), Options { watch, ..Options::default() })?;
    std::thread::sleep(Duration::from_millis(400));
    let lines = run.finish()?;
    assert!(idles(&lines) >= 3, "{lines:?}");
    // A renewal brings no resync: one EXAMINE for the job, one for the watch.
    assert_eq!(lines.iter().filter(|line| line.contains("EXAMINE")).count(), 2, "{lines:?}");
    Ok(())
}

#[test]
fn without_idle_the_worker_polls_with_a_resync() -> Result {
    let fake = FakeImap::sample()
        .without(Capability::Idle)
        .on(When::command("EXAMINE"), [])
        .on(When::command("EXAMINE"), [])
        .on(When::command("EXAMINE"), [deliver()]);
    let watch = WatchConfig { poll: Duration::from_millis(50), ..WatchConfig::default() };
    let (run, _idle) = watch_inbox(fake, Options { watch, ..Options::default() })?;
    let events = next_resync(&run)?;
    assert_eq!(added(&events).iter().map(|summary| summary.uid().get()).collect::<Vec<_>>(), [4]);
    let lines = run.finish()?;
    assert_eq!(idles(&lines), 0, "{lines:?}");
    Ok(())
}

#[test]
fn a_connection_lost_while_idling_is_reconnected_and_the_folder_resynced() -> Result {
    let fake = FakeImap::sample().on(When::Idle, [deliver(), Action::Fault(Fault::Drop)]);
    let (run, idle) = watch_inbox(fake, Options { connections: 3, ..Options::default() })?;
    let events = run.events_until(|event| event.job == Some(idle) && matches!(event.kind, EventKind::FolderSynced { .. }))?;
    let kinds: Vec<_> = events.iter().map(|event| event.kind.clone()).collect();
    let lost = kinds.iter().position(|kind| *kind == EventKind::Connection(ConnectionState::Offline)).ok_or("not offline")?;
    assert!(matches!(kinds.get(lost - 1), Some(EventKind::Failed { kind: ErrorKind::ConnectionDropped, folder: Some(INBOX) })), "{kinds:?}");
    assert_eq!(
        kinds[lost + 1..lost + 4],
        [
            EventKind::Connection(ConnectionState::BackingOff { attempt: 1, retry_in_secs: 1 }),
            EventKind::Connection(ConnectionState::Connecting),
            EventKind::Connection(ConnectionState::Online),
        ]
    );
    // The resync after the reconnect brings the message delivered before the drop.
    assert_eq!(added(&kinds).iter().map(|summary| summary.uid().get()).collect::<Vec<_>>(), [4]);
    run.until(watching)?;
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("AUTHENTICATE")).count(), 2);
    assert!(lines.iter().any(|line| line.contains("EXAMINE \"INBOX\" (QRESYNC (")), "{lines:?}");
    Ok(())
}

#[test]
fn a_watched_folder_that_is_gone_ends_the_watch() -> Result {
    let mut run = Run::with(FakeImap::sample(), Options::default())?;
    let idle = run.job(JobKind::Idle { folder: FolderId::new(99), path: path("Nowhere")? })?;
    let events = run.events_until(|event| event.job == Some(idle))?;
    assert_eq!(events.last().map(|event| &event.kind), Some(&EventKind::Failed { kind: ErrorKind::FolderGone, folder: Some(FolderId::new(99)) }));
    // No watch is left: the worker waits for the next job and runs it.
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    assert!(!events.iter().any(watching), "{events:?}");
    let lines = run.finish()?;
    assert_eq!(idles(&lines), 0, "{lines:?}");
    Ok(())
}

#[test]
fn another_idle_job_moves_the_watch() -> Result {
    let (mut run, _idle) = watch_inbox(FakeImap::sample(), Options::default())?;
    let archive = FolderId::new(4);
    let moved = run.job(JobKind::Idle { folder: archive, path: path("Archive")? })?;
    let events = run.events_until(|event| event.job == Some(moved) && matches!(event.kind, EventKind::FolderSynced { .. }))?;
    assert!(events.iter().any(|event| matches!(&event.kind, EventKind::FolderSynced { folder, .. } if *folder == archive)));
    let lines = run.finish()?;
    assert!(lines.iter().any(|line| line.contains("EXAMINE Archive") || line.contains("EXAMINE \"Archive\"")), "{lines:?}");
    Ok(())
}
