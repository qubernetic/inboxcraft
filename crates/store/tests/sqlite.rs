// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shared contract on the SQLite backend, and what only it does: files, reopening, damage.

#![allow(clippy::unwrap_used)]

mod contract;

use std::fs;
use std::path::Path;

use contract::{account, body, folder, m, message, two_accounts};
use inboxcraft_model::{Account, AccountId, EmailAddress, FolderId};
use inboxcraft_store::{Batch, Change, DATABASE_FILE, SCHEMA_VERSION, SOURCES_DIR, Search, Sqlite, Storage, StoreConfig, StoreError};

/// A fresh store directory under the target directory, removed when the value drops.
fn dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("store-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

fn fresh() -> (Sqlite, tempfile::TempDir) {
    let tmp = dir();
    (Sqlite::open(tmp.path(), StoreConfig::default()).unwrap(), tmp)
}

contract_tests!(fresh());

fn sources(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        fs::read_dir(dir.join(SOURCES_DIR)).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

fn sql(dir: &Path, statement: &str) {
    rusqlite::Connection::open(dir.join(DATABASE_FILE)).unwrap().execute_batch(statement).unwrap();
}

/// A store in `dir` with message 1 in folder 1 and its body.
fn with_body(dir: &Path) -> Sqlite {
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    two_accounts(&mut store);
    store.apply(&Batch::new().with(Change::PutMessage(message(1, 1, 1, 5, "x", Some(1)))).with(Change::PutBody(m(1), body("hello")))).unwrap();
    store
}

#[test]
fn everything_is_there_after_a_reopen() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    store.apply(&Batch::new().with(Change::MarkOpened(m(1)))).unwrap();
    drop(store);
    let store = Sqlite::open(dir, StoreConfig { body_days: 7 }).unwrap();
    assert_eq!(store.config().body_days, 7);
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.accounts().unwrap(), [account(1), account(2)]);
    assert_eq!(store.folders(AccountId::new(1)).unwrap(), [folder(1, 1, "INBOX"), folder(2, 1, "Archive")]);
    let entry = store.message(m(1)).unwrap().unwrap();
    assert!(entry.opened() && entry.has_body());
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
    assert_eq!(store.search(&Search::new().with_text("hello")).unwrap(), [m(1)]);
}

#[test]
fn a_source_is_one_file_outside_the_database() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    assert_eq!(sources(dir), ["1.eml"]);
    assert_eq!(fs::read(dir.join(SOURCES_DIR).join("1.eml")).unwrap(), body("hello").source());
    store.apply(&Batch::new().with(Change::PutBody(m(1), body("again")))).unwrap();
    assert_eq!(sources(dir), ["2.eml"], "a new body gets a new file, the old one goes after the commit");
    store.apply(&Batch::new().with(Change::RemoveBody(m(1))).with(Change::PutBody(m(1), body("third")))).unwrap();
    assert_eq!(sources(dir), ["3.eml"]);
    store.apply(&Batch::new().with(Change::RemoveAccount(AccountId::new(1)))).unwrap();
    assert!(sources(dir).is_empty());
}

#[test]
fn a_refused_batch_leaves_no_file_behind() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    let refused = Batch::new().with(Change::PutBody(m(1), body("new"))).with(Change::MarkOpened(m(9)));
    assert_eq!(store.apply(&refused), Err(StoreError::UnknownMessage(m(9))));
    assert_eq!(sources(dir), ["1.eml"]);
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
}

#[test]
fn a_failed_source_write_refuses_the_batch() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    fs::remove_dir_all(dir.join(SOURCES_DIR)).unwrap();
    let error = store.apply(&Batch::new().with(Change::PutMessage(message(2, 1, 1, 6, "y", None))).with(Change::PutBody(m(2), body("lost"))));
    assert_eq!(error, Err(StoreError::Io(std::io::ErrorKind::NotFound)));
    assert_eq!(store.message(m(2)).unwrap(), None);
}

#[test]
fn opening_removes_the_files_a_kill_left_behind() {
    let tmp = dir();
    let dir = tmp.path();
    drop(with_body(dir));
    let sources_dir = dir.join(SOURCES_DIR);
    for name in ["7.eml", "8.partial", "notes.txt", "x.eml", "x.partial"] {
        fs::write(sources_dir.join(name), b"left over").unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        fs::write(sources_dir.join(std::ffi::OsStr::from_bytes(b"\xff.eml")), b"not utf-8").unwrap();
    }
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let mut left: Vec<_> = fs::read_dir(&sources_dir).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    left.sort();
    let mut expected: Vec<std::ffi::OsString> = ["1.eml", "notes.txt", "x.eml", "x.partial"].iter().map(Into::into).collect();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        expected.push(std::ffi::OsStr::from_bytes(b"\xff.eml").to_owned());
    }
    expected.sort();
    assert_eq!(left, expected);
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
}

