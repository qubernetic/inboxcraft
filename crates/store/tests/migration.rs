// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A store written by schema version 1 opens in this build with nothing lost.

#![allow(clippy::unwrap_used)]

mod contract;

use std::fs;
use std::path::Path;

use contract::{account, folder, m, message};
use inboxcraft_model::{AccountId, CalendarEventId, FolderId, MessageState};
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

/// Schema version 3 as it was released, applied on top of version 2. Frozen like [`VERSION_1`].
const VERSION_3: &str = "INSERT INTO body_text (body_text, rank) VALUES ('secure-delete', 1);
    PRAGMA user_version = 3;";

/// A version 3 store: the mail of [`version_1`], migrated by hand through versions 2 and 3.
fn version_3(dir: &Path) {
    version_1(dir);
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    conn.execute_batch(VERSION_2).unwrap();
    conn.execute_batch(VERSION_3).unwrap();
}

#[test]
fn a_version_3_store_gets_the_local_tables_with_its_mail_kept() {
    let tmp = dir();
    let dir = tmp.path();
    version_3(dir);
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.accounts().unwrap(), [account(1)]);
    assert!(store.message(m(1)).unwrap().unwrap().opened());
    assert_eq!(store.search(&Search::new().with_text("draft budget")).unwrap(), [m(1)]);
    assert_eq!(secure_delete(dir), Some(1));
    let id = store.id().unwrap();
    assert_eq!(id.len(), 32, "a migrated store gets its own id too: {id}");
    store
        .apply(
            &Batch::new()
                .with(Change::QueueJob(contract::job(1, 1)))
                .with(Change::PutDraft(contract::draft(1, 1, "Re: Budget 2026")))
                .with(Change::SetMessageState(m(1), MessageState { pinned: true, ..MessageState::default() }))
                .with(Change::PutCalendar(contract::calendar(1, 1, "home")))
                .with(Change::PutCalendarEvent(contract::event(1, 1)))
                .with(Change::PutAlert(contract::alert(1, 1, 3))),
        )
        .unwrap();
    drop(store);
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.queued_jobs().unwrap(), [contract::job(1, 1)]);
    assert_eq!(store.drafts(AccountId::new(1)).unwrap(), [contract::draft(1, 1, "Re: Budget 2026")]);
    assert!(store.message_state(m(1)).unwrap().pinned);
    assert_eq!(store.alerts(CalendarEventId::new(1)).unwrap(), [contract::alert(1, 1, 3)]);
    assert_eq!(store.id().unwrap(), id);
}

#[test]
fn the_local_tables_are_the_ones_the_epic_needs() {
    let tmp = dir();
    let dir = tmp.path();
    version_3(dir);
    drop(Sqlite::open(dir, StoreConfig::default()).unwrap());
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'body_text%' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        tables,
        [
            "account",
            "alert",
            "body",
            "calendar",
            "calendar_event",
            "draft",
            "folder",
            "job",
            "message",
            "message_link",
            "message_state",
            "meta",
            "outbox",
            "schedule",
            "sender"
        ]
    );
}

/// What versions 5 and 6 added, taken off again: a store as version 4 left it, with its rows.
const BACK_TO_VERSION_4: &str = "DROP TABLE message_link;
    DROP INDEX draft_newest;
    ALTER TABLE draft DROP COLUMN saved;
    ALTER TABLE draft DROP COLUMN saved_nanos;
    PRAGMA user_version = 4;";

#[test]
fn a_version_4_store_gets_the_save_time_of_its_drafts() {
    let tmp = dir();
    let dir = tmp.path();
    let saved = |id: u64, seconds: i64, nanos: u32| inboxcraft_model::Draft {
        saved: inboxcraft_time::Timestamp::from_unix(seconds, nanos).unwrap(),
        ..contract::draft(id, 1, "kept")
    };
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let mut batch = Batch::new().with(Change::PutAccount(account(1)));
    for (id, seconds, nanos) in [(1, 300, 0), (2, 100, 0), (3, 300, 7), (4, 200, 0)] {
        batch = batch.with(Change::PutDraft(saved(id, seconds, nanos)));
    }
    store.apply(&batch).unwrap();
    drop(store);
    Connection::open(dir.join(DATABASE_FILE)).unwrap().execute_batch(BACK_TO_VERSION_4).unwrap();
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.newest_drafts(AccountId::new(1), 3).unwrap(), (4, vec![saved(3, 300, 7), saved(1, 300, 0), saved(4, 200, 0)]));
}

