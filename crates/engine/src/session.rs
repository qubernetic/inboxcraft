// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Session`: the one place commands run and events land.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use inboxcraft_model::{
    AccountId, CalendarJob, CheckStep, ConnectionState, ErrorKind, Event, EventKind, FolderId, Job, JobId, JobKind, MessageId, Role, RoleMap,
    ScheduleId, StepReport, UidValidity,
};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{Clock, Offset, Offsets, Timestamp};
use serde_json::Value;

use crate::ScreenerMode;
use crate::act::Effects;
use crate::appearance::Appearance;
use crate::draft;
use crate::notify::{self, Notice, Settings};
use crate::schedule::{self, Fired};
use crate::send::Hold;
use crate::undo::{Act, Step};
use crate::writes::{Settled, Writes};
use crate::{Caller, EngineError, Registry, error};

/// A command's answer and the changes it asks for. The session makes the changes only after the command returned.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    value: Value,
    batch: Batch,
    jobs: Vec<Pending>,
    /// Senders (lower case) whose remote pictures load without asking from now on (`true`) or no longer (`false`).
    senders: Vec<(String, bool)>,
    /// Accounts removed: the session forgets what it knows of them and drops their queued jobs.
    removed: Vec<AccountId>,
    /// The appearance choices from now on.
    appearance: Option<Appearance>,
    /// What the command does to the undo history.
    undo: Option<Step>,
    /// The screener's mode from now on.
    screener: Option<ScreenerMode>,
    /// The notification choices from now on.
    notify: Option<Settings>,
    /// Whether a pin outlasts done from now on.
    pins_outlast_done: Option<bool>,
    /// How long every send is held from now on.
    hold: Option<Hold>,
    /// Held jobs withdrawn: their removal from the store's queue is in the batch.
    withdrawn: Vec<JobId>,
}

/// A job before it has an id.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pending {
    pub(crate) account: AccountId,
    pub(crate) kind: JobKind,
    /// The message a `FetchBody` job is for, so the same body is not asked for twice while one job is out.
    pub(crate) message: Option<MessageId>,
    /// Kept in the store's job queue until it ends, so a restart runs it again.
    pub(crate) keep: bool,
    /// A kept job is held back until then before the worker gets it, so it can still be withdrawn: a delete waiting
    /// out its undo window. A restart hands it out at once.
    pub(crate) until: Option<Timestamp>,
}

impl Outcome {
    /// The answer `value`, with nothing to change.
    #[must_use]
    pub fn new(value: Value) -> Self {
        Self {
            value,
            batch: Batch::new(),
            jobs: Vec::new(),
            senders: Vec::new(),
            removed: Vec::new(),
            appearance: None,
            undo: None,
            screener: None,
            notify: None,
            pins_outlast_done: None,
            hold: None,
            withdrawn: Vec::new(),
        }
    }

    /// The outcome with `change` to make in the store.
    #[must_use]
    pub fn with_change(mut self, change: Change) -> Self {
        self.batch.push(change);
        self
    }

    /// The outcome with a job for the `net` worker of `account`.
    #[must_use]
    pub fn with_job(mut self, account: AccountId, kind: JobKind) -> Self {
        self.jobs.push(Pending { account, kind, message: None, keep: false, until: None });
        self
    }

    /// The outcome with a job for the `net` worker of `account` that is kept in the store's job queue, in the same
    /// batch as the outcome's changes, until its last event comes: a restart before then runs it again. Only a job
    /// with a last event is kept ([`keepable`]); a check or a watch runs in this session only.
    #[must_use]
    pub fn with_kept_job(mut self, account: AccountId, kind: JobKind) -> Self {
        self.jobs.push(Pending { account, kind, message: None, keep: true, until: None });
        self
    }

    /// The outcome with an act's store changes and its kept jobs.
    pub(crate) fn with_effects(mut self, effects: Effects) -> Self {
        for change in effects.changes {
            self.batch.push(change);
        }
        for (account, kind) in effects.jobs {
            self = self.with_kept_job(account, kind);
        }
        self
    }

    /// The outcome with a job for the `net` worker of `account` that is kept in the store's queue at once but held
    /// back from the worker until `until` (see [`Session::tick`]); [`Outcome::with_withdrawn`] takes it back before then.
    pub(crate) fn with_held_job(mut self, account: AccountId, kind: JobKind, until: Timestamp) -> Self {
        self.jobs.push(Pending { account, kind, message: None, keep: true, until: Some(until) });
        self
    }

