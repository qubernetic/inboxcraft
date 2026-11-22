// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The draft keeper: one thread with a connection of its own that writes the drafts a store keeps behind
//! ([`Sqlite::keep_drafts_behind`](super::Sqlite::keep_drafts_behind)), in the order they came, the newest of each
//! draft, one write at a time. Until a draft is on the disk the store's reads see it here. A write that fails keeps
//! the draft here, newest text and all, and is tried again after 1 s, 2 s, 4 s … at most 30 s apart, until it lands
//! or the draft is removed.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use inboxcraft_model::{AccountId, Draft, DraftId};
use inboxcraft_time::{Clock, SystemClock};
use rusqlite::{Connection, TransactionBehavior};

use super::local;
use crate::{KeepWake, StoreError};

/// How long the end of a store waits for the drafts still to be written, when nobody waited for them before.
pub(super) const CLOSE_WAIT: Duration = Duration::from_secs(3);

/// How long one write of the keeper waits for a lock another connection holds (another process on the same store):
/// shorter than every wait for the keeper (3 s), so a write blocked by a lock fails, and is said, within that wait.
pub(super) const LOCK_WAIT: Duration = Duration::from_secs(2);

/// The wait before the first new try of a keep that failed; it doubles with each failure.
const FIRST_RETRY: Duration = Duration::from_secs(1);

/// The longest wait between two tries of a keep that keeps failing.
const LAST_RETRY: Duration = Duration::from_secs(30);

/// The wait before the next try after `tries` failures in a row: 1 s, 2 s, 4 s … at most [`LAST_RETRY`].
fn backoff(tries: u32) -> Duration {
    let doublings = tries.saturating_sub(1).min(5);
    FIRST_RETRY.saturating_mul(1 << doublings).min(LAST_RETRY)
}

/// A keep: its number in the order the keeps came in, and the draft.
type Keep = (u64, Arc<Draft>);

/// A draft whose last write failed: why, how many writes failed in a row, and when (the keeper's clock) the next try
/// is due.
struct Failing {
    error: StoreError,
    tries: u32,
    due: Duration,
}

#[derive(Default)]
struct Queue {
    /// Every draft kept and not yet on the disk, the newest of each: what the store's reads see. A draft whose write
    /// failed stays here until a write of it lands or it is removed.
    held: BTreeMap<DraftId, Keep>,
    /// What the keeper still has to write, the newest of each draft.
    waiting: BTreeMap<DraftId, Keep>,
    /// The keeper is writing what it took.
    writing: bool,
    /// The number of the next keep.
    next: u64,
    /// The drafts whose last write failed and none landed since.
    failing: BTreeMap<DraftId, Failing>,
    /// The failures not yet told: one for each draft that started failing.
    untold: BTreeMap<DraftId, StoreError>,
    /// The store ends: the keeper writes what waits, then stops.
    closing: bool,
    /// How many holds keep the keeper from taking anything: a batch written on the store's own connection, or a test.
    paused: u32,
    /// Someone waited for the keeper ([`Keeper::settle_all`]) and nothing was kept since.
    settled: bool,
    /// What the keeper calls after each write, landed or failed: the window's redraw.
    wake: Option<KeepWake>,
    /// Every write fails (tests).
    refuse: bool,
    /// Every write of these drafts fails, the others land (tests).
    refuse_drafts: BTreeSet<DraftId>,
    /// The keeper's thread ends at once (tests: a keeper gone).
    stop: bool,
    /// How many writes the keeper started (tests).
    tries: u64,
    /// The end of the store waits until the keeper looked at the queue once after it began (tests).
    close_after_a_look: bool,
    /// The keeper looked at the queue since the end of the store began (tests).
    looked_at_the_end: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
    clock: SystemClock,
}

impl Queue {
    /// Hands each draft of `ids` whose write failed (each failing one with `None`) to the keeper again now.
    fn retry(&mut self, ids: Option<&[DraftId]>) {
        let Self { held, waiting, failing, .. } = self;
        for (id, failed) in failing.iter_mut() {
            if ids.is_some_and(|ids| !ids.contains(id)) {
                continue;
            }
            if let Some(keep) = held.get(id) {
                waiting.entry(*id).or_insert_with(|| keep.clone());
                failed.due = Duration::ZERO;
            }
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hands each draft of `ids` whose write failed (each failing one with `None`) to the keeper again now.
    fn retry(&self, ids: Option<&[DraftId]>) {
        self.lock().retry(ids);
        self.changed.notify_all();
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared").finish_non_exhaustive()
    }
}

/// The store's end of the keeper.
pub(super) struct Keeper {
    shared: Arc<Shared>,
    thread: JoinHandle<()>,
}

impl std::fmt::Debug for Keeper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keeper").finish_non_exhaustive()
    }
}

