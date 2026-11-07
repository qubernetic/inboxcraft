// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The opt-in live tests of `net` on the owner's two throwaway accounts on the reference provider (`just it-live`,
//! profile `live`): both `live-imap` and `live-imap2` sign in and list their folders; on `live-imap`, the folder list, the per-run folder `InboxCraft-Test/<run id>`, APPEND of an invented message, the first sync,
//! IDLE, the resync of a new worker, and the folder removed; the write path in the run folder (APPEND by the worker, a
//! flag, CREATE of a folder inside it and a move there); and `account.check`, whose every IMAP step passes on the real
//! chain.
//!
//! [`live::start`] skips it, with the reason printed, without the `INBOXCRAFT_LIVE_*` credentials and always in the
//! agent sandbox and in CI. Agents never run it. The accounts are reached over implicit TLS on port 993, checked by
//! the operating system's verifier. Nothing here prints an address, a login name or a secret; output names the
//! accounts only as `live-imap` and `live-imap2`; the `it-live` recipe
//! scans the output for them all the same.

mod support;

use std::num::NonZeroU16;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, CheckStep, EmailAddress, ErrorKind, Event, EventKind, Flag, Flags, FolderId, FolderPath, FolderState, Host, JobId, JobKind,
    Security, Server, StepReport, StepStatus, UidSet,
};
use inboxcraft_net::{CheckTarget, Connector, Credentials, Password, TlsSettings, Trust, Worker, WorkerSettings, check_account};
use inboxcraft_testkit::live::{self, Folders, LiveConfig, RunFolder};
use inboxcraft_time::SystemClock;
use support::Result;
use support::admin::Admin;
use support::run::{added, changed, expunged, state};

const ACCOUNT: AccountId = AccountId::new(1);
const RUN: FolderId = FolderId::new(1);
/// The folder the write test makes inside the run folder.
const MOVED: FolderId = FolderId::new(2);
/// How long a step may take on a real server.
const STEP: Duration = Duration::from_secs(120);

/// The secret of a live account, in memory only.
struct Secret(String);

impl Credentials for Secret {
    fn password(&self, _account: &Account) -> Option<Password> {
        Some(Password::new(self.0.clone()))
    }
}

/// A worker on a live account and its events.
struct Live {
    worker: Worker,
    events: Receiver<Event>,
    next: u64,
}

impl Live {
    fn start(imap: &live::Account, tls: &TlsSettings) -> Result<Self> {
        let port = NonZeroU16::new(993).ok_or("port")?;
        let server = Server::new(Host::new(imap.host())?, port, Security::Tls);
        let account = Account::new(ACCOUNT, imap.name(), EmailAddress::new(imap.address())?, imap.user(), server)?;
        let (sender, events) = mpsc::channel();
        let secret = Arc::new(Secret(imap.secret().expose().to_owned()));
        let worker = Worker::spawn(account, WorkerSettings::new(Connector::new(tls.clone())), secret, sender)?;
        Ok(Self { worker, events, next: 0 })
    }

    fn job(&mut self, kind: JobKind) -> Result<JobId> {
        self.next += 1;
        let id = JobId::new(self.next);
        self.worker.send(inboxcraft_model::Job::new(id, ACCOUNT, kind)).map_err(|_| "job refused")?;
        Ok(id)
    }

    /// The events up to the first one `last` takes; a failure of a job is an error, named by its kind only.
    fn until(&self, last: impl Fn(&EventKind) -> bool) -> Result<Vec<EventKind>> {
        let mut seen = Vec::new();
        loop {
            let event = self.events.recv_timeout(STEP).map_err(|_| "no event in time")?;
            let done = last(&event.kind);
            if !done && let (Some(_), EventKind::Failed { kind, .. }) = (event.job, &event.kind) {
                return Err(format!("a job failed: {kind}").into());
            }
            seen.push(event.kind);
            if done {
                return Ok(seen);
            }
        }
    }

    fn synced(&self) -> Result<Vec<EventKind>> {
        self.until(|kind| matches!(kind, EventKind::FolderSynced { .. }))
    }
}

fn subjects(events: &[EventKind]) -> Vec<String> {
    added(events).iter().map(|summary| summary.envelope().subject().to_owned()).collect()
}

fn subject(config: &LiveConfig, number: u32) -> String {
    format!("{} {} message {number}", config.message_subject_prefix(), config.run_id())
}

