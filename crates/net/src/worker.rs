// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One worker thread per account: it takes the engine's [`Job`]s in order, keeps one signed-in connection, and
//! reports everything as [`Event`]s.
//!
//! The connection opens at the first job and again after a failure that ended it. Jobs wait in an offline queue
//! while there is no connection; a failure the network or the server may get over (no route, a refused or dropped
//! connection, a timeout, a server that broke the protocol) is reported, and the worker tries again after a
//! [`Backoff`] wait that doubles up to its cap, keeping the queue. A read job that was running when its connection
//! ended is reported failed and not run again. A write job (an act of the owner) that a dropped or timed-out
//! connection cut off goes back to the head of the queue and runs again after the reconnect, at most
//! [`WRITE_TRIES`] times in all, so a job that breaks the connection every time cannot hold the queue up; the write
//! path ends in the same state run once or twice (see `write`). A move on a server without MOVE is the one write that
//! does not run again, as its fallback may have copied the messages already. Any other failure of a write is
//! reported, and the engine takes the act back.
//!
//! An Idle job names the folder to watch between jobs: the worker resyncs it, then waits in IDLE (or polls a server
//! without IDLE) until a job comes or the server tells of a change, which brings another resync. A connection that
//! ends while watching is reconnected after the back-off and the folder resynced, so what changed while the worker
//! was offline is reported like any other change. Another Idle job moves the watch; a folder that is gone ends it.
//!
//! A server that refused the password, offers no way to sign in, or failed the certificate check is not asked again:
//! the queued jobs and every later job fail with the same kind until the engine starts a new worker (after the owner
//! changed the settings), so a wrong password is never sent over and over. The same holds for a sign-in the server
//! never answered: after a `BYE` or something that is not IMAP at once, after a dropped or timed-out connection on
//! the second time before a sign-in succeeds. Without a stored password the queued jobs
//! fail, and the next job asks for it again. The password is asked for each time the worker signs in and is dropped
//! right after.
//!
//! A Check job runs at once on a connection of its own, also while the worker is offline, backing off or halted: it
//! reports each step of the account check (see [`crate::check_account`]) under the job's id. After a refused or
//! unanswered sign-in (the worker halted for it, or the last check's sign-in ended so), a check the owner did not
//! start stops before the sign-in with [`inboxcraft_model::StepStatus::NeedsOwner`]: an agent that checks in a loop
//! cannot send the password again and lock the account at the provider.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, CheckStep, ConnectionState, DraftId, EmailAddress, ErrorKind, Event, EventKind, FolderId, FolderPath, FolderState, Job,
    JobId, JobKind, StepStatus,
};

use crate::backoff::{Backoff, whole_seconds};
use crate::sync::{self, SyncConfig};
use crate::watch::{self, WatchConfig};
use crate::{CheckTarget, Connector, ImapConnection, NetError, Password, check_account};
use crate::{smtp, write};

/// The most jobs the offline queue holds by default.
pub const MAX_QUEUED: usize = 1_000;

/// How often a write job is run when its connection drops or times out under it each time.
pub const WRITE_TRIES: u32 = 3;

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
    /// Compress the connection where the server offers COMPRESS=DEFLATE.
    pub compress: bool,
    /// How a watched folder is waited on.
    pub watch: WatchConfig,
}

impl WorkerSettings {
    /// The default settings with `connector`.
    #[must_use]
    pub fn new(connector: Connector) -> Self {
        Self {
            connector,
            sync: SyncConfig::default(),
            backoff: Backoff::default(),
            max_queued: MAX_QUEUED,
            compress: true,
            watch: WatchConfig::default(),
        }
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
        out: Out { account: account.id(), events, listening: true, reported: None, states: HashMap::new() },
        connection: None,
        halted: None,
        pending: VecDeque::new(),
        open: true,
        attempt: 0,
        watch: None,
        stale: false,
        idle_refused: false,
        sign_in_lost: false,
        tries: HashMap::new(),
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
    /// The state of each folder as the last sync left it, for the resync of a watched folder.
    states: HashMap<FolderId, FolderState>,
}

impl Out<'_> {
    /// Sends `event`, noting the folder state it carries.
    fn emit(&mut self, event: Event) {
        match &event.kind {
            EventKind::FolderSynced { folder, state } => {
                self.states.insert(*folder, *state);
            }
            EventKind::UidValidityReset { folder, .. } => {
                self.states.remove(folder);
            }
            _ => {}
        }
        self.listening &= self.events.send(event).is_ok();
    }