    /// The outcome taking the held job `job` back: off the store's queue and never handed to the worker.
    pub(crate) fn with_withdrawn(mut self, job: JobId) -> Self {
        self.batch.push(Change::RemoveJob(job));
        self.withdrawn.push(job);
        self
    }

    /// The outcome doing `step` to the undo history.
    pub(crate) fn with_undo(mut self, step: Step) -> Self {
        self.undo = Some(step);
        self
    }

    /// The answer, to add to.
    pub(crate) fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    /// The outcome letting the remote pictures of `sender` (in lower case) load without asking, or no longer.
    pub(crate) fn with_sender(mut self, sender: String, allowed: bool) -> Self {
        self.senders.push((sender, allowed));
        self
    }

    /// The outcome changing the appearance choices to `appearance`.
    pub(crate) fn with_appearance(mut self, appearance: Appearance) -> Self {
        self.appearance = Some(appearance);
        self
    }

    /// The outcome changing the screener's mode to `mode`.
    pub(crate) fn with_screener(mut self, mode: ScreenerMode) -> Self {
        self.screener = Some(mode);
        self
    }

    /// The outcome changing the notification choices to `settings`.
    pub(crate) fn with_notify(mut self, settings: Settings) -> Self {
        self.notify = Some(settings);
        self
    }

    /// The outcome keeping a pin through done (`true`) or taking it off from now on.
    pub(crate) fn with_pins_outlast_done(mut self, keep: bool) -> Self {
        self.pins_outlast_done = Some(keep);
        self
    }

    /// The outcome holding every send for `hold` from now on.
    pub(crate) fn with_hold(mut self, hold: Hold) -> Self {
        self.hold = Some(hold);
        self
    }

    /// The outcome removing `account` from the store, and from what the session knows and has queued.
    pub(crate) fn with_removed(mut self, account: AccountId) -> Self {
        self.batch.push(Change::RemoveAccount(account));
        self.removed.push(account);
        self
    }

    /// The outcome with a `FetchBody` job for `message`, unless one is already out.
    pub(crate) fn with_fetch(mut self, account: AccountId, kind: JobKind, message: MessageId) -> Self {
        self.jobs.push(Pending { account, kind, message: Some(message), keep: false, until: None });
        self
    }

    /// The answer.
    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// What a command may read: the store, the session's state, the time, the computer's offsets and who asked.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    store: &'a dyn Storage,
    state: &'a State,
    now: Timestamp,
    offsets: &'a dyn Offsets,
    caller: Caller,
}

impl<'a> Context<'a> {
    /// The store.
    #[must_use]
    pub fn store(&self) -> &'a dyn Storage {
        self.store
    }

    /// The time the command runs at.
    #[must_use]
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// The computer's offset from UTC over time, for anything in the owner's wall-clock time.
    #[must_use]
    pub fn offsets(&self) -> &'a dyn Offsets {
        self.offsets
    }

    /// The id for a new schedule entry: past every entry the session has held. A command that adds two entries
    /// counts on from it.
    #[must_use]
    pub fn next_schedule_id(&self) -> ScheduleId {
        ScheduleId::new(self.state.next_schedule)
    }

    /// Who asked for the command; [`Caller::Agent`] while an event from a worker is applied, which nobody asked for.
    #[must_use]
    pub fn caller(&self) -> Caller {
        self.caller
    }

    pub(crate) fn state(&self) -> &'a State {
        self.state
    }
}

