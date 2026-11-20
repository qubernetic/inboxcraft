// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The draft keeper of the SQLite backend: drafts written behind, in order, the newest of each, seen by the reads
//! before they are on the disk, waited for by every other batch and by the end of the store.

#![allow(clippy::unwrap_used)]

mod contract;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use contract::{draft, outgoing, two_accounts};
use inboxcraft_model::{AccountId, DraftId, OutboxState};
use inboxcraft_store::{Batch, Change, DATABASE_FILE, KeepHold, Sqlite, Storage, StoreConfig, StoreError};

fn dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("keeper-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

/// A store in `dir` with two accounts and the draft 1 of account 1 ("first"), keeping drafts behind.
fn keeping(dir: &Path) -> Sqlite {
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    two_accounts(&mut store);
    store.apply(&Batch::new().with(Change::PutDraft(draft(1, 1, "first")))).unwrap();
    store.keep_drafts_behind().unwrap();
    store
}

fn keep(store: &mut Sqlite, subject: &str) {
    store.apply(&Batch::new().with(Change::PutDraft(draft(1, 1, subject)))).unwrap();
}

/// The subject of draft 1 as the disk holds it, read by a store of its own.
fn on_disk(dir: &Path) -> Option<String> {
    Sqlite::open(dir, StoreConfig::default()).unwrap().draft(DraftId::new(1)).unwrap().map(|draft| draft.subject)
}

/// Lets the keeper go after `ms` milliseconds, from another thread.
fn release_after(hold: KeepHold, ms: u64) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        drop(hold);
    })
}

#[test]
fn the_newest_of_two_keeps_wins_and_is_read_before_it_is_on_the_disk() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let hold = store.hold_keeps().unwrap();
    keep(&mut store, "second");
    keep(&mut store, "third");
    // Not on the disk yet, and every read sees the newest.
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("first"));
    assert_eq!(store.draft(DraftId::new(1)).unwrap().unwrap().subject, "third");
    assert_eq!(store.drafts(AccountId::new(1)).unwrap()[0].subject, "third");
    let (total, newest) = store.newest_drafts(AccountId::new(1), 5).unwrap();
    assert_eq!((total, newest[0].subject.as_str()), (1, "third"));
    drop(hold);
    store.settle(Duration::from_secs(5)).unwrap();
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("third"), "the newest is what reaches the disk");
    assert!(store.failed_keep().is_none());
}

#[test]
fn the_newest_drafts_take_a_keep_in_its_new_place() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    // Draft 2 is newer on the disk; draft 1's keep is newer still.
    let mut second = draft(2, 1, "two");
    second.saved = second.saved.checked_add(Duration::from_secs(60)).unwrap();
    store.apply(&Batch::new().with(Change::PutDraft(second))).unwrap();
    store.settle(Duration::from_secs(5)).unwrap();
    let _hold = store.hold_keeps().unwrap();
    let mut first = draft(1, 1, "one again");
    first.saved = first.saved.checked_add(Duration::from_secs(120)).unwrap();
    store.apply(&Batch::new().with(Change::PutDraft(first))).unwrap();
    let (total, newest) = store.newest_drafts(AccountId::new(1), 1).unwrap();
    assert_eq!(total, 2);
    assert_eq!(newest.iter().map(|draft| draft.subject.as_str()).collect::<Vec<_>>(), ["one again"]);
}

#[test]
fn the_end_of_the_store_waits_for_what_is_kept() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let hold = store.hold_keeps().unwrap();
    keep(&mut store, "last words");
    let released = release_after(hold, 200);
    drop(store);
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("last words"));
    released.join().unwrap();
}

#[test]
fn a_discard_after_a_keep_leaves_no_draft_behind() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let hold = store.hold_keeps().unwrap();
    keep(&mut store, "kept");
    let released = release_after(hold, 100);
    store.apply(&Batch::new().with(Change::RemoveDraft(DraftId::new(1)))).unwrap();
    released.join().unwrap();
    store.settle(Duration::from_secs(5)).unwrap();
    assert_eq!(store.draft(DraftId::new(1)).unwrap(), None);
    assert_eq!(on_disk(tmp.path()), None, "the keep never comes after the discard");
}

#[test]
fn a_send_after_a_keep_finds_the_newest() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let hold = store.hold_keeps().unwrap();
    keep(&mut store, "newest");
    let released = release_after(hold, 100);
    store.apply(&Batch::new().with(Change::PutOutgoing(outgoing(1, OutboxState::Waiting)))).unwrap();
    // The send's batch waited for the keep: both are on the disk when it returns.
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("newest"));
    released.join().unwrap();
    assert_eq!(store.outbox().unwrap().len(), 1);
}

