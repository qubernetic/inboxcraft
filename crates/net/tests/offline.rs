// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The worker when the connection fails: back-off with a cap, the offline queue, and the state each fault script
//! (drop, delay, garbage) leaves the worker in.

mod support;

use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::{ConnectionState, ErrorKind, EventKind, JobId, JobKind};
use inboxcraft_net::{Backoff, Timeouts};
use inboxcraft_testkit::imap::{Action, Capability, FakeImap, When};
use inboxcraft_testkit::{CertFault, Fault};
use inboxcraft_time::{Clock, SystemClock};
use support::run::{INBOX, Options, Run, sync_inbox};
use support::{Result, SetBack, TIMEOUTS};

fn drop_at(when: When) -> FakeImap {
    FakeImap::sample().on(when, [Action::Fault(Fault::Drop)])
}

fn backing_off(attempt: u32) -> EventKind {
    EventKind::Connection(ConnectionState::BackingOff { attempt, retry_in_secs: 1 })
}

#[test]
fn the_worker_backs_off_and_keeps_its_queue_until_it_is_online() -> Result {
    let fake = drop_at(When::Greeting).on(When::Greeting, [Action::Fault(Fault::Drop)]);
    let mut run = Run::with(fake, Options { connections: 3, ..Options::default() })?;
    let list = run.job(JobKind::SyncFolderList)?;
    let inbox = run.job(sync_inbox(None)?)?;
    let events = run.events_until(|event| event.job == Some(inbox) && matches!(event.kind, EventKind::FolderSynced { .. }))?;
    let connection: Vec<_> = events.iter().filter(|event| event.job.is_none()).map(|event| event.kind.clone()).collect();
    let dropped = EventKind::Failed { kind: ErrorKind::ConnectionDropped, folder: None };
    assert_eq!(
        connection,
        [
            EventKind::Connection(ConnectionState::Connecting),
            dropped.clone(),
            backing_off(1),
            EventKind::Connection(ConnectionState::Connecting),
            dropped,
            backing_off(2),
            EventKind::Connection(ConnectionState::Connecting),
            EventKind::Connection(ConnectionState::Online),
            EventKind::Abilities { uid_expunge: true },
        ]
    );
    // Both jobs waited and ran, in order, and neither failed.
    let jobs: Vec<_> = events.iter().filter_map(|event| event.job).collect();
    assert_eq!(jobs.first(), Some(&list));
    assert!(events.iter().any(|event| event.job == Some(list) && matches!(event.kind, EventKind::Folders { .. })));
    assert!(!events.iter().any(|event| event.job.is_some() && matches!(event.kind, EventKind::Failed { .. })));
    run.finish()?;
    Ok(())
}

#[test]
fn a_job_beyond_the_queue_cap_fails_at_once_and_stop_ends_the_wait() -> Result {
    let backoff = Backoff { first: Duration::from_secs(30), cap: Duration::from_secs(30) };
    let mut run = Run::with(drop_at(When::Greeting), Options { backoff, max_queued: 1, ..Options::default() })?;
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::BackingOff { .. })))?;
    let refused = run.job(sync_inbox(None)?)?;
    let event = run.next()?;
    assert_eq!(event.job, Some(refused));
    assert_eq!(event.kind, EventKind::Failed { kind: ErrorKind::TooLarge, folder: Some(INBOX) });
    let clock = SystemClock::new();
    let before = clock.now();
    run.worker.stop();
    let took = clock.now().duration_since(before).unwrap_or_default();
    assert!(took < Duration::from_secs(5), "stop waited {took:?}");
    Ok(())
}

#[test]
fn the_wait_takes_new_jobs_and_keeps_its_length() -> Result {
    let backoff = Backoff { first: Duration::from_millis(400), cap: Duration::from_millis(400) };
    let fake = drop_at(When::Greeting);
    let mut run = Run::with(fake, Options { backoff, connections: 2, ..Options::default() })?;
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::BackingOff { .. })))?;
    let clock = SystemClock::new();
    let before = clock.now();
    run.job(JobKind::SyncFolderList)?;
    // Online, then what the server can do.
    run.until(|kind| matches!(kind, EventKind::Abilities { .. }))?;
    let took = clock.now().duration_since(before).unwrap_or_default();
    assert!(took >= Duration::from_millis(250), "a new job cut the wait short: {took:?}");
    let folders = run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    assert_eq!(folders.len(), 1);
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    run.finish()?;
    Ok(())
}

