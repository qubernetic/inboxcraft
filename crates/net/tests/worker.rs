// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The account worker against the fake IMAP server: the folder list, the first sync of a folder with headers,
//! previews and the bodies of the last days, a body on demand, and each way a job can fail.

mod support;

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, ConnectionState, EmailAddress, ErrorKind, Event, EventKind, FolderId, FolderPath, FolderState, Job, JobId, JobKind, Security,
    SpecialUse, Summary, Uid, UidValidity,
};
use inboxcraft_net::{Credentials, Password, SyncConfig, Worker, WorkerSettings};
use inboxcraft_testkit::imap::{Action, Capability, FakeImap, Mailbox, Message, PASSWORD, USER, When};
use inboxcraft_testkit::{FakeServer, Fault, Listen, TestPki};
use support::{Result, connector, days_ago, server, wall};

const ACCOUNT: AccountId = AccountId::new(1);
const INBOX: FolderId = FolderId::new(10);
const WAIT: Duration = Duration::from_secs(10);

/// A fixed password, or none.
struct Fixed(Option<&'static str>);

impl Credentials for Fixed {
    fn password(&self, account: &Account) -> Option<Password> {
        assert_eq!(account.id(), ACCOUNT);
        self.0.map(|text| Password::new(text.to_owned()))
    }
}

struct Run {
    worker: Worker,
    events: Receiver<Event>,
    fake: FakeServer,
    next: u64,
}

impl Run {
    fn start(fake: FakeImap, password: Option<&'static str>, sync: SyncConfig) -> Result<Self> {
        let pki = TestPki::new()?;
        let fake = Listen::tls(pki.server_config(None)?).accept_timeout(Duration::from_secs(2)).start_imap(fake)?;
        let account = Account::new(ACCOUNT, "Test", EmailAddress::new(USER)?, USER, server(Security::Tls)?)?;
        let settings = WorkerSettings { connector: connector(&pki, wall(), fake.addr()), sync };
        let (sender, events) = mpsc::channel();
        let worker = Worker::spawn(account, settings, Arc::new(Fixed(password)), sender)?;
        Ok(Self { worker, events, fake, next: 0 })
    }

    fn sample() -> Result<Self> {
        Self::start(FakeImap::sample(), Some(PASSWORD), SyncConfig::default())
    }

    /// Sends a job; its id.
    fn job(&mut self, kind: JobKind) -> Result<JobId> {
        self.next += 1;
        let id = JobId::new(self.next);
        self.worker.send(Job::new(id, ACCOUNT, kind)).map_err(|_| "job refused")?;
        Ok(id)
    }

    /// The events up to and with the first one `last` takes.
    fn until(&self, last: impl Fn(&EventKind) -> bool) -> Result<Vec<EventKind>> {
        let mut seen = Vec::new();
        loop {
            let event = self.events.recv_timeout(WAIT)?;
            assert_eq!(event.account, ACCOUNT);
            let done = last(&event.kind);
            seen.push(event.kind);
            if done {
                return Ok(seen);
            }
        }
    }

    /// The events of a folder sync, up to its end or its failure.
    fn synced(&self) -> Result<Vec<EventKind>> {
        self.until(|kind| matches!(kind, EventKind::FolderSynced { .. } | EventKind::Failed { .. }))
    }

    /// Stops the worker and returns the lines the server got.
    fn finish(self) -> Result<Vec<String>> {
        self.worker.stop();
        Ok(self.fake.finish()?.received().to_vec())
    }
}

fn path(name: &str) -> Result<FolderPath> {
    Ok(FolderPath::new(name, Some('/'))?)
}

fn sync_inbox(known: Option<FolderState>) -> Result<JobKind> {
    Ok(JobKind::SyncFolder { folder: INBOX, path: path("INBOX")?, known })
}

fn added(events: &[EventKind]) -> Vec<Summary> {
    events
        .iter()
        .filter_map(|kind| if let EventKind::MessagesAdded { messages, .. } = kind { Some(messages.clone()) } else { None })
        .flatten()
        .collect()
}

fn bodies(events: &[EventKind]) -> Vec<(u32, Vec<u8>)> {
    events.iter().filter_map(|kind| if let EventKind::BodyFetched { uid, raw, .. } = kind { Some((uid.get(), raw.clone())) } else { None }).collect()
}

fn uid(value: u32) -> Result<Uid> {
    Ok(Uid::new(value).ok_or("uid")?)
}

fn validity(value: u32) -> Result<UidValidity> {
    Ok(UidValidity::new(value).ok_or("uidvalidity")?)
}

/// INBOX with a message from two days ago, one from forty days ago and one from yesterday.
fn recent_and_old() -> Result<FakeImap> {
    let inbox = Mailbox::new("INBOX")
        .message(Message::new("From: Anna <anna@example.org>\nSubject: Recent\n\nNew text.\n").date(&days_ago(2)?).preview("Server preview"))
        .message(Message::new("From: bob@example.org\nSubject: Old\n\nOld text.\n").date(&days_ago(40)?).flags(&["\\Seen"]))
        .message(Message::new("From: carol@example.org\nSubject: Yesterday\n\nAnother.\n").date(&days_ago(1)?));
    Ok(FakeImap::new().mailbox(inbox))
}

#[test]
fn the_folder_list_comes_with_names_and_special_use() -> Result {
    let mut run = Run::sample()?;
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Folders { .. } | EventKind::Failed { .. }))?;
    assert_eq!(events[..2], [EventKind::Connection(ConnectionState::Connecting), EventKind::Connection(ConnectionState::Online)]);
    let Some(EventKind::Folders { listings }) = events.last() else { return Err("no folder list".into()) };
    let names: Vec<_> = listings.iter().map(|listing| listing.path().name().to_owned()).collect();
    assert!(names.contains(&"INBOX".to_owned()) && names.contains(&"Elküldött".to_owned()) && names.contains(&"Lists/Rust".to_owned()), "{names:?}");
    let sent = listings.iter().find(|listing| listing.path().name() == "Sent").ok_or("no Sent")?;
    assert_eq!(sent.special_use(), [SpecialUse::Sent]);
    let lines = run.finish()?;
    assert!(lines.iter().any(|line| line.contains("LIST")), "{lines:?}");
    Ok(())
}