#[test]
fn a_keep_that_fails_is_told_once_keeps_the_newest_text_and_lands_when_tried_again() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    store.refuse_keeps(true);
    keep(&mut store, "newest");
    store.settle(Duration::from_secs(5)).unwrap();
    let (id, error) = store.failed_keep().unwrap();
    assert_eq!(id, DraftId::new(1));
    assert!(matches!(error, StoreError::DatabaseRefused(_)), "{error:?}");
    assert!(store.failed_keep().is_none(), "told once");
    assert_eq!(store.unkept().len(), 1, "listed until it lands");
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("first"), "the disk holds the draft as it was");
    assert_eq!(store.draft(DraftId::new(1)).unwrap().unwrap().subject, "newest", "the newest text is not dropped");
    assert_eq!(store.drafts(AccountId::new(1)).unwrap()[0].subject, "newest");
    // The keeper tries it again by itself, after a second.
    store.refuse_keeps(false);
    let mut waited = 0;
    while on_disk(tmp.path()).as_deref() != Some("newest") && waited < 50 {
        std::thread::sleep(Duration::from_millis(100));
        waited += 1;
    }
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("newest"), "the newest text lands when tried again");
    assert!(store.unkept().is_empty());
    assert!(store.failed_keep().is_none());
}

/// Waits (at most 5 s, every 10 ms) until `done` holds for the store; whether it did.
fn until(store: &Sqlite, done: impl Fn(&Sqlite) -> bool) -> bool {
    for _ in 0..500 {
        if done(store) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    done(store)
}

#[test]
fn a_keep_that_keeps_failing_is_tried_again_with_a_growing_wait_and_told_once() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    store.refuse_keeps(true);
    keep(&mut store, "newest");
    // No `settle` here: it would give the failed write a try of its own, and the count would depend on which thread
    // ran first. Tries at 0 s, 1 s and 3 s: the second a second after the first, the third two seconds after that.
    assert!(until(&store, |store| store.keeper_tries() >= 2), "tried again within a second or so");
    assert_eq!(store.keeper_tries(), 2);
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(store.keeper_tries(), 2, "the wait grew past a second, not one try a frame");
    assert!(until(&store, |store| store.keeper_tries() >= 3), "and the third try comes");
    assert!(store.failed_keep().is_some());
    assert!(store.failed_keep().is_none(), "one failure said for the whole run of failures");
}

#[test]
fn the_end_of_the_store_tries_a_failed_keep_again_when_the_disk_came_back() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    store.refuse_keeps(true);
    keep(&mut store, "newest");
    assert!(until(&store, |store| !store.unkept().is_empty()));
    store.refuse_keeps(false);
    // Long before the keeper's own next try (1 s), and nobody waited for it: the end of the store tries it now.
    drop(store);
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("newest"));
}

#[test]
fn removing_the_account_of_a_draft_whose_keep_failed_ends_the_draft() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    store.refuse_keeps(true);
    keep(&mut store, "newest");
    store.settle(Duration::from_secs(5)).unwrap();
    assert_eq!(store.unkept().len(), 1);
    store.apply(&Batch::new().with(Change::RemoveAccount(AccountId::new(1)))).unwrap();
    assert!(store.unkept().is_empty(), "nothing of it is to be written");
    assert_eq!(store.draft(DraftId::new(1)).unwrap(), None, "and the reads do not see it");
    store.refuse_keeps(false);
    store.settle(Duration::from_secs(5)).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(store.draft(DraftId::new(1)).unwrap(), None);
    drop(store);
    assert_eq!(on_disk(tmp.path()), None, "the held copy is never written after the account went");
}

#[test]
fn a_wait_on_a_keeper_gone_writes_what_it_held_at_once() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let _hold = store.hold_keeps().unwrap();
    keep(&mut store, "held");
    store.stop_keeper();
    // The close of a window finds the keeper gone: it does not wait for a thread that ended.
    assert_eq!(store.settle(Duration::from_secs(1)), Ok(()));
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("held"));
    assert!(store.unkept().is_empty());
}

#[test]
fn a_wake_that_panics_ends_neither_the_keeper_nor_its_write() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let woken = Arc::new(AtomicUsize::new(0));
    let theirs = Arc::clone(&woken);
    store.wake_on_keep(Arc::new(move || {
        assert!(theirs.fetch_add(1, Ordering::SeqCst) > 0, "the first wake panics");
    }));
    keep(&mut store, "one");
    store.settle(Duration::from_secs(5)).unwrap();
    keep(&mut store, "two");
    store.settle(Duration::from_secs(5)).unwrap();
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("two"));
    assert_eq!(store.keeper_tries(), 2, "the keeper wrote both: it lives on");
    assert_eq!(woken.load(Ordering::SeqCst), 2);
}

#[test]
fn a_batch_on_a_draft_whose_keep_failed_is_refused_every_time_and_a_discard_goes_through() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    store.refuse_keeps(true);
    keep(&mut store, "newest");
    store.settle(Duration::from_secs(5)).unwrap();
    // The keeper is idle: the failure is old, and the batch is refused all the same, each time.
    for _ in 0..2 {
        let sent = store.apply(&Batch::new().with(Change::PutOutgoing(outgoing(1, OutboxState::Waiting))));
        assert_eq!(sent, Err(StoreError::DraftNotKept(DraftId::new(1))));
    }
    let moved = store.apply(&Batch::new().with(Change::PutDraft(draft(1, 2, "moved"))));
    assert_eq!(moved, Err(StoreError::DraftNotKept(DraftId::new(1))));
    assert!(store.outbox().unwrap().is_empty(), "nothing goes with the draft as it was before");
    // A batch that does not name the draft goes on.
    store.apply(&Batch::new().with(Change::PutDraft(draft(2, 1, "other")))).unwrap();
    // A discard ends it: the keep is never written after it.
    store.apply(&Batch::new().with(Change::RemoveDraft(DraftId::new(1)))).unwrap();
    assert!(store.unkept().is_empty());
    store.refuse_keeps(false);
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(on_disk(tmp.path()), None);
    assert_eq!(store.draft(DraftId::new(1)).unwrap(), None);
}