#[test]
fn live_both_accounts_sign_in_and_list_folders() -> Result {
    let Some(config) = live::start("live_both_accounts_sign_in_and_list_folders")? else {
        return Ok(());
    };
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    for account in config.accounts() {
        // Errors and output name the account as `live-imap` or `live-imap2` only.
        let name = account.name();
        let mut worker = Live::start(account, &tls).map_err(|err| format!("{name}: {err}"))?;
        worker.job(JobKind::SyncFolderList)?;
        let events = worker.until(|kind| matches!(kind, EventKind::Folders { .. })).map_err(|err| format!("{name}: {err}"))?;
        let Some(EventKind::Folders { listings }) = events.last() else { return Err(format!("{name}: no folder list").into()) };
        assert!(listings.iter().any(|listing| listing.path().is_inbox()), "{name}: no inbox in the folder list");
        worker.worker.stop();
        println!("live: {name} signed in, {} folders", listings.len());
    }
    Ok(())
}

#[test]
fn live_imap_round_trip() -> Result {
    let Some(config) = live::start("live_imap_round_trip")? else {
        return Ok(());
    };
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));

    // Sign in and list the folders; the run folder takes the server's hierarchy delimiter.
    let mut worker = Live::start(config.imap(), &tls)?;
    worker.job(JobKind::SyncFolderList)?;
    let events = worker.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    let Some(EventKind::Folders { listings }) = events.last() else { return Err("no folder list".into()) };
    let delimiter = listings.iter().find(|listing| listing.path().is_inbox()).and_then(|listing| listing.path().delimiter()).unwrap_or('/');
    println!("live: signed in, {} folders, delimiter {delimiter:?}", listings.len());

    let imap = config.imap();
    let mut admin = Admin::connect_host(imap.host(), tls.client_config()?, imap.user(), imap.secret().expose())?;
    let mut folder = RunFolder::create(&mut admin, &config, delimiter)?;
    let name = folder.name().to_owned();
    let path = FolderPath::new(&name, Some(delimiter))?;
    folder.folders().append(&name, &config.invented_message(1))?;
    println!("live: run folder created, one invented message appended");

    // The first sync brings the message.
    worker.job(JobKind::SyncFolder { folder: RUN, path: path.clone(), known: None })?;
    let events = worker.synced()?;
    assert_eq!(subjects(&events), [subject(&config, 1)]);
    println!("live: first sync done");

    // Watched with IDLE, a message appended by another client comes in.
    worker.job(JobKind::Idle { folder: RUN, path: path.clone() })?;
    worker.until(|kind| *kind == EventKind::Connection(inboxcraft_model::ConnectionState::Watching))?;
    folder.folders().append(&name, &config.invented_message(2))?;
    let events = worker.synced()?;
    assert_eq!(subjects(&events), [subject(&config, 2)]);
    let known: FolderState = state(&events).ok_or("no state")?;
    worker.worker.stop();
    println!("live: IDLE brought the second message");

    // A new worker resyncs from the state the last one left: only what came since.
    folder.folders().append(&name, &config.invented_message(3))?;
    let mut again = Live::start(config.imap(), &tls)?;
    again.job(JobKind::SyncFolder { folder: RUN, path: path.clone(), known: Some(known) })?;
    let events = again.synced()?;
    assert_eq!(subjects(&events), [subject(&config, 3)]);
    assert!(!events.iter().any(|kind| matches!(kind, EventKind::UidValidityReset { .. })));
    println!("live: resync brought only the third message");

    // The run folder is removed, and the folder is gone for the worker too.
    folder.remove()?;
    again.job(JobKind::SyncFolder { folder: RUN, path, known: Some(known) })?;
    let events = again.until(|kind| matches!(kind, EventKind::Failed { .. }))?;
    assert_eq!(events.last(), Some(&EventKind::Failed { kind: ErrorKind::FolderGone, folder: Some(RUN) }));
    again.worker.stop();
    admin.logout();
    println!("live: run folder removed");
    Ok(())
}

