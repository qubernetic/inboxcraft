// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The opt-in live test of `net` on the owner's throwaway account `live-imap` (`just it-live`, profile `live`): sign
//! in, the folder list, the per-run folder `InboxCraft-Test/<run id>`, APPEND of an invented message, the first sync,
//! IDLE, the resync of a new worker, and the folder removed.
//!
//! [`live::start`] skips it, with the reason printed, without the `INBOXCRAFT_LIVE_*` credentials and always in the
//! agent sandbox and in CI. Agents never run it. The account is reached over implicit TLS on port 993, checked by the
//! operating system's verifier. Nothing here prints an address, a login name or the secret; the `it-live` recipe
//! scans the output for them all the same.

mod support;

use std::num::NonZeroU16;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, EmailAddress, ErrorKind, Event, EventKind, FolderId, FolderPath, FolderState, Host, JobId, JobKind, Security, Server,
};
use inboxcraft_net::{Connector, Credentials, Password, TlsSettings, Trust, Worker, WorkerSettings};
use inboxcraft_testkit::live::{self, LiveConfig, RunFolder};
use inboxcraft_time::SystemClock;
use support::Result;
use support::admin::Admin;
use support::run::{added, state};

const ACCOUNT: AccountId = AccountId::new(1);
const RUN: FolderId = FolderId::new(1);
/// How long a step may take on a real server.
const STEP: Duration = Duration::from_secs(120);

/// The secret of `live-imap`, in memory only.
struct Secret(String);

impl Credentials for Secret {
    fn password(&self, _account: &Account) -> Option<Password> {
        Some(Password::new(self.0.clone()))
    }
}

/// A worker on the live account and its events.
struct Live {
    worker: Worker,
    events: Receiver<Event>,
    next: u64,
}

impl Live {
    fn start(config: &LiveConfig, tls: &TlsSettings) -> Result<Self> {
        let imap = config.imap();
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
fn live_imap_round_trip() -> Result {
    let Some(config) = live::start("live_imap_round_trip")? else {
        return Ok(());
    };
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));

    // Sign in and list the folders; the run folder takes the server's hierarchy delimiter.
    let mut worker = Live::start(&config, &tls)?;
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
    let mut again = Live::start(&config, &tls)?;
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