#[test]
fn a_version_4_store_with_a_damaged_draft_opens() {
    let tmp = dir();
    let dir = tmp.path();
    let saved = |id: u64| inboxcraft_model::Draft {
        saved: inboxcraft_time::Timestamp::from_unix(i64::try_from(id * 37 % 101 + 1).unwrap(), 0).unwrap(),
        ..contract::draft(id, 1, "kept")
    };
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let mut batch = Batch::new().with(Change::PutAccount(account(1)));
    for id in 1..=50 {
        batch = batch.with(Change::PutDraft(saved(id)));
    }
    store.apply(&batch).unwrap();
    drop(store);
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    conn.execute_batch(BACK_TO_VERSION_4).unwrap();
    conn.execute_batch("UPDATE draft SET record = 'not a record' WHERE id = 7; UPDATE draft SET record = x'00ff10' WHERE id = 13;").unwrap();
    drop(conn);
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    let mut kept: Vec<_> = (1..=50).filter(|id| *id != 7 && *id != 13).map(saved).collect();
    kept.sort_by(|a, b| b.saved.cmp(&a.saved).then(b.id.cmp(&a.id)));
    // The damaged two sort oldest, below the 48 the backfill could read.
    assert_eq!(store.newest_drafts(AccountId::new(1), 48).unwrap(), (50, kept));
}

/// What version 6 added, taken off again: a store as version 5 left it, with its messages.
const BACK_TO_VERSION_5: &str = "DROP TABLE message_link;
    PRAGMA user_version = 5;";

#[test]
fn a_version_5_store_gets_the_links_of_its_messages() {
    let tmp = dir();
    let dir = tmp.path();
    let named = |id: u64, own: &str, answers: &str| {
        let message = message(id, 1, 1, u32::try_from(id).unwrap(), "Linked", Some(1));
        let envelope = message
            .summary()
            .envelope()
            .clone()
            .with_message_id(inboxcraft_model::MessageRef::new(own))
            .with_in_reply_to(inboxcraft_model::MessageRef::new(answers).into_iter().collect());
        let summary = inboxcraft_model::Summary::new(message.summary().uid(), envelope);
        inboxcraft_model::Message::new(message.id(), message.account(), message.folder(), summary)
    };
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let batch = Batch::new()
        .with(Change::PutAccount(account(1)))
        .with(Change::PutFolder(folder(1, 1, "INBOX")))
        .with(Change::PutMessage(named(1, "first@example.org", "")))
        .with(Change::PutMessage(named(2, "second@example.org", "first@example.org")));
    store.apply(&batch).unwrap();
    drop(store);
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    conn.execute_batch(BACK_TO_VERSION_5).unwrap();
    // A record that does not read back is left out of the index; the store opens.
    conn.execute(
        "INSERT INTO message (id, account, folder, uid, from_text, to_text, subject_text, record) VALUES (3, 1, 1, 3, '', '', '', 'damaged')",
        [],
    )
    .unwrap();
    drop(conn);
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    let linked = |store: &Sqlite, link: &str| -> Vec<u64> {
        store
            .linked(&inboxcraft_model::MessageRef::new(link).unwrap(), &std::collections::BTreeSet::new())
            .unwrap()
            .iter()
            .map(|entry| entry.message().id().get())
            .collect()
    };
    assert_eq!(linked(&store, "first@example.org"), [1, 2]);
    assert_eq!(linked(&store, "second@example.org"), [2]);
    drop(store);
    // A record changed behind the index's back is linked only by what it names now.
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    conn.execute_batch("DELETE FROM message WHERE id = 3; UPDATE message SET record = (SELECT record FROM message WHERE id = 1) WHERE id = 2;")
        .unwrap();
    drop(conn);
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(linked(&store, "second@example.org"), Vec::<u64>::new());
}