#[test]
fn a_send_after_a_failed_keep_tries_it_again_first() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    store.refuse_keeps(true);
    keep(&mut store, "newest");
    store.settle(Duration::from_secs(5)).unwrap();
    store.refuse_keeps(false);
    // Long before the keeper's own next try: the send's batch tries the keep again and goes.
    store.apply(&Batch::new().with(Change::PutOutgoing(outgoing(1, OutboxState::Waiting)))).unwrap();
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("newest"));
    assert_eq!(store.outbox().unwrap().len(), 1);
}

#[test]
fn an_account_move_after_a_keep_on_its_way_comes_after_it() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let hold = store.hold_keeps().unwrap();
    keep(&mut store, "kept");
    let released = release_after(hold, 100);
    store.apply(&Batch::new().with(Change::PutDraft(draft(1, 2, "moved")))).unwrap();
    released.join().unwrap();
    store.settle(Duration::from_secs(5)).unwrap();
    let other = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap();
    let moved = other.draft(DraftId::new(1)).unwrap().unwrap();
    assert_eq!((moved.subject.as_str(), moved.account), ("moved", AccountId::new(2)), "the older keep never lands after the move");
}

#[test]
fn the_keeper_wakes_the_window_after_each_write() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let woken = Arc::new(AtomicUsize::new(0));
    let theirs = Arc::clone(&woken);
    store.wake_on_keep(Arc::new(move || {
        theirs.fetch_add(1, Ordering::SeqCst);
    }));
    keep(&mut store, "landed");
    store.settle(Duration::from_secs(5)).unwrap();
    assert_eq!(woken.load(Ordering::SeqCst), 1, "after a write that landed");
    store.refuse_keeps(true);
    keep(&mut store, "failed");
    store.settle(Duration::from_secs(5)).unwrap();
    assert_eq!(woken.load(Ordering::SeqCst), 2, "after a write that failed");
}

#[test]
fn a_keeper_gone_leaves_the_store_writing_at_once_with_what_it_held() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let _hold = store.hold_keeps().unwrap();
    keep(&mut store, "held");
    store.stop_keeper();
    // The next batch finds the keeper gone: what it held is written first, and this batch at once.
    store.apply(&Batch::new().with(Change::PutDraft(draft(2, 1, "next")))).unwrap();
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("held"));
    keep(&mut store, "at once");
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("at once"));
    assert!(store.hold_keeps().is_none(), "no keeper any more");
}

#[test]
fn a_write_blocked_by_a_lock_fails_within_the_wait_and_the_end_does_not_wait_again() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let other = rusqlite::Connection::open(tmp.path().join(DATABASE_FILE)).unwrap();
    other.execute_batch("BEGIN IMMEDIATE;").unwrap();
    keep(&mut store, "blocked");
    // The keeper's lock wait (2 s) is shorter than the window's (3 s): the write fails, and is said, within it.
    store.settle(Duration::from_secs(3)).unwrap();
    assert!(matches!(store.unkept().first(), Some((_, StoreError::DatabaseBusy(_)))), "{:?}", store.unkept());
    #[allow(clippy::disallowed_methods, reason = "a test of the wall time")]
    let start = std::time::Instant::now();
    drop(store);
    assert!(start.elapsed() < Duration::from_millis(1500), "the window waited already: {:?}", start.elapsed());
    other.execute_batch("ROLLBACK;").unwrap();
}

#[test]
fn a_new_draft_or_one_moved_to_another_account_is_written_at_once() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    let _hold = store.hold_keeps().unwrap();
    store.apply(&Batch::new().with(Change::PutDraft(draft(2, 1, "new")))).unwrap();
    store.apply(&Batch::new().with(Change::PutDraft(draft(1, 2, "moved")))).unwrap();
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("moved"));
    let other = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap();
    assert!(other.draft(DraftId::new(2)).unwrap().is_some());
}

#[test]
fn a_store_without_a_keeper_writes_at_once_and_has_nothing_to_wait_for() {
    let tmp = dir();
    let mut store = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap();
    two_accounts(&mut store);
    assert!(store.hold_keeps().is_none());
    store.apply(&Batch::new().with(Change::PutDraft(draft(1, 1, "at once")))).unwrap();
    assert_eq!(on_disk(tmp.path()).as_deref(), Some("at once"));
    assert_eq!(store.settle(Duration::ZERO), Ok(()));
    assert!(store.failed_keep().is_none());
}
