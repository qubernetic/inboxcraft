// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, FolderId, JobId, JobKind, MessageId};
use inboxcraft_store::Change;
use serde_json::{Value, json};

use super::Outcome;
use crate::test_support::{mailbox, session_with};
use crate::{AgentClass, Caller, CommandSpec, Context, EngineError, Registry};

fn removes_and_panics(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    panic!("a bug in a command")
}

fn removes_message_one(context: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    assert!(context.now() > inboxcraft_time::Timestamp::UNIX_EPOCH);
    Ok(Outcome::new(json!({ "removed": 1 }))
        .with_change(Change::RemoveMessage(MessageId::new(1)))
        .with_job(AccountId::new(1), JobKind::SyncFolderList)
        .with_job(AccountId::new(2), JobKind::SyncFolderList))
}

fn removes_then_fails(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    // The second change names a folder the store does not hold, so the batch fails as a whole.
    Ok(Outcome::new(json!({}))
        .with_change(Change::RemoveMessage(MessageId::new(1)))
        .with_change(Change::SetFlags(MessageId::new(999), inboxcraft_model::Flags::default()))
        .with_job(AccountId::new(1), JobKind::SyncFolderList))
}

fn test_registry() -> Registry {
    let mut registry = Registry::new();
    for (id, class, run) in [
        ("test.panics", AgentClass::Safe, removes_and_panics as crate::Run),
        ("test.removes", AgentClass::Safe, removes_message_one),
        ("test.fails", AgentClass::Safe, removes_then_fails),
        ("test.owner", AgentClass::Owner, removes_message_one),
        ("test.never", AgentClass::Never, removes_message_one),
    ] {
        registry.register(CommandSpec { id, class: Some(class), summary: "A test command", run }).unwrap();
    }
    registry
}

fn held(session: &super::Session) -> usize {
    session.store().messages(inboxcraft_model::FolderId::new(1)).unwrap().len()
}

#[test]
fn an_escaped_panic_becomes_an_error_and_changes_nothing() {
    let mut session = session_with(mailbox(), test_registry());
    assert_eq!(session.run("test.panics", &json!({})).unwrap_err(), EngineError::Panicked("test.panics"));
    assert_eq!(held(&session), 2);
    assert!(session.take_jobs().is_empty());
    // The session goes on working.
    assert_eq!(session.run("test.removes", &json!({})).unwrap(), json!({ "removed": 1 }));
    assert_eq!(held(&session), 1);
}

#[test]
fn an_outcome_is_made_after_the_command_returned() {
    let mut session = session_with(mailbox(), test_registry());
    session.run("test.removes", &json!(null)).unwrap();
    let jobs = session.take_jobs();
    assert_eq!(
        jobs.iter().map(|job| (job.id, job.account)).collect::<Vec<_>>(),
        [(JobId::new(1), AccountId::new(1)), (JobId::new(2), AccountId::new(2))]
    );
    assert!(session.take_jobs().is_empty(), "taken once");
    session.run("test.removes", &json!(null)).unwrap();
    assert_eq!(session.take_jobs().first().map(|job| job.id), Some(JobId::new(3)), "ids go on");
}

#[test]
fn a_failed_batch_changes_nothing_and_queues_no_job() {
    let mut session = session_with(mailbox(), test_registry());
    let error = session.run("test.fails", &json!({})).unwrap_err();
    assert_eq!(error.code(), "store");
    assert_eq!(held(&session), 2);
    assert!(session.take_jobs().is_empty());
}

#[test]
fn an_agent_runs_safe_commands_only() {
    let mut session = session_with(mailbox(), test_registry());
    assert_eq!(
        session.run_as(Caller::Agent, "test.owner", &json!({})).unwrap_err(),
        EngineError::NotAllowed { command: "test.owner", class: AgentClass::Owner }
    );
    assert_eq!(
        session.run_as(Caller::Agent, "test.never", &json!({})).unwrap_err(),
        EngineError::NotAllowed { command: "test.never", class: AgentClass::Never }
    );
    assert_eq!(held(&session), 2);
    session.run_as(Caller::Agent, "test.removes", &json!({})).unwrap();
    session.run_as(Caller::Owner, "test.owner", &json!({})).unwrap();
    session.run_as(Caller::Owner, "test.never", &json!({})).unwrap();
}

#[test]
fn an_unknown_or_unclassified_command_does_not_run() {
    let mut registry = test_registry();
    assert!(registry.register(CommandSpec { id: "test.unclassified", class: None, summary: "No class", run: removes_message_one }).is_err());
    let mut session = session_with(mailbox(), registry);
    assert_eq!(session.run("test.unclassified", &json!({})).unwrap_err().code(), "unknown_command");
    assert_eq!(session.run("mail.\r\nlost", &json!({})).unwrap_err(), EngineError::UnknownCommand("mail.lost".into()));
    assert_eq!(held(&session), 2);
}

#[test]
fn a_session_shows_its_commands_and_store() {
    let session = crate::test_support::session();
    assert!(session.registry().get("account.list").is_some());
    assert_eq!(session.store().accounts().unwrap().len(), 2);
    assert!(format!("{session:?}").starts_with("Session {"));
    assert_eq!(Outcome::new(json!(1)).value(), &json!(1));
}

