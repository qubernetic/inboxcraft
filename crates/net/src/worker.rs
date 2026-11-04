// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One worker thread per account: it takes the engine's [`Job`]s in order, keeps one signed-in connection, and
//! reports everything as [`Event`]s.
//!
//! The connection opens at the first job and again after a failure that ended it; there is no back-off yet (E1.7b),
//! so a failed job is reported and the next job tries again. A server that refused the password, or offers no way to
//! sign in, is not asked again: every later job fails with the same kind until the engine starts a new worker
//! (after the owner changed the password), so a wrong password is never sent over and over. The password is asked
//! for each time the worker signs in and is dropped right after.

use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use inboxcraft_model::{Account, AccountId, ConnectionState, ErrorKind, Event, EventKind, Job};

use crate::sync::{self, SyncConfig};
use crate::{Connector, ImapConnection, NetError, Password};

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
}

/// The worker of one account; dropping it (or [`Worker::stop`]) lets the thread finish its jobs, sign out and end.
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

    /// Lets the thread finish the queued jobs, sign out and end, and waits for it.
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

/// The thread: one job after the other, until the queue closes or nobody listens to the events.
fn work(account: &Account, settings: &WorkerSettings, credentials: &dyn Credentials, queue: &Receiver<Job>, events: &Sender<Event>) {
    let mut connection: Option<ImapConnection> = None;
    let mut refused: Option<NetError> = None;
    for job in queue {
        let mut listening = true;
        let mut emit = |event: Event| listening &= events.send(event).is_ok();
        let say = |emit: &mut dyn FnMut(Event), kind: EventKind| emit(Event::new(account.id(), Some(job.id), kind));
        let failed = |error: &NetError| EventKind::Failed { kind: error.kind(), folder: job.kind.folder() };
        if let Some(error) = &refused {
            say(&mut emit, failed(error));
        } else if connection.is_none() {
            say(&mut emit, EventKind::Connection(ConnectionState::Connecting));
            match sign_in(account, settings, credentials) {
                Ok(open) => {
                    connection = Some(open);
                    say(&mut emit, EventKind::Connection(ConnectionState::Online));
                }
                Err(error) => {
                    say(&mut emit, failed(&error));
                    say(&mut emit, EventKind::Connection(ConnectionState::Offline));
                    if matches!(error.kind(), ErrorKind::LoginRefused | ErrorKind::NoLoginMethod) {
                        refused = Some(error);
                    }
                }
            }
        }
        if let Some(open) = connection.as_mut() {
            let now = settings.connector.tls().clock().now();
            if let Err(error) = sync::run(open, &job, &settings.sync, now, &mut emit) {
                say(&mut emit, failed(&error));
                if error.ends_connection() {
                    connection = None;
                    say(&mut emit, EventKind::Connection(ConnectionState::Offline));
                }
            }
        }
        if !listening {
            break;
        }
    }
    if let Some(open) = connection {
        open.logout();
    }
}

/// A signed-in connection for `account`; the password is asked for first, so none means no connection at all.
fn sign_in(account: &Account, settings: &WorkerSettings, credentials: &dyn Credentials) -> Result<ImapConnection, NetError> {
    let password = credentials.password(account).ok_or_else(|| NetError::new(ErrorKind::CredentialMissing, "no password stored"))?;
    let mut connection = ImapConnection::open(account.imap(), &settings.connector)?;
    connection.login(account.username(), &password)?;
    Ok(connection)
}
