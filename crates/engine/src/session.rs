// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Session`: the one place commands run.

use std::collections::BTreeMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use inboxcraft_model::{AccountId, ConnectionState, ErrorKind, FolderId, Job, JobId, JobKind};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{Clock, Timestamp};
use serde_json::Value;

use crate::{Caller, EngineError, Registry, error};

/// A command's answer and the changes it asks for. The session makes the changes only after the command returned.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    value: Value,
    batch: Batch,
    jobs: Vec<Pending>,
}

/// A job before it has an id.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pending {
    pub(crate) account: AccountId,
    pub(crate) kind: JobKind,
}

impl Outcome {
    /// The answer `value`, with nothing to change.
    #[must_use]
    pub fn new(value: Value) -> Self {
        Self { value, batch: Batch::new(), jobs: Vec::new() }
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
        self.jobs.push(Pending { account, kind });
        self
    }

    /// The answer.
    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// What a command may read: the store, the session's state and the time.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    store: &'a dyn Storage,
    state: &'a State,
    now: Timestamp,
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

    pub(crate) fn state(&self) -> &'a State {
        self.state
    }
}

/// What the session knows besides the store: lost on a restart and rebuilt from the store and the next sync.
#[derive(Debug, Clone, Default)]
pub(crate) struct State {
    pub(crate) accounts: BTreeMap<AccountId, AccountState>,
    pub(crate) next_job: u64,
}

/// What the session knows of one account.
#[derive(Debug, Clone, Default)]
pub(crate) struct AccountState {
    pub(crate) connection: Option<ConnectionState>,
    /// The last failure, until the account is online again.
    pub(crate) failure: Option<(ErrorKind, Option<FolderId>)>,
}

impl State {
    /// The state of what `store` holds.
    fn load(store: &dyn Storage) -> Result<Self, EngineError> {
        let mut state = Self { next_job: 1, ..Self::default() };
        for account in store.accounts()? {
            state.accounts.insert(account.id(), AccountState::default());
        }
        Ok(state)
    }

    fn job_id(&mut self) -> JobId {
        let id = JobId::new(self.next_job);
        self.next_job = self.next_job.saturating_add(1);
        id
    }
}

/// The engine: commands in by id, jobs out to the `net` workers.
///
/// Every command runs through [`Session::run`]. A command only reads; the session makes the store changes it
/// returns as one batch and queues its jobs, so a command that fails changes nothing. A command that panics, despite
/// the rules, becomes [`EngineError::Panicked`] and also changes nothing.
pub struct Session {
    store: Box<dyn Storage>,
    clock: Box<dyn Clock>,
    registry: Registry,
    state: State,
    jobs: Vec<Job>,
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
        let state = State::load(&*store)?;
        Ok(Self { store, clock, registry, state, jobs: Vec::new() })
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
        let context = Context { store: &*self.store, state: &self.state, now: self.clock.now() };
        let outcome = catch_unwind(AssertUnwindSafe(|| (spec.run)(&context, params))).map_err(|_| EngineError::Panicked(spec.id))??;
        let Outcome { value, batch, jobs } = outcome;
        self.commit(spec.id, &batch, jobs)?;
        Ok(value)
    }

    /// The jobs queued since the last call, in order, for the `net` workers.
    pub fn take_jobs(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.jobs)
    }

    /// Makes `batch`, then queues `jobs`; nothing of it when the batch fails.
    fn commit(&mut self, what: &'static str, batch: &Batch, jobs: Vec<Pending>) -> Result<(), EngineError> {
        if !batch.is_empty() {
            let store = &mut self.store;
            catch_unwind(AssertUnwindSafe(|| store.apply(batch))).map_err(|_| EngineError::Panicked(what))??;
        }
        for job in jobs {
            self.queue(job);
        }
        Ok(())
    }

    fn queue(&mut self, pending: Pending) {
        let id = self.state.job_id();
        self.jobs.push(Job::new(id, pending.account, pending.kind));
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