/// What the session knows besides the store: lost on a restart and rebuilt from the store and the next sync.
#[derive(Debug, Clone, Default)]
pub(crate) struct State {
    pub(crate) accounts: BTreeMap<AccountId, AccountState>,
    /// The UIDVALIDITY the UIDs we hold of each folder belong to.
    pub(crate) validity: BTreeMap<FolderId, UidValidity>,
    /// The `FetchBody` jobs out, by job.
    pub(crate) fetching: BTreeMap<JobId, MessageId>,
    /// The messages whose body came back too large to keep; not asked for again in this session.
    pub(crate) too_large: BTreeSet<MessageId>,
    /// The senders (addresses in lower case) whose remote pictures the owner lets load without asking. Kept for the
    /// session only until settings are kept on disk.
    pub(crate) remote_senders: BTreeSet<String>,
    /// The owner's appearance choices; the front end keeps them on disk and hands them back at the start.
    pub(crate) appearance: Appearance,
    /// What the screener does with the first mail of an unknown sender; the front end keeps it on disk and hands it
    /// back at the start.
    pub(crate) screener: ScreenerMode,
    /// The owner's notification choices; the front end keeps them on disk and hands them back at the start.
    pub(crate) notify: Settings,
    /// Whether done leaves a pin on (`pin.set`); the front end keeps it on disk and hands it back at the start.
    pub(crate) pins_outlast_done: bool,
    /// How long every send is held (`send.set`); the front end keeps it on disk and hands it back at the start.
    pub(crate) hold: Hold,
    /// What to tell the owner, for the front end to take ([`Session::take_notices`]).
    pub(crate) notices: VecDeque<Notice>,
    /// The owner's own moves that come back through a sync, to tell nothing of (see [`crate::notify`]).
    pub(crate) quiet: VecDeque<String>,
    /// The id the next added account gets: past every id this session has held, so a removed account's id is not
    /// given again and a late event of its worker cannot land on a new account.
    pub(crate) next_account: u64,
    pub(crate) next_folder: u64,
    pub(crate) next_message: u64,
    pub(crate) next_job: u64,
    pub(crate) next_schedule: u64,
    pub(crate) next_draft: u64,
    pub(crate) next_calendar: u64,
    pub(crate) next_event: u64,
    /// The jobs in the store's queue: kept until their last event.
    pub(crate) kept: BTreeSet<JobId>,
    /// Kept jobs held back from the workers until a time, in the order they came (see [`Outcome::with_held_job`]).
    pub(crate) held: Vec<(Timestamp, Job)>,
    /// The calendar writes out, one for each event, and the ones waiting for them (see [`crate::writes`]).
    pub(crate) writes: Writes,
    /// The acts to undo, the newest last.
    pub(crate) history: Vec<Act>,
    /// Whether the store is the invented sample mailbox ([`Session::sample`]), where the `sample.*` commands run.
    pub(crate) sample: bool,
}

/// What the session knows of one account.
#[derive(Debug, Clone, Default)]
pub(crate) struct AccountState {
    /// The folder roles: SPECIAL-USE, then names, then the owner.
    pub(crate) roles: RoleMap,
    /// The roles no folder has, to ask the owner about.
    pub(crate) ask: Vec<Role>,
    pub(crate) connection: Option<ConnectionState>,
    /// Whether the server, at the last sign-in, removes only the messages named (UID EXPUNGE); `false` until it said.
    pub(crate) uid_expunge: bool,
    /// The last failure, until the account is online again.
    pub(crate) failure: Option<(ErrorKind, Option<FolderId>)>,
    /// The last account check, running or done.
    pub(crate) check: Option<Check>,
}

/// An account check: every step as it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Check {
    /// The Check job while the check runs; `None` once its last step came.
    pub(crate) job: Option<JobId>,
    /// Every step of [`CheckStep::ALL`], in order.
    pub(crate) steps: Vec<StepReport>,
}

impl Check {
    pub(crate) fn is_running(&self) -> bool {
        self.job.is_some()
    }
}

impl State {
    /// The state of what `store` holds.
    fn load(store: &dyn Storage) -> Result<Self, EngineError> {
        let mut state = Self {
            next_account: 1,
            next_folder: 1,
            next_message: 1,
            next_job: 1,
            next_schedule: 1,
            next_draft: 1,
            next_calendar: 1,
            next_event: 1,
            ..Self::default()
        };
        for account in store.accounts()? {
            let folders = store.folders(account.id())?;
            let mut roles = RoleMap::default();
            let ask = roles.detect(&folders);
            for folder in &folders {
                state.next_folder = state.next_folder.max(folder.id().get().saturating_add(1));
                if let Some(folder_state) = store.folder_state(folder.id())? {
                    state.validity.insert(folder.id(), folder_state.uid_validity());
                }
                for entry in store.messages(folder.id())? {
                    state.next_message = state.next_message.max(entry.message().id().get().saturating_add(1));
                }
            }
            for draft in store.drafts(account.id())? {
                state.next_draft = state.next_draft.max(draft.id.get().saturating_add(1));
            }
            for calendar in store.calendars(account.id())? {
                state.next_calendar = state.next_calendar.max(calendar.id.get().saturating_add(1));
                for event in store.calendar_events(calendar.id)? {
                    state.next_event = state.next_event.max(event.id.get().saturating_add(1));
                }
            }
            state.next_account = state.next_account.max(account.id().get().saturating_add(1));
            state.accounts.insert(account.id(), AccountState { roles, ask, ..AccountState::default() });
        }
        for entry in store.schedule()? {
            state.next_schedule = state.next_schedule.max(entry.id.get().saturating_add(1));
        }
        for job in store.queued_jobs()? {
            state.next_job = state.next_job.max(job.id.get().saturating_add(1));
            state.kept.insert(job.id);
        }
        Ok(state)
    }