#[test]
fn live_imap_writes_in_the_run_folder() -> Result {
    let Some(config) = live::start("live_imap_writes_in_the_run_folder")? else {
        return Ok(());
    };
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let mut worker = Live::start(config.imap(), &tls)?;
    worker.job(JobKind::SyncFolderList)?;
    let events = worker.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    let Some(EventKind::Folders { listings }) = events.last() else { return Err("no folder list".into()) };
    let delimiter = listings.iter().find(|listing| listing.path().is_inbox()).and_then(|listing| listing.path().delimiter()).unwrap_or('/');

    let imap = config.imap();
    let mut admin = Admin::connect_host(imap.host(), tls.client_config()?, imap.user(), imap.secret().expose())?;
    let mut folder = RunFolder::create(&mut admin, &config, delimiter)?;
    let name = folder.name().to_owned();
    let path = FolderPath::new(&name, Some(delimiter))?;
    // Inside the run folder, so the test touches nothing else of the account.
    let moved_name = format!("{name}{delimiter}Moved");
    let moved_path = FolderPath::new(&moved_name, Some(delimiter))?;

    // APPEND by the worker; the first sync finds the message.
    let raw = config.invented_message(1).into_bytes();
    worker.job(JobKind::Append { folder: RUN, path: path.clone(), flags: Flags::default(), raw, draft: None, message: None })?;
    worker.until(|kind| matches!(kind, EventKind::Appended { .. }))?;
    worker.job(JobKind::SyncFolder { folder: RUN, path: path.clone(), known: None })?;
    let events = worker.synced()?;
    assert_eq!(subjects(&events), [subject(&config, 1)]);
    let message = added(&events).into_iter().next().ok_or("no message")?;
    let state = state(&events).ok_or("no state")?;
    let uids: UidSet = std::iter::once(message.uid()).collect();
    println!("live: APPEND done, the sync found the message");

    // A flag, as the server then has it.
    let flagged: Flags = std::iter::once(Flag::Flagged).collect();
    let uid_validity = state.uid_validity();
    worker.job(JobKind::StoreFlags { folder: RUN, path: path.clone(), uid_validity, uids: uids.clone(), add: flagged, remove: Flags::default() })?;
    let events = worker.until(|kind| matches!(kind, EventKind::FlagsStored { .. }))?;
    assert!(changed(&events).iter().any(|change| change.uid == message.uid() && change.flags.is_flagged()));
    println!("live: flag stored");

    // A folder of its own, and the message moved there with its flag. The folder goes before anything is checked: a
    // server may refuse to delete the run folder while it has a child.
    let moved = || -> Result<(Vec<EventKind>, Vec<EventKind>)> {
        worker.job(JobKind::CreateFolder { path: moved_path.clone() })?;
        worker.until(|kind| matches!(kind, EventKind::FolderCreated { .. }))?;
        worker.job(JobKind::Move { folder: RUN, path: path.clone(), uid_validity, uids, to: MOVED, to_path: moved_path.clone() })?;
        let events = worker.until(|kind| matches!(kind, EventKind::Moved { .. }))?;
        println!("live: moved, new UID named: {}", matches!(events.last(), Some(EventKind::Moved { copied: Some(_), .. })));
        worker.job(JobKind::SyncFolder { folder: MOVED, path: moved_path.clone(), known: None })?;
        let in_moved = worker.synced()?;
        worker.job(JobKind::SyncFolder { folder: RUN, path: path.clone(), known: Some(state) })?;
        Ok((in_moved, worker.synced()?))
    };
    let outcome = moved();
    worker.worker.stop();
    let removed = folder.folders().delete(&moved_name);
    let (in_moved, in_run) = outcome?;
    removed?;
    assert_eq!(subjects(&in_moved), [subject(&config, 1)]);
    assert!(added(&in_moved).iter().all(|summary| summary.flags().is_flagged()));
    assert!(expunged(&in_run).contains(&message.uid().get()), "the message is still in the run folder");
    println!("live: the message is in the new folder only");

    folder.remove()?;
    admin.logout();
    println!("live: folders removed");
    Ok(())
}

#[test]
fn live_account_check_passes_every_imap_step() -> Result {
    let Some(config) = live::start("live_account_check_passes_every_imap_step")? else {
        return Ok(());
    };
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let imap = config.imap();
    let server = Server::new(Host::new(imap.host())?, NonZeroU16::new(993).ok_or("port")?, Security::Tls);
    let target = CheckTarget::new(server, imap.user());
    let mut reports = Vec::new();
    check_account(&target, &Connector::new(tls), &mut || Some(Password::new(imap.secret().expose().to_owned())), &mut |report| {
        // The step and its status only: no host, no issuer, nothing of the account.
        println!("live: {:?} {:?}", report.step(), report.status());
        reports.push(report);
    });
    let statuses: Vec<StepStatus> = reports.iter().map(StepReport::status).collect();
    let passed = StepStatus::Passed;
    assert_eq!(statuses, [passed, passed, passed, passed, passed, StepStatus::NotBuilt, StepStatus::NotBuilt]);
    assert!(reports.iter().find(|report| report.step() == CheckStep::Certificate).and_then(StepReport::issuer).is_some());
    Ok(())
}