#[test]
fn a_store_from_a_newer_build_is_left_alone() {
    let tmp = dir();
    let dir = tmp.path();
    drop(fresh_in(dir));
    sql(dir, "PRAGMA user_version = 99;");
    assert_eq!(Sqlite::open(dir, StoreConfig::default()).err(), Some(StoreError::NewerSchema { found: 99, known: SCHEMA_VERSION }));
    let version: u32 =
        rusqlite::Connection::open(dir.join(DATABASE_FILE)).unwrap().pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    assert_eq!(version, 99);
}

fn fresh_in(dir: &Path) -> Sqlite {
    Sqlite::open(dir, StoreConfig::default()).unwrap()
}

#[test]
fn a_file_that_is_no_database_is_refused() {
    let tmp = dir();
    let dir = tmp.path();
    fs::write(dir.join(DATABASE_FILE), vec![b'x'; 4096]).unwrap();
    assert!(matches!(Sqlite::open(dir, StoreConfig::default()), Err(StoreError::Database(_))));
}

#[test]
fn a_directory_that_cannot_be_made_is_refused() {
    let tmp = dir();
    let dir = tmp.path();
    fs::write(dir.join(SOURCES_DIR), b"a file where the directory goes").unwrap();
    assert!(matches!(Sqlite::open(dir, StoreConfig::default()), Err(StoreError::Io(_))));
}

#[test]
fn a_damaged_source_is_reported() {
    let tmp = dir();
    let dir = tmp.path();
    let store = with_body(dir);
    let file = dir.join(SOURCES_DIR).join("1.eml");
    fs::write(&file, b"short").unwrap();
    assert_eq!(store.body(m(1)), Err(StoreError::Corrupt("a message source has the wrong length")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(store.body(m(1)), Err(StoreError::Io(std::io::ErrorKind::PermissionDenied)));
    }
    fs::remove_file(&file).unwrap();
    assert_eq!(store.body(m(1)), Err(StoreError::Corrupt("a message source is missing")));
    sql(dir, "UPDATE body SET length = -1;");
    assert_eq!(store.body(m(1)), Err(StoreError::Corrupt("a source length")));
}

#[test]
fn a_damaged_record_is_reported() {
    let tmp = dir();
    let dir = tmp.path();
    let store = with_body(dir);
    sql(dir, "UPDATE account SET record = '{' WHERE id = 1; UPDATE message SET record = '[]'; UPDATE folder SET record = 'null', state = '1';");
    assert_eq!(store.accounts(), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.folders(AccountId::new(1)), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.folder_state(FolderId::new(1)), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.messages(FolderId::new(1)), Err(StoreError::Corrupt("a record")));
    let mut store = store;
    assert_eq!(store.apply(&Batch::new().with(Change::SetFlags(m(1), Default::default()))), Err(StoreError::Corrupt("a record")));
    sql(dir, "PRAGMA foreign_keys = OFF; UPDATE message SET id = -4;");
    assert_eq!(store.search(&Search::new()), Err(StoreError::Corrupt("a message id")));
    sql(dir, "UPDATE message SET opened = 'yes';");
    assert!(matches!(store.messages(FolderId::new(1)), Err(StoreError::Database(_))));
}

#[test]
fn an_id_sqlite_cannot_hold_is_refused() {
    let (mut store, _tmp) = fresh();
    let big = Account::new(AccountId::new(u64::MAX), "Big", EmailAddress::new("big@example.org").unwrap(), "big", account(1).imap().clone()).unwrap();
    let refused = store.apply(&Batch::new().with(Change::PutAccount(big)));
    assert_eq!(refused, Err(StoreError::Database(format!("id {} is out of range", u64::MAX))));
    assert!(store.accounts().unwrap().is_empty());
}

#[test]
fn the_database_runs_in_wal_mode_with_full_sync() {
    let tmp = dir();
    let dir = tmp.path();
    drop(fresh_in(dir));
    let conn = rusqlite::Connection::open(dir.join(DATABASE_FILE)).unwrap();
    let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0)).unwrap();
    assert_eq!(mode, "wal");
}