    fn say(&mut self, job: Option<JobId>, kind: EventKind) {
        self.emit(Event::new(self.account, job, kind));
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

/// The folder an Idle job asked to watch.
#[derive(Debug, Clone)]
struct Watch {
    job: JobId,
    folder: FolderId,
    path: FolderPath,
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
    /// Failed attempts in a row: to connect, or to keep a connection that ended while watching.
    attempt: u32,
    /// The folder watched between jobs.
    watch: Option<Watch>,
    /// The watched folder needs a resync before the next wait: new watch, new connection, a change, or a job that
    /// opened another folder.
    stale: bool,
    /// The server refused IDLE on this connection: poll instead.
    idle_refused: bool,
    /// A sign-in went unanswered since the last one that succeeded: the next one that does halts the tries.
    sign_in_lost: bool,
    /// The write jobs a dropped connection cut off, with how often that happened.
    tries: HashMap<JobId, u32>,
}

impl Run<'_> {
    /// One turn: takes the jobs that came, connects when there is none, runs the next job or watches the folder.
    /// `false` when the worker ends.
    fn turn(&mut self, queue: &Receiver<Job>) -> bool {
        if self.pending.is_empty() && self.watch.is_none() {
            match queue.recv() {
                Ok(job) => self.take(job),
                Err(_) => return false,
            }
        }
        self.drain(queue);
        // Stopped: the queued jobs run while online; offline, or with nothing queued, the worker ends.
        if !self.open && (self.pending.is_empty() || self.connection.is_none()) {
            return false;
        }
        // A check or a send ran on a connection of its own: nothing needs this one.
        if self.pending.is_empty() && self.watch.is_none() {
            return self.out.listening;
        }
        if let Some(error) = self.halted.clone() {
            self.fail_pending(&error);
            self.watch = None;
        } else if self.connection.is_some() || self.connect(queue) {
            match self.pending.pop_front() {
                Some(job) => self.run(job),
                None => self.watch(queue),
            }
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

    /// Queues `job`, or fails it when the queue is full. A check runs at once, on a connection of its own: it is how
    /// the owner finds out why the worker cannot connect, so it waits for neither the connection nor the back-off,
    /// and runs also after the worker stopped trying.
    fn take(&mut self, job: Job) {
        if let JobKind::Check { by_owner } = job.kind {
            self.check(job.id, by_owner);
        } else if let JobKind::Send { draft, from, recipients, raw } = &job.kind {
            self.send(job.id, *draft, from, recipients, raw);
        } else if self.pending.len() >= self.settings.max_queued {
            self.out.failed(&job, &NetError::new(ErrorKind::TooLarge, "the offline queue is full"));
        } else {
            self.pending.push_back(job);
        }
    }

    /// Runs a check; it signs in only when the owner started it or the worker has not halted for a refused or
    /// unanswered sign-in. A check whose sign-in ends so halts the worker like its own would; one that signs in ends
    /// such a halt, as the password works again.
    fn check(&mut self, job: JobId, by_owner: bool) {
        let refused = self.halted.as_ref().is_some_and(|error| refuses_the_password(error.kind()));
        let target = CheckTarget::of(self.account);
        let target = if refused && !by_owner { target.held() } else { target };
        let (account, credentials) = (self.account, self.credentials);
        let mut signed_in = None;
        let out = &mut self.out;
        check_account(&target, &self.settings.connector, &mut || credentials.password(account), &mut |report| {
            if report.step() == CheckStep::ImapLogin {
                signed_in = Some(report.status());
            }
            out.say(Some(job), EventKind::CheckStep(report));
        });
        match signed_in {
            Some(StepStatus::Passed) if refused => self.halted = None,
            Some(StepStatus::Failed(kind)) if refuses_the_password(kind) => {
                self.halted = Some(NetError::new(kind, "the check's sign-in was refused or went unanswered"));
            }
            _ => {}
        }
    }

    /// Sends a message on a submission connection of its own, at once: it needs no IMAP connection. After the
    /// password was refused or went unanswered, nothing is sent until the engine starts a new worker.
    fn send(&mut self, job: JobId, draft: DraftId, from: &EmailAddress, recipients: &[EmailAddress], raw: &[u8]) {
        let halted = self.halted.as_ref().filter(|error| refuses_the_password(error.kind()));
        let sent = match halted {
            Some(error) => Err(error.clone()),
            None => smtp::submit(self.account, &self.settings.connector, self.credentials, from, recipients, raw),
        };
        let kind = match sent {
            Ok(Ok(_)) => EventKind::Sent { draft },
            Ok(Err(refusal)) => EventKind::SendFailed { draft, kind: ErrorKind::MessageRefused, reply: Some(smtp::reply(&refusal)) },
            Err(error) => EventKind::SendFailed { draft, kind: error.kind(), reply: None },
        };
        self.out.say(Some(job), kind);
    }

    fn fail_pending(&mut self, error: &NetError) {
        for job in std::mem::take(&mut self.pending) {
            self.out.failed(&job, error);
        }
    }

    /// Opens and signs in a connection; whether it did. After a failure the network may get over, waits the back-off
    /// while taking jobs; after any other failure, fails the queued jobs and ends the watch.
    fn connect(&mut self, queue: &Receiver<Job>) -> bool {
        self.out.report(ConnectionState::Connecting);
        let error = match sign_in(self.account, self.settings, self.credentials) {
            Ok(open) => {
                self.connection = Some(open);
                self.stale = self.watch.is_some();
                self.idle_refused = false;
                self.sign_in_lost = false;
                self.out.report(ConnectionState::Online);
                return true;
            }
            Err(error) => error,
        };
        self.out.say(None, EventKind::Failed { kind: error.kind(), folder: None });
        // An unanswered sign-in gets one more try until a sign-in succeeds, and only after a dropped or timed-out
        // connection.
        let again = error.may_sign_in_again() && !self.sign_in_lost;
        self.sign_in_lost |= error.may_sign_in_again();
        if error.is_transient() || again {
            self.back_off(queue);
        } else {
            self.out.report(ConnectionState::Offline);
            if error.kind() != ErrorKind::CredentialMissing {
                self.halted = Some(error.clone());
            }
            self.fail_pending(&error);
            self.watch = None;
        }
        false
    }

    /// Counts a failed attempt, tells the wait, and waits it.
    fn back_off(&mut self, queue: &Receiver<Job>) {
        self.attempt = self.attempt.saturating_add(1);
        let wait = self.settings.backoff.delay(self.attempt);
        self.out.report(ConnectionState::BackingOff { attempt: self.attempt, retry_in_secs: whole_seconds(wait) });
        self.wait(queue, wait);
    }

    /// Waits `wait`, taking the jobs that come meanwhile; ends early when the engine stops the worker. The wait is
    /// measured on the monotonic clock, so a wall clock set back cannot stretch it past the back-off cap.
    fn wait(&mut self, queue: &Receiver<Job>, wait: Duration) {
        let clock = Arc::clone(self.settings.connector.tls().clock());
        let started = clock.monotonic();
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
            match wait.checked_sub(clock.monotonic().saturating_sub(started)) {
                Some(rest) if !rest.is_zero() => left = rest,
                _ => return,
            }
        }
    }

    /// Runs `job` on the connection; a failure that ends the connection drops it, and puts a write job that may run
    /// again back at the head of the queue. An Idle job only names the folder to watch.
    fn run(&mut self, job: Job) {
        if let JobKind::Idle { folder, path } = &job.kind {
            self.watch = Some(Watch { job: job.id, folder: *folder, path: path.clone() });
            self.stale = true;
            return;
        }
        let Some(open) = self.connection.as_mut() else { return };
        self.out.report(ConnectionState::Online);
        // The job may open another folder: IDLE must open the watched one again.
        self.stale |= self.watch.is_some();
        let rerun = job.kind.writes() && write::may_run_again(open, &job.kind);
        let now = self.settings.connector.tls().clock().now();
        let out = &mut self.out;
        let done = sync::run(open, &job, &self.settings.sync, now, &mut |event| out.emit(event));
        let tried = self.tries.remove(&job.id).unwrap_or(0).saturating_add(1);
        match done {
            Ok(()) => self.attempt = 0,
            Err(error) => {
                let again = rerun && error.kind().is_retryable() && tried < WRITE_TRIES;
                if !again {
                    self.out.failed(&job, &error);
                }
                if error.ends_connection() {
                    self.connection = None;
                    self.out.report(ConnectionState::Offline);
                }
                if again {
                    self.tries.insert(job.id, tried);
                    self.pending.push_front(job);
                }
            }
        }
    }

    /// Watches the folder: resyncs it when it is stale, waits for a change otherwise. A connection that ends while
    /// watching is reconnected after the back-off; a folder that is gone, or a command refused, ends the watch.
    fn watch(&mut self, queue: &Receiver<Job>) {
        let Some(watch) = self.watch.clone() else { return };
        let done = if self.stale { self.resync(&watch) } else { self.wait_for_change(queue) };
        let Err(error) = done else { return };
        self.out.say(Some(watch.job), EventKind::Failed { kind: error.kind(), folder: Some(watch.folder) });
        if error.ends_connection() {
            self.connection = None;
            self.out.report(ConnectionState::Offline);
            self.back_off(queue);
        } else {
            self.watch = None;
        }
    }

    /// A sync of the watched folder from the state the last sync left, its events under the Idle job's id.
    fn resync(&mut self, watch: &Watch) -> Result<(), NetError> {
        let Some(open) = self.connection.as_mut() else { return Ok(()) };
        let known = self.out.states.get(&watch.folder).copied();
        let job = Job::new(watch.job, self.account.id(), JobKind::SyncFolder { folder: watch.folder, path: watch.path.clone(), known });
        let now = self.settings.connector.tls().clock().now();
        let out = &mut self.out;
        sync::run(open, &job, &self.settings.sync, now, &mut |event| out.emit(event))?;
        self.stale = false;
        self.attempt = 0;
        Ok(())
    }

    /// IDLE in the watched folder, or a poll without it, until something wakes the worker.
    fn wait_for_change(&mut self, queue: &Receiver<Job>) -> Result<(), NetError> {
        let Some(open) = self.connection.as_mut() else { return Ok(()) };
        self.out.report(ConnectionState::Watching);
        let wake = if self.idle_refused || !open.offers_idle() {
            watch::poll(&self.settings.watch, queue)
        } else {
            watch::idle(open, &self.settings.watch, queue)?
        };
        self.idle_refused |= wake.refused;
        self.stale |= wake.changed;
        self.open &= !wake.closed;
        if let Some(job) = wake.job {
            self.take(job);
        }
        Ok(())
    }
}

/// A sign-in that ended so is sent again only by a check the owner started: refused, or sent and never answered.
const fn refuses_the_password(kind: ErrorKind) -> bool {
    matches!(kind, ErrorKind::LoginRefused | ErrorKind::LoginUnanswered)
}

/// A signed-in connection for `account`, with QRESYNC on and compressed where the server offers them; the password
/// is asked for first, so none means no connection at all.
fn sign_in(account: &Account, settings: &WorkerSettings, credentials: &dyn Credentials) -> Result<ImapConnection, NetError> {
    let password = credentials.password(account).ok_or_else(|| NetError::new(ErrorKind::CredentialMissing, "no password stored"))?;
    let mut connection = ImapConnection::open(account.imap(), &settings.connector)?;
    connection.login(account.username(), &password)?;
    drop(password);
    connection.enable_qresync()?;
    if settings.compress {
        connection.compress()?;
    }
    Ok(connection)
}