/// While it lives, the keeper takes nothing to write: a batch on the store's own connection, or a test's hold.
#[derive(Debug)]
pub struct KeepHold {
    shared: Arc<Shared>,
}

impl Drop for KeepHold {
    fn drop(&mut self) {
        let mut queue = self.shared.lock();
        queue.paused = queue.paused.saturating_sub(1);
        drop(queue);
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
        conn.busy_timeout(LOCK_WAIT)?;
        let shared = Arc::new(Shared { queue: Mutex::new(Queue::default()), changed: Condvar::new(), clock: SystemClock::new() });
        let theirs = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("draft keeper".to_owned())
            .spawn(move || run(&theirs, conn))
            .map_err(|error| StoreError::Io(error.kind()))?;
        Ok(Self { shared, thread })
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
        queue.settled = false;
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

    /// Every draft kept and not on the disk, in the order they were kept.
    pub(super) fn all_held(&self) -> Vec<Arc<Draft>> {
        let queue = self.shared.lock();
        let mut held: Vec<&Keep> = queue.held.values().collect();
        held.sort_by_key(|(number, _)| *number);
        held.into_iter().map(|(_, draft)| Arc::clone(draft)).collect()
    }

    /// Why the last write of draft `id` failed, while none of it landed since.
    pub(super) fn failing(&self, id: DraftId) -> Option<StoreError> {
        self.shared.lock().failing.get(&id).map(|failing| failing.error.clone())
    }

    /// Every draft whose last write failed and none landed since, with why.
    pub(super) fn unkept(&self) -> Vec<(DraftId, StoreError)> {
        self.shared.lock().failing.iter().map(|(id, failing)| (*id, failing.error.clone())).collect()
    }

    /// How many drafts were handed to the keeper so far.
    pub(super) fn keeps(&self) -> u64 {
        self.shared.lock().next
    }

    /// A draft that started failing, with why, once.
    pub(super) fn take_failed(&self) -> Option<(DraftId, StoreError)> {
        self.shared.lock().untold.pop_first()
    }

    /// Hands each draft of `ids` whose write failed to the keeper again now.
    pub(super) fn retry(&self, ids: &[DraftId]) {
        self.shared.retry(Some(ids));
    }

    /// Forgets draft `id`: it is gone from the store, and nothing of it is to be written.
    pub(super) fn forget(&self, id: DraftId) {
        let mut queue = self.shared.lock();
        queue.held.remove(&id);
        queue.waiting.remove(&id);
        queue.failing.remove(&id);
        queue.untold.remove(&id);
    }

    /// Calls `wake` after each write from now on, landed or failed.
    pub(super) fn wake_on(&self, wake: KeepWake) {
        self.shared.lock().wake = Some(wake);
    }

    /// Whether the keeper's thread ended while the store still uses it.
    pub(super) fn is_gone(&self) -> bool {
        self.thread.is_finished()
    }

    /// Waits up to `within` until what was handed to the keeper is written (landed or failed).
    ///
    /// # Errors
    ///
    /// [`StoreError::DatabaseBusy`] when it is not written in that time.
    pub(super) fn settle(&self, within: Duration) -> Result<(), StoreError> {
        self.settled(within).map(drop)
    }

    /// [`Keeper::settle`], then holds the keeper (no new try of a failed write starts) while the hold lives.
    ///
    /// # Errors
    ///
    /// As [`Keeper::settle`].
    pub(super) fn gate(&self, within: Duration) -> Result<KeepHold, StoreError> {
        let mut queue = self.settled(within)?;
        queue.paused = queue.paused.saturating_add(1);
        Ok(KeepHold { shared: Arc::clone(&self.shared) })
    }

    /// Gives every failed write one more try now and waits up to `within` until all is written: the end of a window.
    ///
    /// # Errors
    ///
    /// As [`Keeper::settle`].
    pub(super) fn settle_all(&self, within: Duration) -> Result<(), StoreError> {
        self.shared.retry(None);
        let waited = self.settle(within);
        self.shared.lock().settled = true;
        waited
    }

    fn settled(&self, within: Duration) -> Result<MutexGuard<'_, Queue>, StoreError> {
        let busy = |queue: &mut Queue| !queue.waiting.is_empty() || queue.writing;
        let (mut queue, _) = self.shared.changed.wait_timeout_while(self.shared.lock(), within, busy).unwrap_or_else(PoisonError::into_inner);
        if busy(&mut queue) {
            return Err(StoreError::DatabaseBusy("a draft kept was not written in time".to_owned()));
        }
        Ok(queue)
    }