    /// The id for an account to add: past every account in `held` and every id given before.
    pub(crate) fn next_account(&self, held: &[inboxcraft_model::Account]) -> AccountId {
        let past = held.iter().map(|account| account.id().get().saturating_add(1)).max().unwrap_or(1);
        AccountId::new(self.next_account.max(past))
    }

    pub(crate) fn folder_id(&mut self) -> FolderId {
        let id = FolderId::new(self.next_folder);
        self.next_folder = self.next_folder.saturating_add(1);
        id
    }

    pub(crate) fn message_id(&mut self) -> MessageId {
        let id = MessageId::new(self.next_message);
        self.next_message = self.next_message.saturating_add(1);
        id
    }

    fn job_id(&mut self) -> JobId {
        let id = JobId::new(self.next_job);
        self.next_job = self.next_job.saturating_add(1);
        id
    }
}

/// Whether an applied event changed anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The event was applied.
    Changed,
    /// The event was stale or named something the session does not hold, and was dropped.
    Dropped,
}

/// The engine: commands in by id, jobs out to the `net` workers, events back in.
///
/// Every command runs through [`Session::run`]. A command only reads; the session makes the store changes it
/// returns as one batch and queues its jobs, so a command that fails changes nothing. A command that panics, despite
/// the rules, becomes [`EngineError::Panicked`] and also changes nothing.
pub struct Session {
    store: Box<dyn Storage>,
    clock: Box<dyn Clock>,
    offsets: Box<dyn Offsets>,
    /// When [`Session::start`] ran: a schedule entry due before then was missed.
    started: Option<Timestamp>,
    registry: Registry,
    state: State,
    jobs: Vec<Job>,
    /// The jobs an earlier run left in the store's queue, for [`Session::start`].
    replay: Vec<Job>,
    /// What [`Session::next_due`] said, until something changes (`None`: ask again).
    due: Option<Option<Timestamp>>,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session").field("registry", &self.registry).field("jobs", &self.jobs.len()).finish_non_exhaustive()
    }
}

impl Session {
    /// A session over `store` with InboxCraft's own commands.
    ///
    /// # Errors
    ///
    /// A store that cannot be read.
    pub fn new(store: Box<dyn Storage>, clock: Box<dyn Clock>) -> Result<Self, EngineError> {
        Self::with_registry(store, clock, Registry::builtin()?)
    }

    /// A session over `store` with the commands of `registry`.
    ///
    /// # Errors
    ///
    /// A store that cannot be read.
    pub fn with_registry(store: Box<dyn Storage>, clock: Box<dyn Clock>, registry: Registry) -> Result<Self, EngineError> {
        let mut store = store;
        let (replay, stray): (Vec<Job>, Vec<Job>) = store.queued_jobs()?.into_iter().partition(|job| keepable(&job.kind));
        if !stray.is_empty() {
            // No last event would take them off the queue, and a check could sign in again without the owner asking.
            let batch = stray.iter().fold(Batch::new(), |batch, job| batch.with(Change::RemoveJob(job.id)));
            store.apply(&batch)?;
        }
        let cut = crate::send::cut_off(&*store)?;
        if !cut.is_empty() {
            store.apply(&cut)?;
        }
        let state = State::load(&*store)?;
        Ok(Self { store, clock, offsets: Box::new(Offset::UTC), started: None, registry, state, jobs: Vec::new(), replay, due: None })
    }

