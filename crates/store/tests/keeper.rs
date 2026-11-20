// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The draft keeper of the SQLite backend: drafts written behind, in order, the newest of each, seen by the reads
//! before they are on the disk, waited for by every other batch and by the end of the store.

#![allow(clippy::unwrap_used)]

mod contract;

use std::path::Path;
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
fn a_keep_that_fails_is_told_once_and_the_disk_keeps_the_draft_as_it_was() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    rusqlite::Connection::open(tmp.path().join(DATABASE_FILE))
        .unwrap()
        .execute_batch("CREATE TRIGGER refuse BEFORE UPDATE ON draft BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    keep(&mut store, "lost");
    store.settle(Duration::from_secs(5)).unwrap();
    let (id, error) = store.failed_keep().unwrap();
    assert_eq!(id, DraftId::new(1));
    assert!(matches!(error, StoreError::DatabaseRefused(_)), "{error:?}");
    assert!(store.failed_keep().is_none(), "told once");
    assert_eq!(store.draft(DraftId::new(1)).unwrap().unwrap().subject, "first", "the reads say what the disk holds");
}

#[test]
fn a_send_whose_keep_failed_meanwhile_is_refused() {
    let tmp = dir();
    let mut store = keeping(tmp.path());
    rusqlite::Connection::open(tmp.path().join(DATABASE_FILE))
        .unwrap()
        .execute_batch("CREATE TRIGGER refuse BEFORE UPDATE ON draft BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let hold = store.hold_keeps().unwrap();
    keep(&mut store, "lost");
    let released = release_after(hold, 50);
    let sent = store.apply(&Batch::new().with(Change::PutOutgoing(outgoing(1, OutboxState::Waiting))));
    released.join().unwrap();
    assert!(matches!(sent, Err(StoreError::DatabaseRefused(_))), "{sent:?}");
    assert!(store.outbox().unwrap().is_empty(), "nothing goes with the draft as it was before");
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
