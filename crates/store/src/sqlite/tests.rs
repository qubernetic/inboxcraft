// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use rusqlite::config::DbConfig;

use super::{Sqlite, phrase};
use crate::StoreConfig;

#[test]
fn the_connection_trusts_nothing_in_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap();
    assert!(store.conn.db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE).unwrap(), "defensive mode");
    assert!(!store.conn.db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA).unwrap(), "no trusted schema");
    let secure_delete: i64 = store.conn.pragma_query_value(None, "secure_delete", |row| row.get(0)).unwrap();
    assert_eq!(secure_delete, 1);
}

#[test]
fn a_phrase_is_one_quoted_string() {
    assert_eq!(phrase(&[]), None);
    assert_eq!(phrase(&["near".to_owned(), "or".to_owned()]), Some("\"near or\"".to_owned()));
    assert_eq!(phrase(&["a\"b".to_owned(), "c".to_owned()]), Some("\"a\"\"b c\"".to_owned()), "a quote is doubled");
}

#[test]
fn a_full_database_is_full() {
    use crate::{Batch, Change, Storage, StoreError};
    use inboxcraft_model::{Account, AccountId, EmailAddress, Host, Security, Server};
    let tmp = tempfile::tempdir().unwrap();
    let mut store = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap();
    let pages: i64 = store.conn.pragma_query_value(None, "page_count", |row| row.get(0)).unwrap();
    store.conn.pragma_update(None, "max_page_count", pages).unwrap();
    let server = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    let mut batch = Batch::new();
    for id in 1..=200 {
        let name = "n".repeat(200);
        let account = Account::new(AccountId::new(id), &name, EmailAddress::new("one@example.org").unwrap(), "one", server.clone()).unwrap();
        batch = batch.with(Change::PutAccount(account));
    }
    let error = store.apply(&batch).unwrap_err();
    assert!(matches!(error, StoreError::DatabaseFull(_)), "{error:?}");
    assert_eq!(error.to_string(), "the store is full: there is no room left on the disk");
    assert!(error.detail().is_some_and(|detail| !detail.is_empty()));
    assert!(store.accounts().unwrap().is_empty(), "nothing of the batch is kept");
}

#[test]
fn a_migration_step_keeps_a_kind_and_takes_the_rest() {
    use crate::StoreError;
    let failure = |code| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), Some("words".to_owned()));
    let migrated = super::schema::migration_failed;
    assert_eq!(migrated(failure(rusqlite::ffi::SQLITE_BUSY)), StoreError::DatabaseBusy("words".to_owned()));
    assert_eq!(migrated(failure(rusqlite::ffi::SQLITE_FULL)), StoreError::DatabaseFull("words".to_owned()));
    assert_eq!(migrated(failure(rusqlite::ffi::SQLITE_CORRUPT)), StoreError::DatabaseDamaged("words".to_owned()));
    assert_eq!(migrated(failure(rusqlite::ffi::SQLITE_ERROR)), StoreError::Migration("words".to_owned()));
    assert_eq!(migrated(failure(rusqlite::ffi::SQLITE_CONSTRAINT)), StoreError::Migration("words".to_owned()), "the owner changed nothing");
}

/// A store with account 1 and its drafts 1 to `count`, draft `n` saved `n` seconds after 1970.
fn drafts_saved_in_turn(count: u64) -> (tempfile::TempDir, Sqlite) {
    use crate::{Batch, Change, Storage};
    use inboxcraft_model::{Account, AccountId, Draft, DraftId, EmailAddress, Host, Security, Server};
    use inboxcraft_time::Timestamp;
    let tmp = tempfile::tempdir().unwrap();
    let mut store = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap();
    let server = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    let account = Account::new(AccountId::new(1), "one", EmailAddress::new("one@example.org").unwrap(), "one", server).unwrap();
    let mut batch = Batch::new().with(Change::PutAccount(account));
    for id in 1..=count {
        let saved = Timestamp::from_unix(i64::try_from(id).unwrap(), 0).unwrap();
        batch = batch.with(Change::PutDraft(Draft::new(DraftId::new(id), AccountId::new(1), saved)));
    }
    store.apply(&batch).unwrap();
    (tmp, store)
}

#[test]
fn the_newest_drafts_read_no_older_record() {
    use crate::Storage;
    use inboxcraft_model::{AccountId, DraftId};
    let (_tmp, store) = drafts_saved_in_turn(50);
    // Every record but the newest two no longer reads back: a store that read them all would fail.
    store.conn.execute("UPDATE draft SET record = 'not a record' WHERE id <= 48", []).unwrap();
    let (total, newest) = store.newest_drafts(AccountId::new(1), 2).unwrap();
    assert_eq!(total, 50, "the count still holds every draft");
    assert_eq!(newest.iter().map(|draft| draft.id).collect::<Vec<_>>(), [DraftId::new(50), DraftId::new(49)]);
    assert!(store.drafts(AccountId::new(1)).is_err(), "the damage is real");
}

#[test]
fn the_newest_drafts_are_read_in_index_order_without_a_sort() {
    let (_tmp, store) = drafts_saved_in_turn(3);
    let mut statement = store.conn.prepare(&format!("EXPLAIN QUERY PLAN {}", super::NEWEST_DRAFTS)).unwrap();
    let plan: Vec<String> = statement.query_map(rusqlite::params![1, 2], |row| row.get(3)).unwrap().map(Result::unwrap).collect();
    assert!(plan.iter().any(|step| step.contains("draft_newest")), "{plan:?}");
    assert!(!plan.iter().any(|step| step.contains("TEMP B-TREE")), "no sort of every row: {plan:?}");
}