#[test]
fn a_clock_set_back_during_the_wait_does_not_stretch_it() -> Result {
    let backoff = Backoff { first: Duration::from_millis(400), cap: Duration::from_millis(400) };
    let clock = Arc::new(SetBack::default());
    let options = Options { backoff, connections: 2, clock: clock.clone(), ..Options::default() };
    let mut run = Run::with(drop_at(When::Greeting), options)?;
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::BackingOff { .. })))?;
    let stopwatch = SystemClock::new();
    clock.set_back(Duration::from_secs(30 * 60));
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| *kind == EventKind::Connection(ConnectionState::Online))?;
    let took = stopwatch.monotonic();
    assert!(took < Duration::from_secs(5), "the wait went past its cap: {took:?}");
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    run.finish()?;
    Ok(())
}

#[test]
fn a_dropped_connection_fails_its_job_and_the_next_job_reconnects() -> Result {
    let mut run = Run::with(drop_at(When::command("UID FETCH")), Options { connections: 2, ..Options::default() })?;
    run.job(sync_inbox(None)?)?;
    let events = run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::Offline)))?;
    assert!(events.contains(&EventKind::Failed { kind: ErrorKind::ConnectionDropped, folder: Some(INBOX) }), "{events:?}");
    run.job(sync_inbox(None)?)?;
    let events = run.synced()?;
    assert_eq!(events[..2], [EventKind::Connection(ConnectionState::Connecting), EventKind::Connection(ConnectionState::Online)], "{events:?}");
    assert!(matches!(events.last(), Some(EventKind::FolderSynced { .. })), "{events:?}");
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("AUTHENTICATE")).count(), 2);
    Ok(())
}

#[test]
fn garbage_from_the_server_fails_the_job_and_the_next_job_reconnects() -> Result {
    let garbage = Action::Fault(Fault::Garbage(b"* 1 FETCH (\x01\r\n".to_vec()));
    let fake = FakeImap::sample().on(When::command("LIST"), [garbage]);
    let mut run = Run::with(fake, Options { connections: 2, ..Options::default() })?;
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::Offline)))?;
    assert!(events.contains(&EventKind::Failed { kind: ErrorKind::Protocol, folder: None }), "{events:?}");
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Folders { .. } | EventKind::Failed { .. }))?;
    assert!(matches!(events.last(), Some(EventKind::Folders { .. })), "{events:?}");
    run.worker.stop();
    Ok(())
}

#[test]
fn a_server_that_stalls_times_out_and_the_next_job_reconnects() -> Result {
    let stall = Action::Fault(Fault::Delay(Duration::from_millis(1_500)));
    let fake = FakeImap::sample().on(When::command("LIST"), [stall]);
    let timeouts = Timeouts { io: Duration::from_millis(300), ..TIMEOUTS };
    let mut run = Run::with(fake, Options { connections: 20, timeouts, ..Options::default() })?;
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::Offline)))?;
    assert!(events.contains(&EventKind::Failed { kind: ErrorKind::TimedOut, folder: None }), "{events:?}");
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    assert!(events.contains(&EventKind::Connection(ConnectionState::Online)), "{events:?}");
    run.worker.stop();
    Ok(())
}

#[test]
fn a_certificate_fault_stops_the_tries_for_good() -> Result {
    let mut run = Run::with(FakeImap::sample(), Options { cert: Some(CertFault::UnknownCa), ..Options::default() })?;
    let first = run.job(JobKind::SyncFolderList)?;
    let events = run.events_until(|event| event.job == Some(first))?;
    let kinds: Vec<_> = events.iter().map(|event| event.kind.clone()).collect();
    let fault = |folder| EventKind::Failed { kind: ErrorKind::CertificateUnknownIssuer, folder };
    assert_eq!(
        kinds,
        [EventKind::Connection(ConnectionState::Connecting), fault(None), EventKind::Connection(ConnectionState::Offline), fault(None)]
    );
    run.job(sync_inbox(None)?)?;
    assert_eq!(run.until(|_| true)?, [fault(Some(INBOX))]);
    run.worker.stop();
    let (transcript, _refused) = run.fake.finish_with_transcript();
    assert!(transcript.received().is_empty());
    Ok(())
}

