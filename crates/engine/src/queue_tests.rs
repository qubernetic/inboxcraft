// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Kept jobs: queued in the store with the changes of their command, replayed after a restart, gone when they end.

use inboxcraft_model::{AccountId, ConnectionState, ErrorKind, Event, EventKind, FolderId, Job, JobId, JobKind, MessageId};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use serde_json::{Value, json};

use super::{Applied, Outcome};
use crate::test_support::{mailbox, session_with};
use crate::{AgentClass, CommandSpec, Context, EngineError, Registry};

fn keeps(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Ok(Outcome::new(json!({}))
        .with_change(Change::RemoveMessage(MessageId::new(1)))
        .with_job(AccountId::new(2), JobKind::SyncFolderList)
        .with_kept_job(AccountId::new(1), JobKind::Check { by_owner: false }))
}

fn keeps_then_fails(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Ok(Outcome::new(json!({}))
        .with_kept_job(AccountId::new(1), JobKind::SyncFolderList)
        .with_change(Change::SetFlags(MessageId::new(999), inboxcraft_model::Flags::default())))
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    for (id, run) in [("test.keeps", keeps as crate::Run), ("test.keeps_then_fails", keeps_then_fails)] {
        registry.register(CommandSpec { id, class: Some(AgentClass::Safe), summary: "A test command", run }).unwrap();
    }
    registry
}

fn queued(store: &dyn Storage) -> Vec<Job> {
    store.queued_jobs().unwrap()
}

/// The mailbox with job 7 of account 1 left in the queue by an earlier run.
fn left_over() -> Memory {
    let mut store = mailbox();
    store.apply(&Batch::new().with(Change::QueueJob(Job::new(JobId::new(7), AccountId::new(1), JobKind::SyncFolderList)))).unwrap();
    store
}

fn ended(job: u64) -> Event {
    Event::new(AccountId::new(1), Some(JobId::new(job)), EventKind::Failed { kind: ErrorKind::ConnectionRefused, folder: None })
}

#[test]
fn a_kept_job_goes_into_the_store_with_its_commands_changes() {
    let mut session = session_with(mailbox(), registry());
    session.run("test.keeps", &json!({})).unwrap();
    let jobs = session.take_jobs();
    assert_eq!(
        jobs.iter().map(|job| (job.id.get(), job.account.get())).collect::<Vec<_>>(),
        [(2, 2), (1, 1)],
        "in the command's order; a kept job has its id before the batch is made"
    );
    assert_eq!(queued(session.store()), [jobs[1].clone()]);
    assert_eq!(session.store().message(MessageId::new(1)).unwrap(), None);
}

#[test]
fn a_failed_batch_keeps_no_job() {
    let mut session = session_with(mailbox(), registry());
    assert!(session.run("test.keeps_then_fails", &json!({})).is_err());
    assert!(session.take_jobs().is_empty());
    assert!(queued(session.store()).is_empty());
    session.run("test.keeps", &json!({})).unwrap();
    let ids: Vec<u64> = session.take_jobs().iter().map(|job| job.id.get()).collect();
    assert_eq!(ids, [2, 1], "no id was spent on the failed batch");
}

#[test]
fn a_restart_replays_the_kept_jobs_first() {
    let mut session = session_with(left_over(), Registry::builtin().unwrap());
    session.start();
    let jobs = session.take_jobs();
    let shown: Vec<(u64, u64)> = jobs.iter().map(|job| (job.id.get(), job.account.get())).collect();
    assert_eq!(shown, [(7, 1), (8, 1), (9, 2)], "the left-over job, then the folder lists with ids past it");
    session.start();
    assert_eq!(session.take_jobs().len(), 2, "replayed once");
    assert_eq!(queued(session.store()).len(), 1, "kept until it ends");
}

#[test]
fn a_job_leaves_the_store_when_it_ends() {
    let mut session = session_with(left_over(), registry());
    let connection = Event::new(AccountId::new(1), Some(JobId::new(7)), EventKind::Connection(ConnectionState::Online));
    session.apply(&connection).unwrap();
    session.apply(&ended(8)).unwrap();
    assert_eq!(queued(session.store()).len(), 1, "not ended, or not a kept job");
    assert_eq!(session.apply(&ended(7)).unwrap(), Applied::Changed);
    assert!(queued(session.store()).is_empty());
}

#[test]
fn a_job_leaves_the_store_also_when_its_event_is_dropped() {
    let mut session = session_with(left_over(), registry());
    let gone = Event::new(
        AccountId::new(1),
        Some(JobId::new(7)),
        EventKind::FolderSynced {
            folder: FolderId::new(99),
            state: inboxcraft_model::FolderState::new(inboxcraft_model::UidValidity::new(1).unwrap()),
        },
    );
    assert_eq!(session.apply(&gone).unwrap(), Applied::Dropped);
    assert!(queued(session.store()).is_empty());
}

#[test]
fn every_last_event_of_a_job_ends_it() {
    use super::ends_job;
    let validity = inboxcraft_model::UidValidity::new(1).unwrap();
    let folder = FolderId::new(1);
    assert!(ends_job(&EventKind::Folders { listings: vec![] }));
    assert!(ends_job(&EventKind::FolderSynced { folder, state: inboxcraft_model::FolderState::new(validity) }));
    assert!(ends_job(&EventKind::BodyFetched { folder, uid_validity: validity, uid: inboxcraft_model::Uid::new(1).unwrap(), raw: vec![] }));
    assert!(ends_job(&ended(1).kind));
    assert!(!ends_job(&EventKind::Connection(ConnectionState::Online)));
    assert!(!ends_job(&EventKind::MessagesAdded { folder, uid_validity: validity, messages: vec![] }));
}