    /// A session over the invented sample mailbox in `store`, with InboxCraft's own commands: the `sample.*` commands
    /// run in it and nowhere else. Nothing in it reaches a server; the front end starts no worker for it.
    ///
    /// # Errors
    ///
    /// A store that cannot be read.
    pub fn sample(store: Box<dyn Storage>, clock: Box<dyn Clock>) -> Result<Self, EngineError> {
        let mut session = Self::new(store, clock)?;
        session.state.sample = true;
        Ok(session)
    }

    /// Whether this is a session over the sample mailbox ([`Session::sample`]).
    #[must_use]
    pub fn is_sample(&self) -> bool {
        self.state.sample
    }

    /// The commands.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &dyn Storage {
        &*self.store
    }

    /// The store, to change as a test sets the scene.
    #[cfg(test)]
    pub(crate) fn store_mut(&mut self) -> &mut dyn Storage {
        &mut *self.store
    }

    /// The owner's appearance choices, as the `ui.*.set` commands left them.
    #[must_use]
    pub fn appearance(&self) -> &Appearance {
        &self.state.appearance
    }

    /// Takes the appearance choices kept from an earlier run, read with [`Appearance::from_value`].
    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.state.appearance = appearance;
    }

    /// The screener's mode, as `screener.mode` left it.
    #[must_use]
    pub fn screener(&self) -> ScreenerMode {
        self.state.screener
    }

    /// Takes the screener's mode kept from an earlier run.
    pub fn set_screener(&mut self, mode: ScreenerMode) {
        self.state.screener = mode;
    }

    /// The notification choices, as `notify.set` left them.
    #[must_use]
    pub fn notify_settings(&self) -> &Settings {
        &self.state.notify
    }

    /// Takes the notification choices kept from an earlier run, read with [`Settings::from_value`].
    pub fn set_notify_settings(&mut self, settings: Settings) {
        self.state.notify = settings;
    }

    /// Whether done leaves a pin on, as `pin.set` left it.
    #[must_use]
    pub fn pins_outlast_done(&self) -> bool {
        self.state.pins_outlast_done
    }

    /// Takes the pin setting kept from an earlier run.
    pub fn set_pins_outlast_done(&mut self, keep: bool) {
        self.state.pins_outlast_done = keep;
    }

    /// How long every send is held before it goes, as `send.set` left it.
    #[must_use]
    pub fn send_hold(&self) -> Hold {
        self.state.hold
    }

    /// Takes the hold kept from an earlier run.
    pub fn set_send_hold(&mut self, hold: Hold) {
        self.state.hold = hold;
    }

    /// What to tell the owner since the last call, in order: more than [`notify::MAX_SINGLE`] new messages of one
    /// account as one count.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        notify::collapse(std::mem::take(&mut self.state.notices))
    }

    /// Takes the computer's offset from UTC over time; until then the session works in UTC.
    pub fn set_offsets(&mut self, offsets: Box<dyn Offsets>) {
        self.offsets = offsets;
    }

    /// Runs the command `id` with `params` for the owner.
    ///
    /// # Errors
    ///
    /// As [`Session::run_as`].
    pub fn run(&mut self, id: &str, params: &Value) -> Result<Value, EngineError> {
        self.run_as(Caller::Owner, id, params)
    }

    /// Runs the command `id` with `params` for `caller` and returns its answer.
    ///
    /// # Errors
    ///
    /// [`EngineError::UnknownCommand`] (also for a command that was refused as unclassified), [`EngineError::NotAllowed`] when the
    /// command's class does not let `caller` run it, the command's own error, a store failure, or
    /// [`EngineError::Panicked`]. On every error the session is as it was.
    pub fn run_as(&mut self, caller: Caller, id: &str, params: &Value) -> Result<Value, EngineError> {
        // The registry holds classified commands only.
        let (spec, class) = self.registry.lookup(id).ok_or_else(|| EngineError::UnknownCommand(error::clean(id)))?;
        if !class.allows(caller) {
            return Err(EngineError::NotAllowed { command: spec.id, class });
        }
        let context = Context { store: &*self.store, state: &self.state, now: self.clock.now(), offsets: &*self.offsets, caller };
        let outcome = catch_unwind(AssertUnwindSafe(|| (spec.run)(&context, params))).map_err(|_| EngineError::Panicked(spec.id))??;
        let Outcome { value, batch, jobs, senders, removed, appearance, undo, screener, notify, pins_outlast_done, hold, withdrawn } = outcome;
        self.commit(spec.id, None, &batch, jobs)?;
        for job in withdrawn {
            self.state.held.retain(|(_, held)| held.id != job);
            self.state.writes.withdraw(job);
            self.state.kept.remove(&job);
        }
        if let Some(step) = undo {
            crate::undo::take(&mut self.state.history, step);
        }
        let mut made = BTreeSet::new();
        for change in batch.changes() {
            match change {
                Change::PutAccount(account) => self.state.next_account = self.state.next_account.max(account.id().get().saturating_add(1)),
                Change::PutScheduled(entry) => self.state.next_schedule = self.state.next_schedule.max(entry.id.get().saturating_add(1)),
                Change::PutDraft(draft) => self.state.next_draft = self.state.next_draft.max(draft.id.get().saturating_add(1)),
                Change::PutCalendarEvent(event) => self.state.next_event = self.state.next_event.max(event.id.get().saturating_add(1)),
                Change::PutFolder(folder) => {
                    self.state.next_folder = self.state.next_folder.max(folder.id().get().saturating_add(1));
                    made.insert(folder.account());
                }
                _ => {}
            }
        }
        // A folder a command made takes its role at once.
        for account in made {
            // The batch is made: a store that cannot list the folders now leaves the roles to the next folder list.
            let Ok(folders) = self.store.folders(account) else { continue };
            let held = self.state.accounts.entry(account).or_default();
            let questions = held.roles.detect(&folders);
            held.ask.extend(questions);
            held.ask.retain(|role| held.roles.folder(*role).is_none());
        }
        if let Some(mode) = screener {
            self.state.screener = mode;
        }
        if let Some(settings) = notify {
            self.state.notify = settings;
        }
        if let Some(keep) = pins_outlast_done {
            self.state.pins_outlast_done = keep;
        }
        if let Some(hold) = hold {
            self.state.hold = hold;
        }
        for account in removed {
            self.state.accounts.remove(&account);
            self.state.notify.forget(account);
            self.state.notices.retain(|notice| notice.account() != account);
            self.jobs.retain(|job| job.account != account);
            self.state.held.retain(|(_, job)| job.account != account);
            self.state.writes.forget(account);
        }
        for (sender, allowed) in senders {
            if allowed {
                self.state.remote_senders.insert(sender);
            } else {
                self.state.remote_senders.remove(&sender);
            }
        }
        if let Some(appearance) = appearance {
            self.state.appearance = appearance;
        }
        Ok(value)
    }

    /// Hands the jobs an earlier run left in the store's queue over again, then asks every account's worker for its
    /// folder list, and each account with a CalDAV address for its calendars; the answers bring the folder and
    /// calendar syncs. A check still running is over: the worker that ran it is gone,
    /// so its last step never comes, and the account can be checked again.
    ///
    /// A schedule entry due before the start was missed: the next [`Session::tick`] fires it as such.
    pub fn start(&mut self) {
        self.started = Some(self.clock.now());
        for job in std::mem::take(&mut self.replay) {
            self.hand_out(job);
        }
        for held in self.state.accounts.values_mut() {
            if let Some(check) = held.check.as_mut() {
                check.job = None;
            }
        }
        let accounts: Vec<AccountId> = self.state.accounts.keys().copied().collect();
        for account in accounts {
            self.queue(Pending { account, kind: JobKind::SyncFolderList, message: None, keep: false, until: None });
        }
        // A store that cannot list the accounts now leaves the calendars to `calendar.sync`.
        for account in self.store.accounts().unwrap_or_default() {
            if let Some(url) = account.caldav() {
                self.queue(Pending { account: account.id(), kind: crate::calendar::discover(url), message: None, keep: false, until: None });
            }
        }
    }

    /// Hands the held jobs whose time has come to the workers (a delete whose undo window is over), then fires every
    /// schedule entry whose time has come, in time order: each leaves the schedule, with what it changes
    /// (see [`crate::schedule`]: a snoozed message comes back to the inbox), in one batch, and the jobs that make it on
    /// the server. A returned snooze and a follow-up reminder tell the owner as
    /// [`Session::take_notices`] hands them over, unless they turned that off. Then hands each draft that has been
    /// still long enough to its account's worker (see `crate::draft`). The front end calls it at
    /// [`Session::next_due`] and after [`Session::start`].
    ///
    /// # Errors
    ///
    /// A store failure, or [`EngineError::Panicked`]; nothing fires then, and the next tick tries again.
    pub fn tick(&mut self) -> Result<Vec<Fired>, EngineError> {
        let now = self.clock.now();
        let (due, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.state.held).into_iter().partition(|(until, _)| *until <= now);
        self.state.held = waiting;
        for (_, job) in due {
            self.hand_out(job);
        }
        let context = Context { store: &*self.store, state: &self.state, now: self.clock.now(), offsets: &*self.offsets, caller: Caller::Agent };
        let started = self.started;
        let planned = catch_unwind(AssertUnwindSafe(|| {
            let plan = schedule::due(&context, started)?;
            let notices = notify::fired(context.store, &context.state.notify, &plan.fired, &plan.sent)?;
            Ok::<_, EngineError>((plan, notices))
        }));
        let (plan, notices) = planned.map_err(|_| EngineError::Panicked("schedule"))??;
        self.commit("schedule", None, &plan.batch, plan.jobs)?;
        for notice in notices {
            notify::push(&mut self.state, notice);
        }
        let context = Context { store: &*self.store, state: &self.state, now: self.clock.now(), offsets: &*self.offsets, caller: Caller::Agent };
        let (batch, jobs) = catch_unwind(AssertUnwindSafe(|| draft::uploads(&context))).map_err(|_| EngineError::Panicked("draft"))??;
        self.commit("draft", None, &batch, jobs)?;
        Ok(plan.fired)
    }

    /// Runs [`Session::tick`] when something is due ([`Session::next_due`]), for a front end that asks every frame; the
    /// time it next asks the store is kept until a command or an event changes something. `None` when nothing was due.
    ///
    /// # Errors
    ///
    /// As [`Session::tick`].
    pub fn tick_if_due(&mut self) -> Result<Option<Vec<Fired>>, EngineError> {
        let due = self.due_at()?;
        if !due.is_some_and(|due| due <= self.clock.now()) {
            return Ok(None);
        }
        self.due = None;
        self.tick().map(Some)
    }

    /// [`Session::next_due`], kept until a command or an event changes something, for a front end that asks every
    /// frame when to wake next.
    ///
    /// # Errors
    ///
    /// A store failure.
    pub fn due_at(&mut self) -> Result<Option<Timestamp>, EngineError> {
        if let Some(due) = self.due {
            return Ok(due);
        }
        let due = self.next_due()?;
        self.due = Some(due);
        Ok(due)
    }

    /// When the next schedule entry, draft copy or held job is due, if any.
    ///
    /// # Errors
    ///
    /// A store failure.
    pub fn next_due(&self) -> Result<Option<Timestamp>, EngineError> {
        let entry = self.store.schedule()?.into_iter().map(|entry| entry.due).min();
        let upload = draft::next_upload(&*self.store, &self.state)?;
        let held = self.state.held.iter().map(|(until, _)| *until).min();
        Ok(entry.into_iter().chain(upload).chain(held).min())
    }

    /// The jobs queued since the last call, in order, for the `net` workers.
    pub fn take_jobs(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.jobs)
    }

    /// Applies an event from a `net` worker.
    ///
    /// # Errors
    ///
    /// A store failure, or [`EngineError::Panicked`]; the session is then as it was.
    pub fn apply(&mut self, event: &Event) -> Result<Applied, EngineError> {
        self.due = None;
        let context = Context { store: &*self.store, state: &self.state, now: self.clock.now(), offsets: &*self.offsets, caller: Caller::Agent };
        let plan = catch_unwind(AssertUnwindSafe(|| crate::sync::plan(&context, event))).map_err(|_| EngineError::Panicked("event"))??;
        let ended = event.job.filter(|job| self.state.kept.contains(job) && ends_job(&event.kind));
        // The calendar writes the event lets go, with the new tags of the ones that still wait, in the same batch.
        let (mut writes, mut held) = (self.state.writes.clone(), self.state.held.clone());
        let Settled { jobs: released, changes: retagged } = writes.settle(&mut held, event);
        let applied = match plan {
            Some(mut plan) => {
                if let Some(job) = ended {
                    plan.batch.push(Change::RemoveJob(job));
                }
                for change in retagged {
                    plan.batch.push(change);
                }
                plan.state.writes = writes;
                plan.state.held = held;
                self.commit("event", Some(plan.state), &plan.batch, plan.jobs)?;
                Applied::Changed
            }
            None => {
                let batch = ended.into_iter().map(Change::RemoveJob).chain(retagged).fold(Batch::new(), Batch::with);
                if !batch.is_empty() {
                    self.commit("event", None, &batch, Vec::new())?;
                }
                self.state.writes = writes;
                self.state.held = held;
                Applied::Dropped
            }
        };
        if let Some(job) = ended {
            self.state.kept.remove(&job);
        }
        self.jobs.extend(released);
        Ok(applied)
    }

    /// Makes `batch` with the kept jobs among `jobs` added to the store's queue, then takes `state` and queues `jobs`;
    /// nothing of it when the batch fails.
    fn commit(&mut self, what: &'static str, state: Option<State>, batch: &Batch, mut jobs: Vec<Pending>) -> Result<(), EngineError> {
        self.due = None;
        for pending in &mut jobs {
            pending.keep &= keepable(&pending.kind);
        }
        let mut next_job = self.state.next_job;
        let mut kept = Vec::new();
        for pending in jobs.iter().filter(|pending| pending.keep) {
            kept.push(Job::new(JobId::new(next_job), pending.account, pending.kind.clone()));
            next_job = next_job.saturating_add(1);
        }
        let with_kept;
        let batch = if kept.is_empty() {
            batch
        } else {
            with_kept = kept.iter().fold(batch.clone(), |batch, job| batch.with(Change::QueueJob(job.clone())));
            &with_kept
        };
        if !batch.is_empty() {
            let store = &mut self.store;
            catch_unwind(AssertUnwindSafe(|| store.apply(batch))).map_err(|_| EngineError::Panicked(what))??;
        }
        if let Some(state) = state {
            self.state = state;
        }
        self.state.next_job = self.state.next_job.max(next_job);
        let mut kept = kept.into_iter();
        for pending in jobs {
            let job = if pending.keep { kept.next() } else { None };
            match job {
                Some(job) => {
                    self.state.kept.insert(job.id);
                    match pending.until {
                        Some(until) => self.state.held.push((until, job)),
                        None => self.hand_out(job),
                    }
                }
                None => self.queue(pending),
            }
        }
        Ok(())
    }

    /// Hands the kept `job` to its worker, unless it is a calendar write that waits for an earlier one.
    fn hand_out(&mut self, job: Job) {
        if let Some(job) = self.state.writes.hand_out(job) {
            self.jobs.push(job);
        }
    }

    fn queue(&mut self, pending: Pending) {
        if let Some(message) = pending.message
            && self.state.fetching.values().any(|out| *out == message)
        {
            return;
        }
        let id = self.state.job_id();
        if let Some(message) = pending.message {
            self.state.fetching.insert(id, message);
        }
        if matches!(pending.kind, JobKind::Check { .. }) {
            self.state.accounts.entry(pending.account).or_default().check = Some(Check { job: Some(id), steps: CheckStep::plan() });
        }
        self.jobs.push(Job::new(id, pending.account, pending.kind));
    }
}

/// Whether `kind` is the last event of the job that caused it: the answer of a folder list, folder sync, body fetch
/// or write, or its failure.
pub(crate) fn ends_job(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Folders { .. }
            | EventKind::FolderSynced { .. }
            | EventKind::BodyFetched { .. }
            | EventKind::FlagsStored { .. }
            | EventKind::Moved { .. }
            | EventKind::Expunged { .. }
            | EventKind::Appended { .. }
            | EventKind::FolderCreated { .. }
            | EventKind::Calendar(_)
            | EventKind::Failed { .. }
    )
}

/// Whether a job of `kind` may be kept in the store's queue: one with a last event ([`ends_job`]) to take it off. A
/// check ends with its steps and a watch never ends, so either would be handed out again at every start. Of the
/// calendar jobs, the writes are kept; a discovery or a sync is asked for again at the next start.
pub(crate) fn keepable(kind: &JobKind) -> bool {
    kind.writes()
        || matches!(
            kind,
            JobKind::SyncFolderList
                | JobKind::SyncFolder { .. }
                | JobKind::FetchBody { .. }
                | JobKind::Calendar(CalendarJob::Put { .. } | CalendarJob::Delete { .. })
        )
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "queue_tests.rs"]
mod queue_tests;