#[test]
fn the_first_sync_brings_headers_previews_and_the_bodies_of_the_last_days() -> Result {
    let mut run = Run::start(recent_and_old()?, Some(PASSWORD), SyncConfig::default())?;
    run.job(sync_inbox(None)?)?;
    let events = run.synced()?;
    let summaries = added(&events);
    let subjects: Vec<_> = summaries.iter().map(|summary| summary.envelope().subject().to_owned()).collect();
    assert_eq!(subjects, ["Recent", "Old", "Yesterday"]);
    assert_eq!(summaries[0].preview(), Some("Server preview"));
    assert_eq!(summaries[2].preview(), Some("Another."));
    assert_eq!(summaries[0].envelope().from()[0].name(), Some("Anna"));
    assert!(summaries[1].flags().is_seen());
    assert!(summaries.iter().all(|summary| summary.received().is_some() && summary.size() > 0));
    let fetched = bodies(&events);
    assert_eq!(fetched.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [1, 3]);
    assert!(fetched[0].1.starts_with(b"From: Anna <anna@example.org>\r\nSubject: Recent"));
    let Some(EventKind::FolderSynced { folder, state }) = events.last() else { return Err("not synced".into()) };
    assert_eq!(*folder, INBOX);
    assert_eq!(state.uid_validity(), validity(1)?);
    assert_eq!(state.uid_next(), Some(uid(4)?));
    let lines = run.finish()?;
    // Read-only: EXAMINE and BODY.PEEK, nothing that sets a flag.
    assert!(lines.iter().any(|line| line.contains("EXAMINE \"INBOX\"") || line.contains("EXAMINE INBOX")), "{lines:?}");
    let verbs: Vec<String> = lines.iter().map(|line| line.split(' ').skip(1).take(2).collect::<Vec<_>>().join(" ")).collect();
    assert_eq!(verbs, ["AUTHENTICATE PLAIN", "EXAMINE \"INBOX\"", "UID FETCH", "UID FETCH", "LOGOUT"]);
    assert!(lines.iter().all(|line| !line.contains("BODY[")), "{lines:?}");
    Ok(())
}

