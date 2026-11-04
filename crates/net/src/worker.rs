// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One worker thread per account: it takes the engine's [`Job`]s in order, keeps one signed-in connection, and
//! reports everything as [`Event`]s.
//!
//! The connection opens at the first job and again after a failure that ended it. Jobs wait in an offline queue
//! while there is no connection; a failure the network or the server may get over (no route, a refused or dropped
//! connection, a timeout, a server that broke the protocol) is reported, and the worker tries again after a
//! [`Backoff`] wait that doubles up to its cap, keeping the queue. A job that was running when its connection ended
//! is reported failed and not run again: a job that breaks the connection every time cannot hold the queue up.
//!
//! A server that refused the password, offers no way to sign in, or failed the certificate check is not asked again:
//! the queued jobs and every later job fail with the same kind until the engine starts a new worker (after the owner
//! changed the settings), so a wrong password is never sent over and over. Without a stored password the queued jobs
//! fail, and the next job asks for it again. The password is asked for each time the worker signs in and is dropped
//! right after.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use inboxcraft_model::{Account, AccountId, ConnectionState, ErrorKind, Event, EventKind, Job, JobId};

use crate::backoff::{Backoff, whole_seconds};
use crate::sync::{self, SyncConfig};
use crate::{Connector, ImapConnection, NetError, Password};

/// The most jobs the offline queue holds by default.
pub const MAX_QUEUED: usize = 1_000;

/// Where the worker gets an account's password: the OS credential store in the app, a fixed one in tests.
pub trait Credentials: Send + Sync {
    /// The password of `account`, or `None` when there is none.
    fn password(&self, account: &Account) -> Option<Password>;
}

/// A worker's settings.
#[derive(Debug, Clone)]
pub struct WorkerSettings {
    /// How connections are opened.
    pub connector: Connector,
    /// How folders are synced.
    pub sync: SyncConfig,
    /// How long to wait before connecting again after a failure.
    pub backoff: Backoff,
    /// The most jobs that wait for a connection; a job beyond them fails at once with [`ErrorKind::TooLarge`].
    pub max_queued: usize,
}

impl WorkerSettings {
    /// The default settings with `connector`.
    #[must_use]
    pub fn new(connector: Connector) -> Self {
        Self { connector, sync: SyncConfig::default(), backoff: Backoff::default(), max_queued: MAX_QUEUED }
    }
}

/// The worker of one account; dropping it (or [`Worker::stop`]) lets the thread finish its queued jobs while it is
/// online, sign out and end. Offline, it ends at once and the queued jobs are dropped.
pub struct Worker {
    account: AccountId,
    jobs: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for Worker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Worker").field("account", &self.account).field("running", &self.thread.is_some()).finish()
    }
}

impl Worker {
    /// Starts the worker thread of `account`; its events go to `events`.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::ConnectionDropped`] when the thread cannot be started.
    pub fn spawn(account: Account, settings: WorkerSettings, credentials: Arc<dyn Credentials>, events: Sender<Event>) -> Result<Self, NetError> {
        let id = account.id();
        let (jobs, queue) = mpsc::channel();
        let thread = thread::Builder::new()
            .name(format!("net-{id}"))
            .spawn(move || work(&account, &settings, credentials.as_ref(), &queue, &events))
            .map_err(|error| NetError::new(ErrorKind::ConnectionDropped, error.to_string()))?;
        Ok(Self { account: id, jobs: Some(jobs), thread: Some(thread) })
    }

    /// The account.
    #[must_use]
    pub fn account(&self) -> AccountId {
        self.account
    }

    /// Queues `job`.
    ///
    /// # Errors
    ///
    /// The job back when it is for another account or the worker has ended.
    pub fn send(&self, job: Job) -> Result<(), Box<Job>> {
        if job.account != self.account {
            return Err(Box::new(job));
        }
        match &self.jobs {
            Some(jobs) => jobs.send(job).map_err(|refused| Box::new(refused.0)),
            None => Err(Box::new(job)),
        }
    }

    /// Lets the thread end as dropping the worker does, and waits for it.
    pub fn stop(mut self) {
        self.end();
    }

    fn end(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ended = thread.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.end();
    }
}

/// The thread: turn after turn, until the queue closes or nobody listens to the events.
fn work(account: &Account, settings: &WorkerSettings, credentials: &dyn Credentials, queue: &Receiver<Job>, events: &Sender<Event>) {
    let mut run = Run {
        account,
        settings,
        credentials,
        out: Out { account: account.id(), events, listening: true, reported: None },
        connection: None,
        halted: None,
        pending: VecDeque::new(),
        open: true,
        attempt: 0,
    };
    while run.turn(queue) {}
    if let Some(open) = run.connection {
        open.logout();
    }
}

/// Where the events go.
struct Out<'a> {
    account: AccountId,
    events: &'a Sender<Event>,
    /// The engine still takes events.
    listening: bool,
    /// The connection state told last.
    reported: Option<ConnectionState>,
}

