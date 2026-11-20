// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The draft keeper: one thread with a connection of its own that writes the drafts a store keeps behind
//! ([`Sqlite::keep_drafts_behind`](super::Sqlite::keep_drafts_behind)), in the order they came, the newest of each
//! draft, one write at a time. Until a draft is on the disk the store's reads see it here.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use inboxcraft_model::{AccountId, Draft, DraftId};
use rusqlite::Connection;

use super::local;
use crate::StoreError;

/// How long the end of a store waits for the drafts still to be written.
pub(super) const CLOSE_WAIT: Duration = Duration::from_secs(3);

/// A keep: its number in the order the keeps came in, and the draft.
type Keep = (u64, Arc<Draft>);

#[derive(Default)]
struct Queue {
    /// Every draft kept and not yet on the disk, the newest of each: what the store's reads see.
    held: BTreeMap<DraftId, Keep>,
    /// What the keeper still has to write, the newest of each draft.
    waiting: BTreeMap<DraftId, Keep>,
    /// The keeper is writing what it took.
    writing: bool,
    /// The number of the next keep.
    next: u64,
    /// The keeps that failed, by draft, not yet told.
    failed: BTreeMap<DraftId, StoreError>,
    /// How many keeps failed since the keeper started.
    failures: u64,
    /// The store ends: the keeper writes what waits, then stops.
    closing: bool,
    /// Held by a [`KeepHold`]: the keeper takes nothing (tests).
    paused: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The store's end of the keeper.
pub(super) struct Keeper {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Keeper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keeper").finish_non_exhaustive()
    }
}

/// While it lives, the keeper writes nothing; what is kept waits (a test's way to see a keep pending).
#[derive(Debug)]
pub struct KeepHold {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared").finish_non_exhaustive()
    }
}

impl Drop for KeepHold {
    fn drop(&mut self) {
        self.shared.lock().paused = false;
        self.shared.changed.notify_all();
    }
}

impl Keeper {
    /// Starts the keeper on its own connection to `database`.
    ///
    /// # Errors
    ///
    /// The connection or the thread could not be made.
    pub(super) fn start(database: &Path) -> Result<Self, StoreError> {
        let conn = super::connect(database)?;
        let shared = Arc::new(Shared { queue: Mutex::new(Queue::default()), changed: Condvar::new() });
        let theirs = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("draft keeper".to_owned())
            .spawn(move || run(&theirs, conn))
            .map_err(|error| StoreError::Io(error.kind()))?;
        Ok(Self { shared })
    }

    /// Hands `drafts` to the keeper; each takes the place of what waits for the same draft.
    pub(super) fn keep(&self, drafts: Vec<Arc<Draft>>) {
        let mut queue = self.shared.lock();
        for draft in drafts {
            let number = queue.next;
            queue.next = number.saturating_add(1);
            queue.held.insert(draft.id, (number, Arc::clone(&draft)));
            queue.waiting.insert(draft.id, (number, draft));
        }
        drop(queue);
        self.shared.changed.notify_all();
    }

    /// The newest keep of draft `id` while it is not on the disk.
    pub(super) fn held(&self, id: DraftId) -> Option<Arc<Draft>> {
        self.shared.lock().held.get(&id).map(|(_, draft)| Arc::clone(draft))
    }

    /// The drafts of `account` kept and not on the disk.
    pub(super) fn held_of(&self, account: AccountId) -> Vec<Arc<Draft>> {
        let queue = self.shared.lock();
        queue.held.values().filter(|(_, draft)| draft.account == account).map(|(_, draft)| Arc::clone(draft)).collect()
    }

    /// Whether nothing kept is still to reach the disk.
    pub(super) fn is_idle(&self) -> bool {
        let queue = self.shared.lock();
        queue.held.is_empty() && queue.waiting.is_empty() && !queue.writing
    }

    /// How many keeps failed so far.
    pub(super) fn failures(&self) -> u64 {
        self.shared.lock().failures
    }

    /// Whether the keep of draft `id` failed and is not yet told.
    pub(super) fn failed(&self, id: DraftId) -> Option<StoreError> {
        self.shared.lock().failed.get(&id).cloned()
    }

    /// Waits up to `within` until what was kept is written.
    ///
    /// # Errors
    ///
    /// [`StoreError::DatabaseBusy`] when it is not written in that time.
    pub(super) fn settle(&self, within: Duration) -> Result<(), StoreError> {
        let busy = |queue: &mut Queue| !queue.waiting.is_empty() || queue.writing;
        let (mut queue, _) = self.shared.changed.wait_timeout_while(self.shared.lock(), within, busy).unwrap_or_else(PoisonError::into_inner);
        if busy(&mut queue) {
            return Err(StoreError::DatabaseBusy("a draft kept was not written in time".to_owned()));
        }
        Ok(())
    }

    /// A keep that failed, with its draft, once.
    pub(super) fn take_failed(&self) -> Option<(DraftId, StoreError)> {
        self.shared.lock().failed.pop_first()
    }

    /// Holds the keeper until the hold is dropped.
    pub(super) fn hold(&self) -> KeepHold {
        self.shared.lock().paused = true;
        KeepHold { shared: Arc::clone(&self.shared) }
    }
}

/// The end of the store waits, at most [`CLOSE_WAIT`], for what was kept to be written, then lets the keeper stop.
impl Drop for Keeper {
    fn drop(&mut self) {
        {
            let mut queue = self.shared.lock();
            queue.closing = true;
        }
        self.shared.changed.notify_all();
        // Nobody is left to tell: a draft not written in time is what the window already waited for and said.
        let _ = self.settle(CLOSE_WAIT);
    }
}

/// The keeper's thread: takes what waits, writes it in one transaction, and tells the store how it went.
fn run(shared: &Shared, mut conn: Connection) {
    loop {
        let taken = {
            let mut queue = shared.lock();
            loop {
                if !queue.waiting.is_empty() && (!queue.paused || queue.closing) {
                    break;
                }
                if queue.closing && queue.waiting.is_empty() {
                    return;
                }
                queue = shared.changed.wait(queue).unwrap_or_else(PoisonError::into_inner);
            }
            queue.writing = true;
            std::mem::take(&mut queue.waiting)
        };
        let mut keeps: Vec<(DraftId, Keep)> = taken.into_iter().collect();
        keeps.sort_by_key(|(_, (number, _))| *number);
        let written = catch_unwind(AssertUnwindSafe(|| write(&mut conn, &keeps)))
            .unwrap_or_else(|_| Err(StoreError::Database("the draft keeper failed".to_owned())));
        let mut queue = shared.lock();
        queue.writing = false;
        for (id, (number, _)) in &keeps {
            // A newer keep of the same draft came meanwhile: it stays held until it is written.
            if queue.held.get(id).is_some_and(|(held, _)| held == number) {
                queue.held.remove(id);
            }
            if let Err(error) = &written {
                queue.failed.insert(*id, error.clone());
                queue.failures = queue.failures.saturating_add(1);
            }
        }
        drop(queue);
        shared.changed.notify_all();
    }
}

/// Writes `keeps` in one transaction, or none of them.
fn write(conn: &mut Connection, keeps: &[(DraftId, Keep)]) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    for (_, (_, draft)) in keeps {
        local::put_draft(&tx, draft)?;
    }
    Ok(tx.commit()?)
}