#[test]
fn small_batches_split_the_summaries_and_the_body_cap_holds() -> Result {
    let config = SyncConfig { batch: 2, body_batch: 1, max_bodies: 1, ..SyncConfig::default() };
    let mut run = Run::start(recent_and_old()?, Some(PASSWORD), config)?;
    run.job(sync_inbox(None)?)?;
    let events = run.synced()?;
    let batches: Vec<usize> =
        events.iter().filter_map(|kind| if let EventKind::MessagesAdded { messages, .. } = kind { Some(messages.len()) } else { None }).collect();
    assert_eq!(batches, [2, 1]);
    assert_eq!(bodies(&events).len(), 1);
    run.finish()?;
    Ok(())
}

#[test]
fn a_server_without_preview_gives_summaries_without_one() -> Result {
    let mut run = Run::start(recent_and_old()?.without(Capability::Preview), Some(PASSWORD), SyncConfig::default())?;
    run.job(sync_inbox(None)?)?;
    let summaries = added(&run.synced()?);
    assert_eq!(summaries.len(), 3);
    assert!(summaries.iter().all(|summary| summary.preview().is_none()));
    let lines = run.finish()?;
    assert!(lines.iter().all(|line| !line.contains("PREVIEW")));
    Ok(())
}

#[test]
fn a_known_folder_brings_only_what_is_new() -> Result {
    let mut run = Run::start(recent_and_old()?, Some(PASSWORD), SyncConfig::default())?;
    run.job(sync_inbox(Some(FolderState::new(validity(1)?).with_uid_next(Some(uid(3)?))))?)?;
    let events = run.synced()?;
    assert_eq!(added(&events).iter().map(|summary| summary.uid().get()).collect::<Vec<_>>(), [3]);
    assert!(!events.iter().any(|kind| matches!(kind, EventKind::UidValidityReset { .. })));
    run.finish()?;
    Ok(())
}

#[test]
fn a_new_uidvalidity_resets_the_folder() -> Result {
    let mut run = Run::start(recent_and_old()?, Some(PASSWORD), SyncConfig::default())?;
    run.job(sync_inbox(Some(FolderState::new(validity(99)?).with_uid_next(Some(uid(3)?))))?)?;
    let events = run.synced()?;
    assert!(events.contains(&EventKind::UidValidityReset { folder: INBOX, old: Some(validity(99)?), new: validity(1)? }));
    assert_eq!(added(&events).len(), 3);
    run.finish()?;
    Ok(())
}

#[test]
fn an_empty_folder_is_synced_without_a_fetch() -> Result {
    let mut run = Run::sample()?;
    run.job(JobKind::SyncFolder { folder: FolderId::new(3), path: path("Drafts")?, known: None })?;
    let events = run.synced()?;
    assert!(added(&events).is_empty());
    assert!(matches!(events.last(), Some(EventKind::FolderSynced { .. })));
    let lines = run.finish()?;
    assert!(lines.iter().all(|line| !line.contains("FETCH")), "{lines:?}");
    Ok(())
}

#[test]
fn a_body_comes_on_demand_while_its_uidvalidity_holds() -> Result {
    let mut run = Run::sample()?;
    let archive = path("Archive")?;
    let fetch = |uid_validity, uid| JobKind::FetchBody { folder: FolderId::new(4), path: archive.clone(), uid_validity, uid };
    run.job(fetch(validity(7)?, uid(12)?))?;
    let events = run.until(|kind| matches!(kind, EventKind::BodyFetched { .. } | EventKind::Failed { .. }))?;
    let fetched = bodies(&events);
    assert_eq!(fetched.len(), 1);
    assert!(fetched[0].1.ends_with(b"From last year.\r\n"));

    run.job(fetch(validity(8)?, uid(12)?))?;
    let failed = run.until(|kind| matches!(kind, EventKind::Failed { .. }))?;
    assert_eq!(failed.last(), Some(&EventKind::Failed { kind: ErrorKind::MessageGone, folder: Some(FolderId::new(4)) }));
    run.job(fetch(validity(7)?, uid(13)?))?;
    let failed = run.until(|kind| matches!(kind, EventKind::Failed { .. }))?;
    assert_eq!(failed, [EventKind::Failed { kind: ErrorKind::MessageGone, folder: Some(FolderId::new(4)) }]);
    run.finish()?;
    Ok(())
}

