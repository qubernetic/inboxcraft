// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A store written by schema version 1 opens in this build with nothing lost.

#![allow(clippy::unwrap_used)]

mod contract;

use std::fs;
use std::path::Path;

use contract::{account, folder, m, message};
use inboxcraft_model::{AccountId, FolderId};
use inboxcraft_store::{Batch, Body, Change, DATABASE_FILE, SCHEMA_VERSION, SOURCES_DIR, Search, Sqlite, Storage, StoreConfig};
use rusqlite::{Connection, params};

/// Schema version 1 as it was released. Frozen: it is what old stores on disk look like.
const VERSION_1: &str = "CREATE TABLE account (
        id INTEGER PRIMARY KEY,
        record TEXT NOT NULL
    );
    CREATE TABLE folder (
        id INTEGER PRIMARY KEY,
        account INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
        record TEXT NOT NULL,
        state TEXT
    );
    CREATE INDEX folder_account ON folder (account, id);
    CREATE TABLE message (
        id INTEGER PRIMARY KEY,
        account INTEGER NOT NULL,
        folder INTEGER NOT NULL REFERENCES folder (id) ON DELETE CASCADE,
        uid INTEGER NOT NULL,
        date INTEGER,
        from_text TEXT NOT NULL,
        to_text TEXT NOT NULL,
        subject_text TEXT NOT NULL,
        opened INTEGER NOT NULL DEFAULT 0,
        record TEXT NOT NULL,
        UNIQUE (folder, uid)
    );
    CREATE INDEX message_date ON message (date);
    CREATE TABLE body (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        message INTEGER NOT NULL UNIQUE REFERENCES message (id) ON DELETE CASCADE,
        length INTEGER NOT NULL,
        text TEXT NOT NULL
    );
    PRAGMA user_version = 1;";

const SOURCE: &[u8] = b"Subject: Budget 2026\r\n\r\nThe draft budget is attached.\r\n";

/// A version 1 store with one account, folder and message, its body as source file 4.
fn version_1(dir: &Path) {
    fs::create_dir_all(dir.join(SOURCES_DIR)).unwrap();
    fs::write(dir.join(SOURCES_DIR).join("4.eml"), SOURCE).unwrap();
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    conn.execute_batch(VERSION_1).unwrap();
    conn.execute("INSERT INTO account (id, record) VALUES (1, ?1)", [json(&account(1))]).unwrap();
    conn.execute("INSERT INTO folder (id, account, record) VALUES (1, 1, ?1)", [json(&folder(1, 1, "INBOX"))]).unwrap();
    conn.execute(
        "INSERT INTO message (id, account, folder, uid, date, from_text, to_text, subject_text, opened, record)
         VALUES (1, 1, 1, 5, ?1, 'anna kovacs <anna@example.org>', 'bela@example.org\ncili <cili@example.net>', 'budget 2026', 1, ?2)",
        params![10 * contract::DAY, json(&message(1, 1, 1, 5, "Budget 2026", Some(10)))],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO body (id, message, length, text) VALUES (4, 1, ?1, 'The draft budget is attached.')",
        [i64::try_from(SOURCE.len()).unwrap()],
    )
    .unwrap();
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}

/// A fresh store directory under the target directory, removed when the value drops.
fn dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("migrate-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

#[test]
fn a_version_1_store_migrates_with_nothing_lost() {
    let tmp = dir();
    let dir = tmp.path();
    version_1(dir);
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.accounts().unwrap(), [account(1)]);
    assert_eq!(store.folders(AccountId::new(1)).unwrap(), [folder(1, 1, "INBOX")]);
    let entry = store.message(m(1)).unwrap().unwrap();
    assert!(entry.opened() && entry.has_body());
    assert_eq!(store.body(m(1)).unwrap(), Some(Body::new(SOURCE.to_vec(), "The draft budget is attached.").unwrap()));
    assert_eq!(store.search(&Search::new().with_text("draft budget").with_from("anna")).unwrap(), [m(1)], "the old text is indexed");

    store.apply(&Batch::new().with(Change::PutBody(m(1), Body::new(b"x".to_vec(), "new words").unwrap()))).unwrap();
    assert_eq!(store.search(&Search::new().with_text("draft")).unwrap(), []);
    assert_eq!(store.search(&Search::new().with_text("new words")).unwrap(), [m(1)]);
    assert!(!dir.join(SOURCES_DIR).join("4.eml").exists());
    store.apply(&Batch::new().with(Change::RemoveFolder(FolderId::new(1)))).unwrap();
    assert_eq!(store.search(&Search::new().with_text("new")).unwrap(), []);
}

#[test]
fn the_body_text_leaves_the_body_table() {
    let tmp = dir();
    let dir = tmp.path();
    version_1(dir);
    drop(Sqlite::open(dir, StoreConfig::default()).unwrap());
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    let columns: Vec<String> =
        conn.prepare("SELECT name FROM pragma_table_info('body')").unwrap().query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(columns, ["id", "message", "length"]);
}

#[test]
fn a_migrated_store_opens_again_unchanged() {
    let tmp = dir();
    let dir = tmp.path();
    version_1(dir);
    drop(Sqlite::open(dir, StoreConfig::default()).unwrap());
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.search(&Search::new().with_text("attached")).unwrap(), [m(1)]);
}

/// Schema version 2 as it was released, applied on top of version 1. Frozen like [`VERSION_1`].
const VERSION_2: &str = "CREATE VIRTUAL TABLE body_text USING fts5 (text, tokenize = 'unicode61 remove_diacritics 0');
    INSERT INTO body_text (rowid, text) SELECT message, text FROM body;
    ALTER TABLE body DROP COLUMN text;
    CREATE TRIGGER body_text_gone AFTER DELETE ON body BEGIN
        DELETE FROM body_text WHERE rowid = old.message;
    END;
    PRAGMA user_version = 2;";

fn secure_delete(dir: &Path) -> Option<i64> {
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    conn.query_row("SELECT v FROM body_text_config WHERE k = 'secure-delete'", [], |row| row.get(0)).ok()
}

#[test]
fn a_version_2_store_deletes_index_words_at_once() {
    let tmp = dir();
    let dir = tmp.path();
    version_1(dir);
    Connection::open(dir.join(DATABASE_FILE)).unwrap().execute_batch(VERSION_2).unwrap();
    assert_eq!(secure_delete(dir), None);
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(secure_delete(dir), Some(1));
    assert_eq!(store.search(&Search::new().with_text("draft budget")).unwrap(), [m(1)], "the index is kept");
    store.apply(&Batch::new().with(Change::RemoveBody(m(1)))).unwrap();
    assert_eq!(store.search(&Search::new().with_text("draft")).unwrap(), []);
}
