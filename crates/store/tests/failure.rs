// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A database that really fails, each kind of failure made from outside the store: the error has the kind's fixed
//! sentence, and the database's own words only as its detail.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;

use inboxcraft_model::{
    Account, AccountId, EmailAddress, Envelope, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId, Security, Server, Summary, Uid,
};
use inboxcraft_store::{Batch, Change, DATABASE_FILE, MAX_ERROR_DETAIL, Sqlite, Storage, StoreConfig, StoreError};
use rusqlite::Connection;

fn dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("failure-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

fn sql(dir: &Path, statement: &str) -> Connection {
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    conn.execute_batch(statement).unwrap();
    conn
}

/// A store in `dir` with one account, its inbox and message 1 in it.
fn store(dir: &Path) -> Sqlite {
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let server = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    let account = Account::new(AccountId::new(1), "One", EmailAddress::new("one@example.org").unwrap(), "one", server).unwrap();
    let folder = Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", Some('/')).unwrap(), vec![], true));
    let summary = Summary::new(Uid::new(5).unwrap(), Envelope::new("x"));
    let message = Message::new(MessageId::new(1), AccountId::new(1), FolderId::new(1), summary);
    let batch = Batch::new().with(Change::PutAccount(account)).with(Change::PutFolder(folder)).with(Change::PutMessage(message));
    store.apply(&batch).unwrap();
    store
}

/// The fixed sentence of `error`, with its detail kept apart and not in it.
fn sentence(error: &StoreError) -> String {
    let text = error.to_string();
    let detail = error.detail().unwrap();
    assert!(!detail.is_empty() && !text.contains(detail), "{text} | {detail}");
    text
}

#[test]
fn a_missing_table_is_a_failure_of_no_kind_of_its_own() {
    let tmp = dir();
    let store = store(tmp.path());
    sql(tmp.path(), "DROP TABLE body;");
    let error = store.messages(FolderId::new(1)).unwrap_err();
    assert!(matches!(error, StoreError::Database(_)), "{error:?}");
    assert_eq!(sentence(&error), "the store could not be read or written");
    assert!(error.detail().unwrap().contains("body"), "the detail keeps the database's words: {error:?}");
}

#[test]
fn a_held_database_is_busy() {
    let tmp = dir();
    let mut store = store(tmp.path());
    let held = sql(tmp.path(), "BEGIN IMMEDIATE; UPDATE meta SET value = value;");
    let open = Batch::new().with(Change::MarkOpened(MessageId::new(1)));
    let error = store.apply(&open).unwrap_err();
    assert!(matches!(error, StoreError::DatabaseBusy(_)), "{error:?}");
    assert_eq!(sentence(&error), "the store is busy, try again");
    held.execute_batch("COMMIT").unwrap();
    drop(held);
    store.apply(&open).unwrap();
}

#[test]
fn a_table_on_a_page_the_file_lacks_is_damaged() {
    let tmp = dir();
    let store = store(tmp.path());
    let conn = sql(tmp.path(), "PRAGMA writable_schema = ON; UPDATE sqlite_master SET rootpage = 1000000 WHERE name = 'message';");
    let version: i64 = conn.pragma_query_value(None, "schema_version", |row| row.get(0)).unwrap();
    conn.execute_batch(&format!("PRAGMA writable_schema = OFF; PRAGMA schema_version = {};", version + 1)).unwrap();
    let error = store.messages(FolderId::new(1)).unwrap_err();
    assert!(matches!(error, StoreError::DatabaseDamaged(_)), "{error:?}");
    assert_eq!(sentence(&error), "the store is damaged");
}

#[test]
fn a_file_that_is_no_database_is_damaged() {
    let tmp = dir();
    fs::write(tmp.path().join(DATABASE_FILE), vec![b'x'; 4096]).unwrap();
    let error = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap_err();
    assert!(matches!(error, StoreError::DatabaseDamaged(_)), "{error:?}");
    assert_eq!(sentence(&error), "the store is damaged");
}

#[test]
fn a_refused_change_is_refused_and_its_words_stay_in_the_detail() {
    let tmp = dir();
    let mut store = store(tmp.path());
    let words = format!("before a line break\nafter it {}", "x".repeat(MAX_ERROR_DETAIL));
    sql(tmp.path(), &format!("CREATE TRIGGER no BEFORE UPDATE ON message BEGIN SELECT RAISE(ABORT, '{words}'); END;"));
    let error = store.apply(&Batch::new().with(Change::MarkOpened(MessageId::new(1)))).unwrap_err();
    assert!(matches!(error, StoreError::DatabaseRefused(_)), "{error:?}");
    assert_eq!(sentence(&error), "the store refused the change");
    let detail = error.detail().unwrap();
    assert_eq!(detail.chars().count(), MAX_ERROR_DETAIL, "cut: {detail}");
    assert!(!detail.chars().any(char::is_control), "no control characters: {detail:?}");
    assert!(!store.messages(FolderId::new(1)).unwrap()[0].opened(), "nothing changed");
}

#[test]
fn a_migration_that_fails_is_the_migration_s_and_leaves_the_file_as_it_was() {
    let tmp = dir();
    drop(store(tmp.path()));
    // Back to version 3 with the table version 4 makes already there: the migration cannot make it.
    sql(tmp.path(), "PRAGMA user_version = 3; DROP TABLE job;");
    let error = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap_err();
    assert!(matches!(error, StoreError::Migration(_)), "{error:?}");
    assert_eq!(sentence(&error), "the store could not be brought up to this version");
    assert!(error.detail().unwrap().contains("meta"), "{error:?}");
    let version: u32 = sql(tmp.path(), "").pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    assert_eq!(version, 3);
}

#[test]
fn a_busy_database_at_a_migration_stays_busy() {
    let tmp = dir();
    drop(store(tmp.path()));
    let held = sql(tmp.path(), "BEGIN IMMEDIATE; UPDATE meta SET value = value;");
    let error = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap_err();
    assert!(matches!(error, StoreError::DatabaseBusy(_)), "{error:?}");
    drop(held);
}
