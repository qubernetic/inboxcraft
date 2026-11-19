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
