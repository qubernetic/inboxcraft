// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendars' sync followed by its jobs: a press noted once, the jobs of the accounts asked followed until the
//! last comes back, the first failure kept, the time of the last read, and the caps on names and jobs.

use inboxcraft_model::{AccountId, CalendarId, CalendarJob, CalendarReport, ErrorKind, Event, EventKind, Job, JobId, JobKind};
use inboxcraft_time::{Offset, Timestamp};

use super::{MOST_JOBS, MOST_NAME, SyncState, Tracker, Why};
use crate::calendar::{Day, Moment};

/// 2026-11-05 08:20 UTC, and `minutes` after.
fn at(minutes: i64) -> Timestamp {
    Timestamp::from_unix(1_793_866_800 + minutes * 60, 0).unwrap()
}

fn utc() -> Offset {
    Offset::UTC
}

fn discover(id: u64, account: u64) -> Job {
    Job::new(JobId::new(id), AccountId::new(account), JobKind::Calendar(CalendarJob::Discover { url: None }))
}

fn sync_job(id: u64, account: u64) -> Job {
    let kind = CalendarJob::Sync { calendar: CalendarId::new(id), url: format!("https://dav.example.org/c/{id}/"), token: None, known: Vec::new() };
    Job::new(JobId::new(id), AccountId::new(account), JobKind::Calendar(kind))
}

fn found(job: u64, account: u64) -> Event {
    Event::new(
        AccountId::new(account),
        Some(JobId::new(job)),
        EventKind::Calendar(CalendarReport::Found { home: String::new(), calendars: Vec::new() }),
    )
}

fn synced(job: u64, account: u64) -> Event {
    let report = CalendarReport::Synced { calendar: CalendarId::new(job), changed: Vec::new(), removed: Vec::new(), token: None, full: true };
    Event::new(AccountId::new(account), Some(JobId::new(job)), EventKind::Calendar(report))
}

fn failed(job: u64, account: u64, kind: ErrorKind) -> Event {
    Event::new(AccountId::new(account), Some(JobId::new(job)), EventKind::Calendar(CalendarReport::Failed { calendar: None, kind }))
}

fn moment(minutes: i64) -> Option<Moment> {
    super::super::moment(at(minutes), utc())
}

#[test]
fn a_press_is_noted_once_and_none_while_a_sync_is_under_way() {
    let mut tracker = Tracker::default();
    assert!(tracker.ask());
    assert!(!tracker.ask(), "a second press before the command ran asks for nothing more");
    assert!(tracker.take_asked());
    assert!(!tracker.take_asked());
    tracker.start(&[(1, "Personal")]);
    assert!(!tracker.ask(), "a press while the sync is under way does nothing");
    assert!(!tracker.is_asked());
    assert_eq!(tracker.started(), 1);
}

#[test]
fn a_sync_runs_until_every_job_of_its_accounts_comes_back() {
    let mut tracker = Tracker::default();
    tracker.start(&[(1, "Personal"), (2, "Studio")]);
    // Account 3 was not asked: its jobs are not the press's.
    tracker.sent(&[discover(10, 1), discover(11, 2), discover(12, 3)], &[1, 2, 3], at(0), utc());
    assert_eq!(tracker.view().state, SyncState::Syncing { done: 0, of: 2, account: "Personal".to_owned() });
    assert!(tracker.heard(&found(10, 1), at(1), utc()));
    assert!(!tracker.heard(&found(12, 3), at(1), utc()), "another's job is not counted");
    tracker.sent(&[sync_job(20, 1), sync_job(21, 1)], &[1, 2, 3], at(1), utc());
    assert!(matches!(tracker.view().state, SyncState::Syncing { done: 1, of: 4, .. }), "{:?}", tracker.view());
    for (job, account) in [(11, 2), (20, 1)] {
        tracker.heard(&if job == 11 { found(job, account) } else { synced(job, account) }, at(2), utc());
        tracker.sent(&[], &[1, 2, 3], at(2), utc());
        assert!(matches!(tracker.view().state, SyncState::Syncing { .. }));
    }
    tracker.heard(&synced(21, 1), at(3), utc());
    assert!(tracker.sent(&[], &[1, 2, 3], at(3), utc()));
    assert_eq!(tracker.view().state, SyncState::Synced { at: moment(3) });
    assert_eq!(tracker.view().read, moment(3));
    assert!(tracker.ask(), "once it ended the owner can sync again");
}

