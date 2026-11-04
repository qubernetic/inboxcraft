// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The database schema and its migrations, counted in `PRAGMA user_version`.

use rusqlite::{Connection, TransactionBehavior};

use crate::StoreError;

/// The schema version this build writes: the number of [`MIGRATIONS`].
pub const SCHEMA_VERSION: u32 = 2;

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