    /// The keeper's test hooks, as a handle of their own (tests).
    #[cfg(any(test, feature = "test-hooks"))]
    pub(super) fn hooks(&self) -> KeeperHooks {
        KeeperHooks { shared: Arc::clone(&self.shared) }
    }

    /// How many writes the keeper started (tests).
    #[cfg(any(test, feature = "test-hooks"))]
    pub(super) fn tries(&self) -> u64 {
        self.shared.lock().tries
    }

    /// Ends the keeper's thread as if it had died (tests); waits up to 10 s for it to end, a busy computer's time.
    #[cfg(any(test, feature = "test-hooks"))]
    pub(super) fn stop(&self) {
        self.shared.lock().stop = true;
        self.shared.changed.notify_all();
        for _ in 0..1000 {
            if self.thread.is_finished() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// A handle on the keeper of a store for tests: hold it, make its writes fail.
#[cfg(any(test, feature = "test-hooks"))]
#[derive(Debug, Clone)]
pub struct KeeperHooks {
    shared: Arc<Shared>,
}

#[cfg(any(test, feature = "test-hooks"))]
impl KeeperHooks {
    /// Holds the keeper until the hold is dropped: what is kept meanwhile waits.
    #[must_use]
    pub fn hold(&self) -> KeepHold {
        let mut queue = self.shared.lock();
        queue.paused = queue.paused.saturating_add(1);
        KeepHold { shared: Arc::clone(&self.shared) }
    }

    /// Makes every write fail while `refuse` is on, as a full or failing disk would.
    pub fn refuse(&self, refuse: bool) {
        self.shared.lock().refuse = refuse;
        self.shared.changed.notify_all();
    }

    /// Makes the end of the store wait, before anything else, until the keeper's thread looked at the queue once
    /// since the end began (or ended): the order a busy computer may give the two threads.
    pub fn close_after_a_look(&self) {
        self.shared.lock().close_after_a_look = true;
    }

    /// Makes every write of draft `id` fail while `refuse` is on; the other drafts land.
    pub fn refuse_draft(&self, id: DraftId, refuse: bool) {
        let mut queue = self.shared.lock();
        if refuse {
            queue.refuse_drafts.insert(id);
        } else {
            queue.refuse_drafts.remove(&id);
        }
        drop(queue);
        self.shared.changed.notify_all();
    }
}

/// The end of the store: unless the window already waited for the keeper and nothing was kept since, gives every
/// failed write one more try and waits at most [`CLOSE_WAIT`] for what was kept to be written; then lets the keeper
/// stop. A draft not on the disk by then is said on the app's standard error: the disk keeps it as it was before.
impl Drop for Keeper {
    fn drop(&mut self) {
        let settled = {
            let mut queue = self.shared.lock();
            queue.closing = true;
            // The new try is handed over with the end, under one lock: a keeper that sees the end first finds it
            // waiting, and does not stop with nothing to write.
            if !queue.settled {
                queue.retry(None);
            }
            queue.settled
        };
        self.shared.changed.notify_all();
        self.after_a_look();
        if !settled
            && !self.is_gone()
            && let Err(error) = self.settle(CLOSE_WAIT)
        {
            eprintln!("inboxcraft: a draft kept may not be on the disk: {error}");
        }
        for line in self.not_kept() {
            eprintln!("{line}");
        }
    }
}

impl Keeper {
    /// When a test asked for it, waits (at most 5 s) until the keeper's thread looked at the queue since the end of
    /// the store began, or ended.
    fn after_a_look(&self) {
        let queue = self.shared.lock();
        if queue.close_after_a_look {
            let waiting = |queue: &mut Queue| !queue.looked_at_the_end && !self.thread.is_finished();
            drop(self.shared.changed.wait_timeout_while(queue, Duration::from_secs(5), waiting));
        }
    }

    /// A line for each draft not on the disk: each whose last write failed, and, when the keeper's thread ended, each
    /// it held, written or not; the end of the store says them on the app's standard error.
    pub(super) fn not_kept(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .unkept()
            .into_iter()
            .map(|(id, error)| format!("inboxcraft: draft {id} was not kept, the disk holds it as it was before: {error}"))
            .collect();
        if self.is_gone() {
            let queue = self.shared.lock();
            for id in queue.held.keys().filter(|id| !queue.failing.contains_key(id)) {
                lines.push(format!("inboxcraft: draft {id} was not kept, the draft keeper stopped: the disk holds it as it was before"));
            }
        }
        lines
    }
}

/// The keeper's thread: takes what waits, writes it in one transaction, and tells the store how it went; hands a
/// failed write to itself again when its next try is due.
fn run(shared: &Shared, mut conn: Connection) {
    loop {
        let (taken, refuse) = {
            let mut queue = shared.lock();
            loop {
                if queue.stop {
                    return;
                }
                if queue.closing && !queue.looked_at_the_end {
                    queue.looked_at_the_end = true;
                    shared.changed.notify_all();
                }
                let now = shared.clock.monotonic();
                if queue.paused == 0 || queue.closing {
                    let Queue { held, waiting, failing, .. } = &mut *queue;
                    for (id, failed) in failing.iter() {
                        if failed.due <= now
                            && let Some(keep) = held.get(id)
                        {
                            waiting.entry(*id).or_insert_with(|| keep.clone());
                        }
                    }
                }
                if !queue.waiting.is_empty() && (queue.paused == 0 || queue.closing) {
                    break;
                }
                if queue.closing && queue.waiting.is_empty() {
                    return;
                }
                let due = queue.failing.values().filter(|failed| failed.due > now).map(|failed| failed.due).min();
                queue = match due {
                    Some(due) => shared.changed.wait_timeout(queue, due.saturating_sub(now)).unwrap_or_else(PoisonError::into_inner).0,
                    None => shared.changed.wait(queue).unwrap_or_else(PoisonError::into_inner),
                };
            }
            queue.writing = true;
            queue.tries = queue.tries.saturating_add(1);
            let refused: BTreeSet<DraftId> = queue.waiting.keys().copied().filter(|id| queue.refuse || queue.refuse_drafts.contains(id)).collect();
            (std::mem::take(&mut queue.waiting), refused)
        };
        let mut keeps: Vec<(DraftId, Keep)> = taken.into_iter().collect();
        keeps.sort_by_key(|(_, (number, _))| *number);
        // A test's refusal fails its drafts as a failing disk would; the others are written.
        let (refused_keeps, keeps): (Vec<_>, Vec<_>) = keeps.into_iter().partition(|(id, _)| refuse.contains(id));
        let written = if keeps.is_empty() {
            Ok(())
        } else {
            catch_unwind(AssertUnwindSafe(|| write(&mut conn, &keeps)))
                .unwrap_or_else(|_| Err(StoreError::Database("the draft keeper failed".to_owned())))
        };
        let refusal = Err(StoreError::DatabaseRefused("refused by a test".to_owned()));
        let now = shared.clock.monotonic();
        let mut queue = shared.lock();
        let results = keeps.iter().map(|keep| (keep, &written)).chain(refused_keeps.iter().map(|keep| (keep, &refusal)));
        for ((id, (number, _)), written) in results {
            match written {
                Ok(()) => {
                    // A newer keep of the same draft came meanwhile: it stays held until it is written.
                    if queue.held.get(id).is_some_and(|(held, _)| held == number) {
                        queue.held.remove(id);
                    }
                    queue.failing.remove(id);
                    queue.untold.remove(id);
                }
                Err(error) => {
                    // The draft stays held, its newest text with it, and is tried again later.
                    let tries = queue.failing.get(id).map_or(1, |failed| failed.tries.saturating_add(1));
                    if tries == 1 {
                        queue.untold.insert(*id, error.clone());
                    }
                    queue.failing.insert(*id, Failing { error: error.clone(), tries, due: now.saturating_add(backoff(tries)) });
                }
            }
        }
        let wake = queue.wake.clone();
        drop(queue);
        // The window is woken before a wait for the keeper ends: what it reads then is how the write went. A wake that
        // panics ends neither the keeper nor the write.
        if let Some(wake) = wake {
            let _ = catch_unwind(AssertUnwindSafe(|| wake()));
        }
        shared.lock().writing = false;
        shared.changed.notify_all();
    }
}

/// Writes `keeps` in one transaction, or none of them. The transaction takes the write lock as it begins, so a lock
/// another connection holds is waited for (at most [`LOCK_WAIT`]) rather than refused at once.
fn write(conn: &mut Connection, keeps: &[(DraftId, Keep)]) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for (_, (_, draft)) in keeps {
        local::put_draft(&tx, draft)?;
    }
    Ok(tx.commit()?)
}