/// An open killed half-way through indexing the links of a version 5 store leaves it at version 5 with no index, and
/// the next open does the whole of it again.
#[test]
fn a_link_indexing_stopped_half_way_is_done_again_at_the_next_open() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let mut batch = Batch::new().with(Change::PutAccount(account(1))).with(Change::PutFolder(folder(1, 1, "INBOX")));
    for id in 1..=4u64 {
        let message = message(id, 1, 1, u32::try_from(id).unwrap(), "Linked", Some(1));
        let envelope = message.summary().envelope().clone().with_message_id(inboxcraft_model::MessageRef::new(&format!("m{id}@example.org")));
        let summary = inboxcraft_model::Summary::new(message.summary().uid(), envelope);
        batch.push(Change::PutMessage(inboxcraft_model::Message::new(message.id(), message.account(), message.folder(), summary)));
    }
    store.apply(&batch).unwrap();
    drop(store);
    Connection::open(dir.join(DATABASE_FILE)).unwrap().execute_batch(BACK_TO_VERSION_5).unwrap();
    Sqlite::stop_link_indexing_after(Some(2));
    let stopped = Sqlite::open(dir, StoreConfig::default());
    Sqlite::stop_link_indexing_after(None);
    assert!(matches!(stopped, Err(inboxcraft_store::StoreError::Migration(_))), "{stopped:?}");
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    let version: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    let tables: u32 = conn.query_row("SELECT count(*) FROM sqlite_master WHERE name = 'message_link'", [], |row| row.get(0)).unwrap();
    assert_eq!((version, tables), (5, 0));
    drop(conn);
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    for id in 1..=4u64 {
        let link = inboxcraft_model::MessageRef::new(&format!("m{id}@example.org")).unwrap();
        let found: Vec<u64> =
            store.linked(&link, &std::collections::BTreeSet::new()).unwrap().iter().map(|entry| entry.message().id().get()).collect();
        assert_eq!(found, [id]);
    }
}

/// How long the first open after the update takes to index the links of 100 000 messages, each with a Message-ID,
/// an In-Reply-To and 10 References: a timing, run by hand in release
/// (`cargo test --release -p inboxcraft-store --test migration link_indexing_timing -- --ignored --nocapture`).
#[test]
#[ignore = "a timing, run by hand in release"]
#[allow(clippy::disallowed_methods, reason = "a measurement of the wall time")]
fn link_indexing_timing() {
    const MESSAGES: u64 = 100_000;
    let tmp = dir();
    let dir = tmp.path();
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let mut batch = Batch::new().with(Change::PutAccount(account(1))).with(Change::PutFolder(folder(1, 1, "INBOX")));
    let named = |n: u64| inboxcraft_model::MessageRef::new(&format!("m{n}@example.org")).unwrap();
    for id in 1..=MESSAGES {
        let message = message(id, 1, 1, u32::try_from(id).unwrap(), "A subject of a usual length, said once", Some(1));
        let envelope = message
            .summary()
            .envelope()
            .clone()
            .with_message_id(Some(named(id)))
            .with_in_reply_to(vec![named(id.saturating_sub(1))])
            .with_references((1..=10).map(|back| named(id.saturating_sub(back))).collect());
        let summary = inboxcraft_model::Summary::new(message.summary().uid(), envelope);
        batch.push(Change::PutMessage(inboxcraft_model::Message::new(message.id(), message.account(), message.folder(), summary)));
    }
    store.apply(&batch).unwrap();
    drop(store);
    Connection::open(dir.join(DATABASE_FILE)).unwrap().execute_batch(BACK_TO_VERSION_5).unwrap();
    let start = std::time::Instant::now();
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let took = start.elapsed();
    let rows: i64 = Connection::open(dir.join(DATABASE_FILE)).unwrap().query_row("SELECT count(*) FROM message_link", [], |row| row.get(0)).unwrap();
    drop(store);
    println!("indexing the links of {MESSAGES} messages ({rows} rows) on open: {:.3} s", took.as_secs_f64());
}
