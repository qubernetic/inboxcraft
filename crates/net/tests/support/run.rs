// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An account worker against the fake IMAP server, and what the tests read from its events.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, EmailAddress, Event, EventKind, FlagChange, FolderId, FolderPath, FolderState, Job, JobId, JobKind, Security, Summary, Uid,
    UidValidity,
};
use inboxcraft_net::{Backoff, Credentials, Password, SyncConfig, Timeouts, Worker, WorkerSettings};
use inboxcraft_testkit::imap::{FakeImap, PASSWORD, USER};
use inboxcraft_testkit::{CertFault, FakeServer, Listen, TestPki};

use super::{Result, TIMEOUTS, connector, server, wall};

pub const ACCOUNT: AccountId = AccountId::new(1);
pub const INBOX: FolderId = FolderId::new(10);
/// How long a test waits for the next event.
pub const WAIT: Duration = Duration::from_secs(10);

/// A fixed password, or none.
pub struct Fixed(pub Option<&'static str>);

impl Credentials for Fixed {
    fn password(&self, account: &Account) -> Option<Password> {
        assert_eq!(account.id(), ACCOUNT);
        self.0.map(|text| Password::new(text.to_owned()))
    }
}

/// How a test worker runs.
pub struct Options {
    pub password: Option<&'static str>,
    pub sync: SyncConfig,
    pub backoff: Backoff,
    pub max_queued: usize,
    /// How many connections the fake serves, one after the other.
    pub connections: usize,
    pub timeouts: Timeouts,
    /// The certificate fault the fake's certificate has.
    pub cert: Option<CertFault>,
    pub compress: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            password: Some(PASSWORD),
            sync: SyncConfig::default(),
            backoff: Backoff { first: Duration::from_millis(20), cap: Duration::from_millis(80) },
            max_queued: inboxcraft_net::MAX_QUEUED,
            connections: 1,
            timeouts: TIMEOUTS,
            cert: None,
            compress: true,
        }
    }
}

pub struct Run {
    pub worker: Worker,
    pub events: Receiver<Event>,
    pub fake: FakeServer,
    next: u64,
}

impl Run {
    pub fn with(fake: FakeImap, options: Options) -> Result<Self> {
        let pki = TestPki::new()?;
        let listen = Listen::tls(pki.server_config(options.cert)?).accept_timeout(Duration::from_secs(2));
        let fake = if options.connections > 1 { listen.start_imap_many(fake, options.connections)? } else { listen.start_imap(fake)? };
        let account = Account::new(ACCOUNT, "Test", EmailAddress::new(USER)?, USER, server(Security::Tls)?)?;
        let connector = connector(&pki, wall(), fake.addr()).with_timeouts(options.timeouts);
        let settings = WorkerSettings {
            sync: options.sync,
            backoff: options.backoff,
            max_queued: options.max_queued,
            compress: options.compress,
            ..WorkerSettings::new(connector)
        };
        let (sender, events) = mpsc::channel();
        let worker = Worker::spawn(account, settings, Arc::new(Fixed(options.password)), sender)?;
        Ok(Self { worker, events, fake, next: 0 })
    }

    pub fn start(fake: FakeImap, password: Option<&'static str>, sync: SyncConfig) -> Result<Self> {
        Self::with(fake, Options { password, sync, ..Options::default() })
    }

    pub fn sample() -> Result<Self> {
        Self::start(FakeImap::sample(), Some(PASSWORD), SyncConfig::default())
    }

    /// Sends a job; its id.
    pub fn job(&mut self, kind: JobKind) -> Result<JobId> {
        self.next += 1;
        let id = JobId::new(self.next);
        self.worker.send(Job::new(id, ACCOUNT, kind)).map_err(|_| "job refused")?;
        Ok(id)
    }

    /// The next event.
    pub fn next(&self) -> Result<Event> {
        let event = self.events.recv_timeout(WAIT)?;
        assert_eq!(event.account, ACCOUNT);
        Ok(event)
    }

    /// The events up to and with the first one `last` takes.
    pub fn until(&self, last: impl Fn(&EventKind) -> bool) -> Result<Vec<EventKind>> {
        Ok(self.events_until(|event| last(&event.kind))?.into_iter().map(|event| event.kind).collect())
    }

    /// The events, with their job ids, up to and with the first one `last` takes.
    pub fn events_until(&self, last: impl Fn(&Event) -> bool) -> Result<Vec<Event>> {
        let mut seen = Vec::new();
        loop {
            let event = self.next()?;
            let done = last(&event);
            seen.push(event);
            if done {
                return Ok(seen);
            }
        }
    }

    /// The events of a folder sync, up to its end or its failure.
    pub fn synced(&self) -> Result<Vec<EventKind>> {
        self.until(|kind| matches!(kind, EventKind::FolderSynced { .. } | EventKind::Failed { .. }))
    }

    /// Stops the worker and returns the lines the server got.
    pub fn finish(self) -> Result<Vec<String>> {
        self.worker.stop();
        Ok(self.fake.finish()?.received().to_vec())
    }
}

pub fn path(name: &str) -> Result<FolderPath> {
    Ok(FolderPath::new(name, Some('/'))?)
}

pub fn sync_inbox(known: Option<FolderState>) -> Result<JobKind> {
    Ok(JobKind::SyncFolder { folder: INBOX, path: path("INBOX")?, known })
}

pub fn added(events: &[EventKind]) -> Vec<Summary> {
    events
        .iter()
        .filter_map(|kind| if let EventKind::MessagesAdded { messages, .. } = kind { Some(messages.clone()) } else { None })
        .flatten()
        .collect()
}

pub fn changed(events: &[EventKind]) -> Vec<FlagChange> {
    events
        .iter()
        .filter_map(|kind| if let EventKind::MessagesChanged { changes, .. } = kind { Some(changes.clone()) } else { None })
        .flatten()
        .collect()
}

/// The UIDs of every MessagesExpunged in `events`, one by one.
pub fn expunged(events: &[EventKind]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|kind| if let EventKind::MessagesExpunged { uids, .. } = kind { Some(uids.ranges().to_vec()) } else { None })
        .flatten()
        .flat_map(|(first, last)| first.get()..=last.get())
        .collect()
}

pub fn bodies(events: &[EventKind]) -> Vec<(u32, Vec<u8>)> {
    events.iter().filter_map(|kind| if let EventKind::BodyFetched { uid, raw, .. } = kind { Some((uid.get(), raw.clone())) } else { None }).collect()
}

/// The state a FolderSynced in `events` carries, the last one.
pub fn state(events: &[EventKind]) -> Option<FolderState> {
    events.iter().rev().find_map(|kind| if let EventKind::FolderSynced { state, .. } = kind { Some(*state) } else { None })
}

pub fn uid(value: u32) -> Result<Uid> {
    Ok(Uid::new(value).ok_or("uid")?)
}

pub fn validity(value: u32) -> Result<UidValidity> {
    Ok(UidValidity::new(value).ok_or("uidvalidity")?)
}
