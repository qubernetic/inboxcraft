// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The database schema and its migrations, counted in `PRAGMA user_version`.

use rusqlite::{Connection, TransactionBehavior};

use crate::StoreError;

/// The schema version this build writes: the number of [`MIGRATIONS`].
pub const SCHEMA_VERSION: u32 = 4;

/// Migration `n` (from 0) takes a database from version `n` to `n + 1`. A released migration never changes; a new
/// schema is a new migration at the end.
pub(crate) const MIGRATIONS: [&str; SCHEMA_VERSION as usize] = [
    // 1: the rows. A body row's id names its source file, `sources/<id>.eml`; AUTOINCREMENT never hands out an id
    // twice, so a new source never overwrites a committed one.
    "CREATE TABLE account (
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
    );",
    // 2: the body text moves into a full-text index, kept in step with the body rows.
    "CREATE VIRTUAL TABLE body_text USING fts5 (text, tokenize = 'unicode61 remove_diacritics 0');
    INSERT INTO body_text (rowid, text) SELECT message, text FROM body;
    ALTER TABLE body DROP COLUMN text;
    CREATE TRIGGER body_text_gone AFTER DELETE ON body BEGIN
        DELETE FROM body_text WHERE rowid = old.message;
    END;",
    // 3: a deleted body's words leave the index segments at once, not at some later merge.
    "INSERT INTO body_text (body_text, rank) VALUES ('secure-delete', 1);",
    // 4: the local records beside the mail, every table the daily-use features need. Each row holds its record as
    // JSON, so a new field of a record needs no migration; the columns beside it are what rows are found and ordered
    // by. A job's place in the queue is its `seq`, which AUTOINCREMENT never hands out twice. A schedule entry names
    // its draft, message or alert in one of three columns and leaves with it: an id can be handed out again once its
    // record is gone, and an entry left behind would act on the new one.
    "CREATE TABLE job (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        id INTEGER NOT NULL UNIQUE,
        account INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
        record TEXT NOT NULL
    );
    CREATE INDEX job_account ON job (account);
    CREATE TABLE draft (
        id INTEGER PRIMARY KEY,
        account INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
        record TEXT NOT NULL
    );
    CREATE INDEX draft_account ON draft (account, id);
    CREATE TABLE outbox (
        draft INTEGER PRIMARY KEY REFERENCES draft (id) ON DELETE CASCADE,
        record TEXT NOT NULL
    );
    CREATE TABLE schedule (
        id INTEGER PRIMARY KEY,
        due INTEGER NOT NULL,
        draft INTEGER REFERENCES draft (id) ON DELETE CASCADE,
        message INTEGER REFERENCES message (id) ON DELETE CASCADE,
        alert INTEGER REFERENCES alert (id) ON DELETE CASCADE,
        record TEXT NOT NULL
    );
    CREATE INDEX schedule_due ON schedule (due, id);
    CREATE INDEX schedule_draft ON schedule (draft);
    CREATE INDEX schedule_message ON schedule (message);
    CREATE INDEX schedule_alert ON schedule (alert);
    CREATE TABLE sender (
        address TEXT PRIMARY KEY,
        record TEXT NOT NULL
    );
    CREATE TABLE message_state (
        message INTEGER PRIMARY KEY REFERENCES message (id) ON DELETE CASCADE,
        record TEXT NOT NULL
    );
    CREATE TABLE calendar (
        id INTEGER PRIMARY KEY,
        account INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
        record TEXT NOT NULL,
        sync_token TEXT
    );
    CREATE INDEX calendar_account ON calendar (account, id);
    CREATE TABLE calendar_event (
        id INTEGER PRIMARY KEY,
        calendar INTEGER NOT NULL REFERENCES calendar (id) ON DELETE CASCADE,
        record TEXT NOT NULL
    );
    CREATE INDEX calendar_event_calendar ON calendar_event (calendar, id);
    CREATE TABLE alert (
        id INTEGER PRIMARY KEY,
        event INTEGER NOT NULL REFERENCES calendar_event (id) ON DELETE CASCADE,
        at INTEGER NOT NULL,
        record TEXT NOT NULL
    );
    CREATE INDEX alert_event ON alert (event, at, id);",
];

/// Brings `conn` to [`SCHEMA_VERSION`] in one transaction.
///
/// # Errors
///
/// [`StoreError::NewerSchema`] for a database from a newer build, which is left as it is; a database failure.
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let found: u32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if found > SCHEMA_VERSION {
        return Err(StoreError::NewerSchema { found, known: SCHEMA_VERSION });
    }
    for migration in MIGRATIONS.iter().skip(usize::try_from(found).unwrap_or(usize::MAX)) {
        tx.execute_batch(migration)?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}
