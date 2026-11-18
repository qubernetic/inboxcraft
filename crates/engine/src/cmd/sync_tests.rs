// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `sync.status`: the state, the progress and the last sync, kept from the workers' events, and every hostile event
//! dropped or bounded.

use inboxcraft_model::{AccountId, ConnectionState, ErrorKind, Event, EventKind, FolderId, JobId, JobKind};
use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
use serde_json::{Value, json};

use crate::test_support::{NOW, account, at, clock, mailbox};
use crate::{AgentClass, Applied, Caller, EngineError, Session};

/// The two accounts of the test mailbox, 1 and 2.
fn session() -> Session {
    Session::new(Box::new(mailbox()), Box::new(clock())).unwrap()
}

fn tell(session: &mut Session, account: u64, kind: EventKind) -> Applied {
    session.apply(&Event::new(AccountId::new(account), None, kind)).unwrap()
}

fn connection(session: &mut Session, account: u64, state: ConnectionState) {
    assert_eq!(tell(session, account, EventKind::Connection(state)), Applied::Changed);
}

fn progress(session: &mut Session, account: u64, done: u32, of: u32) -> Applied {
    tell(session, account, EventKind::SyncProgress { done, of })
}

fn finished(session: &mut Session, account: u64, seconds: i64) -> Applied {
    tell(session, account, EventKind::SyncFinished { at: at(seconds) })
}

fn failed(session: &mut Session, account: u64) {
    failed_with(session, account, ErrorKind::TimedOut);
}

fn failed_with(session: &mut Session, account: u64, kind: ErrorKind) {
    assert_eq!(tell(session, account, EventKind::Failed { kind, folder: None }), Applied::Changed);
}

fn status(session: &mut Session) -> Value {
    session.run_as(Caller::Agent, "sync.status", &json!({})).unwrap()
}

fn online(session: &mut Session) {
    connection(session, 1, ConnectionState::Online);
    connection(session, 2, ConnectionState::Watching);
}

/// Both accounts signed in, each with a round that ended a day ago (`2026-11-04T09:00:00Z`).
fn current(session: &mut Session) {
    online(session);
    assert_eq!(finished(session, 1, NOW - 86_400), Applied::Changed);
    assert_eq!(finished(session, 2, NOW - 86_400), Applied::Changed);
}

/// A session with the one account 1, nothing told yet.
fn single() -> Session {
    let mut store = Memory::new(StoreConfig::default());
    store.apply(&Batch::new().with(Change::PutAccount(account(1, "anna@example.org")))).unwrap();
    Session::new(Box::new(store), Box::new(clock())).unwrap()
}

fn state(session: &mut Session) -> Value {
    status(session)["state"].clone()
}

#[test]
fn the_status_is_safe_and_takes_no_parameters() {
    let mut session = session();
    assert_eq!(session.registry().get("sync.status").map(|spec| spec.class), Some(Some(AgentClass::Safe)));
    assert!(matches!(session.run("sync.status", &json!({ "account": 1 })), Err(EngineError::BadParams { .. })));
    assert!(matches!(session.run("sync.status", &json!([])), Err(EngineError::BadParams { .. })));
}

#[test]
fn with_no_account_it_is_synced_with_zeros_and_no_time() {
    let mut session = Session::new(Box::new(Memory::new(StoreConfig::default())), Box::new(clock())).unwrap();
    assert_eq!(status(&mut session), json!({ "state": "synced", "done": 0, "of": 0, "last_synced": null, "failing_accounts": 0 }));
}

#[test]
fn accounts_with_no_connection_are_offline_and_offline_wins_over_failing() {
    let mut session = session();
    assert_eq!(status(&mut session)["state"], "offline", "no connection told yet");
    connection(&mut session, 1, ConnectionState::Connecting);
    assert_eq!(status(&mut session)["state"], "offline", "an account on its way is not signed in yet");
    failed(&mut session, 1);
    connection(&mut session, 1, ConnectionState::BackingOff { attempt: 1, retry_in_secs: 2 });
    connection(&mut session, 2, ConnectionState::Offline);
    assert_eq!(status(&mut session), json!({ "state": "offline", "done": 0, "of": 0, "last_synced": null, "failing_accounts": 1 }));
    connection(&mut session, 2, ConnectionState::Online);
    assert_eq!(status(&mut session)["state"], "failing", "one account connected and one failing");
}