#[test]
fn the_first_failure_is_kept_and_named_after_its_account() {
    let mut tracker = Tracker::default();
    tracker.start(&[(1, "Personal"), (2, "Studio")]);
    tracker.sent(&[discover(10, 1), discover(11, 2)], &[1, 2], at(0), utc());
    tracker.heard(&failed(11, 2, ErrorKind::LoginRefused), at(1), utc());
    tracker.heard(&failed(10, 1, ErrorKind::HostNotFound), at(1), utc());
    tracker.sent(&[], &[1, 2], at(1), utc());
    assert_eq!(tracker.view().state, SyncState::Failed { account: Some("Studio".to_owned()), why: Why::SignIn });
    assert_eq!(tracker.view().read, None, "nothing was read");
    assert!(tracker.ask(), "a failure offers the press again");
}

#[test]
fn a_connection_failure_of_a_job_followed_ends_it_too() {
    let mut tracker = Tracker::default();
    tracker.start(&[(1, "Personal")]);
    tracker.sent(&[discover(10, 1)], &[1], at(0), utc());
    let event = Event::new(AccountId::new(1), Some(JobId::new(10)), EventKind::Failed { kind: ErrorKind::TimedOut, folder: None });
    assert!(tracker.heard(&event, at(1), utc()));
    tracker.sent(&[], &[1], at(1), utc());
    assert_eq!(tracker.view().state, SyncState::Failed { account: Some("Personal".to_owned()), why: Why::NoConnection });
}

#[test]
fn without_an_account_to_sync_it_says_there_is_no_address() {
    let mut tracker = Tracker::default();
    tracker.start(&[]);
    assert_eq!(tracker.view().state, SyncState::Failed { account: None, why: Why::NoAddress });
    let mut refused = Tracker::default();
    refused.not_started();
    assert_eq!(refused.view().state, SyncState::Failed { account: None, why: Why::NotStarted });
}

#[test]
fn a_job_of_an_account_that_went_away_is_not_waited_for() {
    let mut tracker = Tracker::default();
    tracker.start(&[(1, "Personal"), (2, "Studio")]);
    tracker.sent(&[discover(10, 1), discover(11, 2)], &[1, 2], at(0), utc());
    tracker.heard(&found(10, 1), at(1), utc());
    tracker.sent(&[], &[1], at(1), utc());
    assert_eq!(tracker.view().state, SyncState::Synced { at: moment(1) });
}

#[test]
fn a_read_of_anyone_s_sync_moves_the_time_and_nothing_else() {
    let mut tracker = Tracker::default();
    assert!(!tracker.heard(&synced(5, 1), at(7), utc()), "no sync of the owner's is under way");
    assert_eq!(tracker.view().state, SyncState::Idle);
    assert_eq!(tracker.view().read, moment(7));
    assert_eq!(tracker.view().read.map(Moment::clock).as_deref(), Some("08:27"));
    assert_eq!(moment(7).map(|moment| moment.day), Day::new(2026, 11, 5));
}

#[test]
fn hostile_names_are_one_capped_line_and_jobs_are_capped() {
    let long = format!("Stu\u{202E}dio\r\n\0{}", "x".repeat(10_000));
    let mut tracker = Tracker::default();
    tracker.start(&[(1, &long)]);
    let jobs: Vec<Job> = (0..u64::try_from(MOST_JOBS).unwrap() + 10).map(|id| sync_job(id, 1)).collect();
    tracker.sent(&jobs, &[1], at(0), utc());
    let SyncState::Syncing { of, account, .. } = tracker.view().state else { panic!("{:?}", tracker.view()) };
    assert_eq!(of, MOST_JOBS);
    assert!(account.chars().count() <= MOST_NAME, "{}", account.chars().count());
    assert!(!account.chars().any(|c| c.is_control() || c == '\u{202E}'), "{account:?}");
    assert!(account.starts_with("Studio"));
}

#[test]
fn every_failure_has_words_of_the_window() {
    for kind in [ErrorKind::LoginRefused, ErrorKind::CredentialMissing, ErrorKind::NoCalendarServer, ErrorKind::Tls, ErrorKind::MessageGone] {
        assert!(!Why::of(kind).words().is_empty());
    }
    assert_eq!(Why::of(ErrorKind::CommandRefused).words(), "refused");
}