#[test]
fn a_folder_that_is_gone_fails_alone() -> Result {
    let mut run = Run::sample()?;
    run.job(JobKind::SyncFolder { folder: FolderId::new(99), path: path("Nowhere")?, known: None })?;
    let failed = run.synced()?;
    assert_eq!(failed.last(), Some(&EventKind::Failed { kind: ErrorKind::FolderGone, folder: Some(FolderId::new(99)) }));
    // The connection stays: the next job runs on it without signing in again.
    run.job(JobKind::Idle { folder: INBOX, path: path("INBOX")? })?;
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    assert_eq!(events.len(), 1);
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("AUTHENTICATE")).count(), 1);
    Ok(())
}

#[test]
fn no_password_means_no_connection() -> Result {
    let mut run = Run::start(FakeImap::new(), None, SyncConfig::default())?;
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::Offline)))?;
    assert_eq!(
        events,
        [
            EventKind::Connection(ConnectionState::Connecting),
            EventKind::Failed { kind: ErrorKind::CredentialMissing, folder: None },
            EventKind::Connection(ConnectionState::Offline),
        ]
    );
    run.worker.stop();
    let (transcript, _no_client) = run.fake.finish_with_transcript();
    assert!(transcript.received().is_empty());
    Ok(())
}

#[test]
fn a_refused_password_is_not_sent_again() -> Result {
    let mut run = Run::start(FakeImap::new(), Some("a-wrong-invented-password"), SyncConfig::default())?;
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::Offline)))?;
    assert!(events.contains(&EventKind::Failed { kind: ErrorKind::LoginRefused, folder: None }));
    run.job(sync_inbox(None)?)?;
    assert_eq!(run.until(|_| true)?, [EventKind::Failed { kind: ErrorKind::LoginRefused, folder: Some(INBOX) }]);
    run.worker.stop();
    let (transcript, _ended) = run.fake.finish_with_transcript();
    assert_eq!(transcript.received().iter().filter(|line| line.contains("AUTHENTICATE")).count(), 1);
    Ok(())
}

#[test]
fn a_dropped_connection_ends_the_job_and_the_connection() -> Result {
    let fake = recent_and_old()?.on(When::command("UID FETCH"), [Action::Fault(Fault::Drop)]);
    let mut run = Run::start(fake, Some(PASSWORD), SyncConfig::default())?;
    run.job(sync_inbox(None)?)?;
    let events = run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::Offline)))?;
    assert!(events.contains(&EventKind::Failed { kind: ErrorKind::ConnectionDropped, folder: Some(INBOX) }), "{events:?}");
    run.worker.stop();
    Ok(())
}

#[test]
fn a_job_for_another_account_is_refused() -> Result {
    let run = Run::sample()?;
    let other = Job::new(JobId::new(1), AccountId::new(2), JobKind::SyncFolderList);
    assert_eq!(run.worker.send(other.clone()).map_err(|job| *job), Err(other));
    assert_eq!(run.worker.account(), ACCOUNT);
    assert!(format!("{:?}", run.worker).contains("running: true"));
    drop(run.worker);
    Ok(())
}

#[test]
fn the_worker_ends_when_nobody_listens() -> Result {
    let mut run = Run::sample()?;
    run.job(JobKind::SyncFolderList)?;
    run.job(JobKind::SyncFolderList)?;
    drop(run.events);
    // The thread sees the closed channel after the first event and ends; stop does not hang.
    run.worker.stop();
    Ok(())
}