#[test]
fn the_worst_state_wins_through_a_sequence_of_events() {
    let mut session = session();
    online(&mut session);
    assert_eq!(status(&mut session)["state"], "syncing", "signed in and no round ended yet");
    finished(&mut session, 1, NOW - 600);
    finished(&mut session, 2, NOW - 600);
    assert_eq!(status(&mut session)["state"], "synced");
    assert_eq!(progress(&mut session, 1, 1, 4), Applied::Changed);
    assert_eq!(status(&mut session), json!({ "state": "syncing", "done": 1, "of": 4, "last_synced": "2026-11-05T08:50:00Z", "failing_accounts": 0 }));
    failed(&mut session, 2);
    assert_eq!(status(&mut session)["state"], "failing", "failing wins over syncing");
    connection(&mut session, 2, ConnectionState::Online);
    assert_eq!(status(&mut session)["state"], "syncing", "online again ends the failure");
    progress(&mut session, 1, 4, 4);
    assert_eq!(status(&mut session)["state"], "synced", "a round with nothing left is not syncing");
    progress(&mut session, 1, 0, 2);
    assert_eq!(finished(&mut session, 1, NOW - 60), Applied::Changed);
    assert_eq!(status(&mut session), json!({ "state": "synced", "done": 0, "of": 0, "last_synced": "2026-11-05T08:59:00Z", "failing_accounts": 0 }));
}

#[test]
fn the_end_of_a_round_ends_the_failure_too() {
    let mut session = session();
    current(&mut session);
    failed(&mut session, 1);
    assert_eq!(status(&mut session)["failing_accounts"], 1);
    finished(&mut session, 1, NOW);
    assert_eq!(status(&mut session)["state"], "synced");
    assert_eq!(status(&mut session)["failing_accounts"], 0);
}

#[test]
fn failing_accounts_counts_accounts_not_failures() {
    let mut session = session();
    online(&mut session);
    for _ in 0..3 {
        failed(&mut session, 1);
    }
    assert_eq!(status(&mut session)["failing_accounts"], 1);
    failed(&mut session, 2);
    assert_eq!(status(&mut session)["failing_accounts"], 2);
}

#[test]
fn done_above_of_is_taken_as_of() {
    let mut session = session();
    online(&mut session);
    progress(&mut session, 1, 9, 4);
    progress(&mut session, 2, 1, 3);
    let answer = status(&mut session);
    assert_eq!((answer["done"].clone(), answer["of"].clone()), (json!(5), json!(7)));
}

#[test]
fn the_sums_saturate_at_the_largest_count() {
    let mut session = session();
    online(&mut session);
    progress(&mut session, 1, u32::MAX - 1, u32::MAX);
    progress(&mut session, 2, u32::MAX, u32::MAX);
    let answer = status(&mut session);
    assert_eq!((answer["state"].clone(), answer["done"].clone(), answer["of"].clone()), (json!("syncing"), json!(u32::MAX), json!(u32::MAX)));
}

#[test]
fn an_event_for_an_unknown_account_is_dropped() {
    let mut session = session();
    current(&mut session);
    assert_eq!(progress(&mut session, 9, 1, 4), Applied::Dropped);
    assert_eq!(finished(&mut session, 9, NOW), Applied::Dropped);
    assert_eq!(status(&mut session), json!({ "state": "synced", "done": 0, "of": 0, "last_synced": "2026-11-04T09:00:00Z", "failing_accounts": 0 }));
}

#[test]
fn a_removed_account_takes_its_progress_and_its_time_with_it() {
    let mut session = session();
    current(&mut session);
    progress(&mut session, 2, 1, 5);
    finished(&mut session, 2, NOW);
    progress(&mut session, 2, 1, 5);
    failed(&mut session, 2);
    session.run("account.remove", &json!({ "account": 2 })).unwrap();
    assert_eq!(status(&mut session), json!({ "state": "synced", "done": 0, "of": 0, "last_synced": "2026-11-04T09:00:00Z", "failing_accounts": 0 }));
    assert_eq!(progress(&mut session, 2, 1, 5), Applied::Dropped, "a late event of its worker");
}

#[test]
fn repeated_progress_replaces_and_never_adds_up() {
    let mut session = session();
    online(&mut session);
    for n in 0..1_000 {
        progress(&mut session, 1, n, 1_000);
    }
    progress(&mut session, 1, 2, 3);
    let answer = status(&mut session);
    assert_eq!((answer["done"].clone(), answer["of"].clone()), (json!(2), json!(3)));
}

#[test]
fn last_synced_is_the_newest_end_over_every_account() {
    let mut session = session();
    online(&mut session);
    finished(&mut session, 1, NOW - 60);
    finished(&mut session, 2, NOW - 3_600);
    finished(&mut session, 1, NOW - 7_200);
    assert_eq!(status(&mut session)["last_synced"], "2026-11-05T08:59:00Z");
}