impl Out<'_> {
    fn say(&mut self, job: Option<JobId>, kind: EventKind) {
        self.listening &= self.events.send(Event::new(self.account, job, kind)).is_ok();
    }

    /// Tells `state` unless it was the last one told.
    fn report(&mut self, state: ConnectionState) {
        if self.reported != Some(state) {
            self.reported = Some(state);
            self.say(None, EventKind::Connection(state));
        }
    }

    fn failed(&mut self, job: &Job, error: &NetError) {
        self.say(Some(job.id), EventKind::Failed { kind: error.kind(), folder: job.kind.folder() });
    }
}

/// The worker's state.
struct Run<'a> {
    account: &'a Account,
    settings: &'a WorkerSettings,
    credentials: &'a dyn Credentials,
    out: Out<'a>,
    connection: Option<ImapConnection>,
    /// Why the worker stopped trying for good.
    halted: Option<NetError>,
    /// The offline queue: jobs taken from the channel and not run yet.
    pending: VecDeque<Job>,
    /// The engine can still send jobs.
    open: bool,
    /// Failed attempts to connect in a row.
    attempt: u32,
}

impl Run<'_> {
    /// One turn: takes the jobs that came, connects when there is none, runs the next job. `false` when the worker
    /// ends.
    fn turn(&mut self, queue: &Receiver<Job>) -> bool {
        if self.pending.is_empty() {
            match queue.recv() {
                Ok(job) => self.take(job),
                Err(_) => return false,
            }
        }
        self.drain(queue);
        if let Some(error) = self.halted.clone() {
            self.fail_pending(&error);
        } else if self.connection.is_none() && !self.connect(queue) {
            return self.out.listening && self.open;
        }
        if let Some(job) = self.pending.pop_front() {
            self.run(&job);
        }
        self.out.listening
    }

    /// Takes the jobs waiting in the channel without blocking.
    fn drain(&mut self, queue: &Receiver<Job>) {
        loop {
            match queue.try_recv() {
                Ok(job) => self.take(job),
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.open = false;
                    return;
                }
            }
        }
    }

    /// Queues `job`, or fails it when the queue is full.
    fn take(&mut self, job: Job) {
        if self.pending.len() >= self.settings.max_queued {
            self.out.failed(&job, &NetError::new(ErrorKind::TooLarge, "the offline queue is full"));
        } else {
            self.pending.push_back(job);
        }
    }

    fn fail_pending(&mut self, error: &NetError) {
        for job in std::mem::take(&mut self.pending) {
            self.out.failed(&job, error);
        }
    }

    /// Opens and signs in a connection; whether it did. After a failure the network may get over, waits the back-off
    /// while taking jobs; after any other failure, fails the queued jobs.
    fn connect(&mut self, queue: &Receiver<Job>) -> bool {
        self.out.report(ConnectionState::Connecting);
        let error = match sign_in(self.account, self.settings, self.credentials) {
            Ok(open) => {
                self.connection = Some(open);
                self.attempt = 0;
                self.out.report(ConnectionState::Online);
                return true;
            }
            Err(error) => error,
        };
        self.out.say(None, EventKind::Failed { kind: error.kind(), folder: None });
        if error.is_transient() {
            self.attempt = self.attempt.saturating_add(1);
            let wait = self.settings.backoff.delay(self.attempt);
            self.out.report(ConnectionState::BackingOff { attempt: self.attempt, retry_in_secs: whole_seconds(wait) });
            self.wait(queue, wait);
        } else {
            self.out.report(ConnectionState::Offline);
            if error.kind() != ErrorKind::CredentialMissing {
                self.halted = Some(error.clone());
            }
            self.fail_pending(&error);
        }
        false
    }

    /// Waits `wait`, taking the jobs that come meanwhile; ends early when the engine stops the worker.
    fn wait(&mut self, queue: &Receiver<Job>, wait: Duration) {
        let clock = self.settings.connector.tls().clock();
        let until = clock.now().saturating_add(wait);
        let mut left = wait;
        loop {
            match queue.recv_timeout(left) {
                Ok(job) => self.take(job),
                Err(RecvTimeoutError::Timeout) => return,
                Err(RecvTimeoutError::Disconnected) => {
                    self.open = false;
                    return;
                }
            }
            match until.duration_since(clock.now()) {
                Some(rest) if !rest.is_zero() => left = rest,
                _ => return,
            }
        }
    }

    /// Runs `job` on the connection; a failure that ends the connection drops it.
    fn run(&mut self, job: &Job) {
        let Some(open) = self.connection.as_mut() else { return };
        let now = self.settings.connector.tls().clock().now();
        let out = &mut self.out;
        if let Err(error) = sync::run(open, job, &self.settings.sync, now, &mut |event| out.listening &= out.events.send(event).is_ok()) {
            out.failed(job, &error);
            if error.ends_connection() {
                self.connection = None;
                out.report(ConnectionState::Offline);
            }
        }
    }
}

/// A signed-in connection for `account`; the password is asked for first, so none means no connection at all.
fn sign_in(account: &Account, settings: &WorkerSettings, credentials: &dyn Credentials) -> Result<ImapConnection, NetError> {
    let password = credentials.password(account).ok_or_else(|| NetError::new(ErrorKind::CredentialMissing, "no password stored"))?;
    let mut connection = ImapConnection::open(account.imap(), &settings.connector)?;
    connection.login(account.username(), &password)?;
    Ok(connection)
}