/// The lines with `word` the fake got.
fn sent(run: Run, word: &str) -> usize {
    run.worker.stop();
    let (transcript, _ended) = run.fake.finish_with_transcript();
    transcript.received().iter().filter(|line| line.contains(word)).count()
}

/// The fake answers `command` with `action` on each of four connections.
fn at_every_sign_in(fake: FakeImap, command: &str, action: &Action) -> FakeImap {
    (0..4).fold(fake, |fake, _| fake.on(When::command(command), [action.clone()]))
}

/// Runs a folder list job against `fake` and returns the events until the worker gives up, and the job.
fn sign_in_until_offline(fake: FakeImap) -> Result<(Run, Vec<EventKind>, JobId)> {
    let mut run = Run::with(fake, Options { connections: 4, ..Options::default() })?;
    let job = run.job(JobKind::SyncFolderList)?;
    let events = run.events_until(|event| event.job == Some(job))?.into_iter().map(|event| event.kind).collect();
    Ok((run, events, job))
}

fn halted(events: &[EventKind]) {
    let offline = events.iter().rposition(|kind| *kind == EventKind::Connection(ConnectionState::Offline));
    let backing_off = events.iter().rposition(|kind| matches!(kind, EventKind::Connection(ConnectionState::BackingOff { .. })));
    assert!(offline.is_some() && backing_off < offline, "{events:?}");
    assert_eq!(events.last(), Some(&EventKind::Failed { kind: ErrorKind::LoginUnanswered, folder: None }), "{events:?}");
}

#[test]
fn a_sign_in_dropped_without_an_answer_is_tried_once_more_then_halts() -> Result {
    let fake = at_every_sign_in(FakeImap::sample(), "AUTHENTICATE", &Action::Fault(Fault::Drop));
    let (mut run, events, _job) = sign_in_until_offline(fake)?;
    halted(&events);
    // Later jobs fail at once: the password is not sent again.
    run.job(sync_inbox(None)?)?;
    assert_eq!(run.until(|_| true)?, [EventKind::Failed { kind: ErrorKind::LoginUnanswered, folder: Some(INBOX) }]);
    assert_eq!(sent(run, "AUTHENTICATE"), 2);
    Ok(())
}

#[test]
fn a_bye_in_answer_to_the_sign_in_halts_at_once() -> Result {
    let fake = at_every_sign_in(FakeImap::sample(), "AUTHENTICATE", &Action::Bye("going away".to_owned()));
    let (run, events, _job) = sign_in_until_offline(fake)?;
    halted(&events);
    assert!(!events.iter().any(|kind| matches!(kind, EventKind::Connection(ConnectionState::BackingOff { .. }))), "{events:?}");
    assert_eq!(sent(run, "AUTHENTICATE"), 1);
    Ok(())
}

#[test]
fn garbage_in_answer_to_the_sign_in_halts_at_once() -> Result {
    let fake = at_every_sign_in(FakeImap::sample(), "AUTHENTICATE", &Action::Fault(Fault::Garbage(b"\x00\xff not imap\r\n".to_vec())));
    let (run, events, _job) = sign_in_until_offline(fake)?;
    halted(&events);
    assert_eq!(sent(run, "AUTHENTICATE"), 1);
    Ok(())
}

#[test]
fn a_login_dropped_without_an_answer_is_tried_once_more_then_halts() -> Result {
    let fake = at_every_sign_in(FakeImap::sample().without(Capability::AuthPlain), "LOGIN", &Action::Fault(Fault::Drop));
    let (run, events, _job) = sign_in_until_offline(fake)?;
    halted(&events);
    assert_eq!(sent(run, " LOGIN "), 2);
    Ok(())
}