#[test]
fn a_store_that_cannot_be_read_fails_the_session() {
    #[derive(Debug)]
    struct Broken;
    fn broken<T>() -> Result<T, inboxcraft_store::StoreError> {
        Err(inboxcraft_store::StoreError::Corrupt("broken"))
    }
    #[rustfmt::skip]
    impl inboxcraft_store::Storage for Broken {
        fn config(&self) -> inboxcraft_store::StoreConfig { inboxcraft_store::StoreConfig::default() }
        fn apply(&mut self, _: &inboxcraft_store::Batch) -> Result<(), inboxcraft_store::StoreError> { broken() }
        fn accounts(&self) -> Result<Vec<inboxcraft_model::Account>, inboxcraft_store::StoreError> { broken() }
        fn folders(&self, _: AccountId) -> Result<Vec<inboxcraft_model::Folder>, inboxcraft_store::StoreError> { broken() }
        fn folder_state(&self, _: FolderId) -> Result<Option<inboxcraft_model::FolderState>, inboxcraft_store::StoreError> { broken() }
        fn messages(&self, _: FolderId) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { broken() }
        fn message(&self, _: MessageId) -> Result<Option<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { broken() }
        fn body(&self, _: MessageId) -> Result<Option<inboxcraft_store::Body>, inboxcraft_store::StoreError> { broken() }
        fn search(&self, _: &inboxcraft_store::Search) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { broken() }
        fn queued_jobs(&self) -> Result<Vec<inboxcraft_model::Job>, inboxcraft_store::StoreError> { broken() }
        fn drafts(&self, _: AccountId) -> Result<Vec<inboxcraft_model::Draft>, inboxcraft_store::StoreError> { broken() }
        fn draft(&self, _: inboxcraft_model::DraftId) -> Result<Option<inboxcraft_model::Draft>, inboxcraft_store::StoreError> { broken() }
        fn outbox(&self) -> Result<Vec<inboxcraft_model::Outgoing>, inboxcraft_store::StoreError> { broken() }
        fn schedule(&self) -> Result<Vec<inboxcraft_model::Scheduled>, inboxcraft_store::StoreError> { broken() }
        fn sender(&self, _: &inboxcraft_model::EmailAddress) -> Result<Option<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { broken() }
        fn senders(&self) -> Result<Vec<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { broken() }
        fn message_state(&self, _: MessageId) -> Result<inboxcraft_model::MessageState, inboxcraft_store::StoreError> { broken() }
        fn message_states(&self) -> Result<Vec<(MessageId, inboxcraft_model::MessageState)>, inboxcraft_store::StoreError> { broken() }
        fn calendars(&self, _: AccountId) -> Result<Vec<inboxcraft_model::Calendar>, inboxcraft_store::StoreError> { broken() }
        fn sync_token(&self, _: CalendarId) -> Result<Option<String>, inboxcraft_store::StoreError> { broken() }
        fn calendar_events(&self, _: CalendarId) -> Result<Vec<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { broken() }
        fn calendar_event(&self, _: CalendarEventId) -> Result<Option<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { broken() }
        fn alerts(&self, _: CalendarEventId) -> Result<Vec<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { broken() }
        fn alert(&self, _: inboxcraft_model::AlertId) -> Result<Option<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { broken() }
        fn stale_bodies(&self, _: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { broken() }
        fn missing_bodies(&self, _: FolderId, _: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { broken() }
    }
    use inboxcraft_store::Storage as _;
    let (folder, message, now) = (FolderId::new(1), MessageId::new(1), inboxcraft_time::Timestamp::UNIX_EPOCH);
    let (account, calendar, event) = (AccountId::new(1), CalendarId::new(1), CalendarEventId::new(1));
    let mut broken = Broken;
    assert_eq!(broken.config(), inboxcraft_store::StoreConfig::default());
    assert!(broken.apply(&inboxcraft_store::Batch::new()).is_err());
    assert!(broken.folders(account).is_err() && broken.folder_state(folder).is_err() && broken.messages(folder).is_err());
    assert!(broken.message(message).is_err() && broken.body(message).is_err() && broken.search(&inboxcraft_store::Search::new()).is_err());
    assert!(broken.stale_bodies(now).is_err() && broken.missing_bodies(folder, now).is_err());
    assert!(broken.drafts(account).is_err() && broken.draft(inboxcraft_model::DraftId::new(1)).is_err() && broken.outbox().is_err());
    assert!(broken.schedule().is_err() && broken.senders().is_err());
    assert!(broken.sender(&inboxcraft_model::EmailAddress::new("anna@example.org").unwrap()).is_err());
    assert!(broken.message_state(message).is_err() && broken.message_states().is_err() && broken.calendars(account).is_err());
    assert!(broken.sync_token(calendar).is_err() && broken.calendar_events(calendar).is_err() && broken.calendar_event(event).is_err());
    assert!(broken.alerts(event).is_err() && broken.queued_jobs().is_err());
    let error = super::Session::new(Box::new(Broken), Box::new(crate::test_support::clock())).unwrap_err();
    assert_eq!(error, EngineError::Store(inboxcraft_store::StoreError::Corrupt("broken")));
}