#[test]
fn an_end_far_in_the_future_is_junk_and_dropped() {
    let mut session = session();
    online(&mut session);
    progress(&mut session, 1, 1, 4);
    let far = NOW + i64::try_from(super::sync::FUTURE_SLACK.as_secs()).unwrap() + 1;
    assert_eq!(finished(&mut session, 1, far), Applied::Dropped);
    let end = Event::new(AccountId::new(1), None, EventKind::SyncFinished { at: inboxcraft_time::Timestamp::MAX });
    assert_eq!(session.apply(&end).unwrap(), Applied::Dropped);
    assert_eq!(status(&mut session), json!({ "state": "syncing", "done": 1, "of": 4, "last_synced": null, "failing_accounts": 0 }));
    // A worker's clock a little ahead of the engine's is no junk.
    assert_eq!(finished(&mut session, 1, NOW + 60), Applied::Changed);
    assert_eq!(status(&mut session)["last_synced"], "2026-11-05T09:01:00Z");
}

#[test]
fn an_account_retrying_in_its_back_off_reads_offline_at_every_attempt() {
    let mut session = single();
    for attempt in 1..=3 {
        connection(&mut session, 1, ConnectionState::Connecting);
        assert_eq!(state(&mut session), "offline", "attempt {attempt}, connecting");
        failed(&mut session, 1);
        assert_eq!(state(&mut session), "offline", "attempt {attempt}, failed");
        connection(&mut session, 1, ConnectionState::BackingOff { attempt, retry_in_secs: 2 });
        assert_eq!(state(&mut session), "offline", "attempt {attempt}, backing off");
    }
}

#[test]
fn the_first_sign_in_reads_offline_then_syncing_until_a_round_ends() {
    let mut session = single();
    assert_eq!(state(&mut session), "offline");
    connection(&mut session, 1, ConnectionState::Connecting);
    assert_eq!(state(&mut session), "offline");
    connection(&mut session, 1, ConnectionState::Online);
    assert_eq!(state(&mut session), "syncing", "signed in, before the worker tells its round");
    progress(&mut session, 1, 0, 2);
    assert_eq!(state(&mut session), "syncing");
    connection(&mut session, 1, ConnectionState::Watching);
    assert_eq!(state(&mut session), "syncing");
    finished(&mut session, 1, NOW);
    assert_eq!(state(&mut session), "synced");
}

#[test]
fn a_refused_sign_in_reads_failing_with_no_account_connected() {
    let mut session = single();
    connection(&mut session, 1, ConnectionState::Connecting);
    failed_with(&mut session, 1, ErrorKind::LoginRefused);
    connection(&mut session, 1, ConnectionState::Offline);
    assert_eq!(status(&mut session), json!({ "state": "failing", "done": 0, "of": 0, "last_synced": null, "failing_accounts": 1 }));
    let mut session = single();
    failed_with(&mut session, 1, ErrorKind::CredentialMissing);
    connection(&mut session, 1, ConnectionState::Offline);
    assert_eq!(state(&mut session), "failing", "no password");
}

#[test]
fn a_server_out_of_reach_reads_offline() {
    for kind in [ErrorKind::HostNotFound, ErrorKind::ConnectionRefused, ErrorKind::TimedOut, ErrorKind::ConnectionDropped] {
        let mut session = single();
        connection(&mut session, 1, ConnectionState::Connecting);
        failed_with(&mut session, 1, kind);
        connection(&mut session, 1, ConnectionState::Offline);
        assert_eq!(state(&mut session), "offline", "{kind:?}");
    }
}

#[test]
fn a_body_fetch_that_failed_does_not_make_the_account_failing() {
    let mut session = super::act_tests::session();
    current(&mut session);
    session.run("mail.read", &json!({ "id": 1 })).unwrap();
    let fetch = session.take_jobs().into_iter().find(|job| matches!(job.kind, JobKind::FetchBody { .. })).unwrap();
    let gone = EventKind::Failed { kind: ErrorKind::MessageGone, folder: Some(FolderId::new(1)) };
    assert_eq!(session.apply(&Event::new(AccountId::new(1), Some(fetch.id), gone.clone())).unwrap(), Applied::Changed);
    assert_eq!(status(&mut session), json!({ "state": "synced", "done": 0, "of": 0, "last_synced": "2026-11-04T09:00:00Z", "failing_accounts": 0 }));
    // Another job's failure still counts.
    session.apply(&Event::new(AccountId::new(1), Some(JobId::new(999)), gone)).unwrap();
    assert_eq!(status(&mut session)["failing_accounts"], 1);
}
