// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The native backend: SQLite with a full-text index, the message sources as files next to it.

mod schema;
mod sources;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use inboxcraft_model::{Account, AccountId, Folder, FolderId, FolderState, Message, MessageId};
use inboxcraft_time::Timestamp;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
pub use schema::SCHEMA_VERSION;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::memory::with_flags;
use crate::search::Fields;
use crate::{Batch, Body, Change, Entry, Search, Storage, StoreConfig, StoreError};

/// The database file inside the store directory.
pub const DATABASE_FILE: &str = "store.sqlite3";

/// The directory of the message sources inside the store directory.
pub const SOURCES_DIR: &str = "sources";

/// A store in a directory: [`DATABASE_FILE`] (SQLite, WAL, `synchronous=FULL`, foreign keys on) and the message
/// sources in [`SOURCES_DIR`].
///
/// One process at a time opens a directory; the app's single-instance guard sees to that.
#[derive(Debug)]
pub struct Sqlite {
    conn: Connection,
    sources: PathBuf,
    config: StoreConfig,
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(_: serde_json::Error) -> Self {
        Self::Corrupt("a record")
    }
}

/// The source files a batch wrote (removed if it fails) and dropped (removed once it committed).
#[derive(Default)]
struct Files {
    written: Vec<i64>,
    dropped: Vec<i64>,
}

impl Sqlite {
    /// Opens the store in `dir`, creating it if needed: migrates the schema to [`SCHEMA_VERSION`] and removes the
    /// source files a kill left behind.
    ///
    /// # Errors
    ///
    /// [`StoreError::NewerSchema`] for a store from a newer build; a database or file failure.
    pub fn open(dir: &Path, config: StoreConfig) -> Result<Self, StoreError> {
        let sources = dir.join(SOURCES_DIR);
        fs::create_dir_all(&sources)?;
        let mut conn = Connection::open(dir.join(DATABASE_FILE))?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;")?;
        schema::migrate(&mut conn)?;
        let kept = conn.prepare("SELECT id FROM body")?.query_map([], |row| row.get(0))?.collect::<Result<BTreeSet<i64>, _>>()?;
        sources::sweep(&sources, &kept)?;
        Ok(Self { conn, sources, config })
    }

    /// The schema version of the open database.
    ///
    /// # Errors
    ///
    /// A database failure.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        Ok(self.conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    fn entries(&self, filter: &str, key: i64) -> Result<Vec<Entry>, StoreError> {
        let sql =
            format!("SELECT record, opened, EXISTS (SELECT 1 FROM body WHERE body.message = message.id) FROM message WHERE {filter} ORDER BY uid");
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map([key], |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?, row.get::<_, bool>(2)?)))?;
        rows.map(|row| {
            let (record, opened, has_body) = row?;
            Ok(Entry::new(serde_json::from_str(&record)?, opened, has_body))
        })
        .collect()
    }

    fn ids(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<MessageId>, StoreError> {
        let mut statement = self.conn.prepare(sql)?;
        let rows = statement.query_map(params, |row| row.get::<_, i64>(0))?;
        rows.map(|row| u64::try_from(row?).map(MessageId::new).map_err(|_| StoreError::Corrupt("a message id"))).collect()
    }
}

/// An id as SQLite stores it.
fn key(id: u64) -> Result<i64, StoreError> {
    i64::try_from(id).map_err(|_| StoreError::Database(format!("id {id} is out of range")))
}

fn json<T: Serialize>(value: &T) -> Result<String, StoreError> {
    Ok(serde_json::to_string(value)?)
}

fn record(row: &Row<'_>) -> rusqlite::Result<String> {
    row.get(0)
}

fn parse<T: DeserializeOwned>(text: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_str(text)?)
}

/// The account a held folder belongs to.
fn folder_account(tx: &Transaction<'_>, folder: FolderId) -> Result<Option<i64>, StoreError> {
    Ok(tx.query_row("SELECT account FROM folder WHERE id = ?1", [key(folder.get())?], |row| row.get(0)).optional()?)
}

/// The key of a held message.
fn exists(tx: &Transaction<'_>, id: MessageId) -> Result<i64, StoreError> {
    let held = key(id.get())?;
    let found = tx.query_row("SELECT 1 FROM message WHERE id = ?1", [held], |_| Ok(())).optional()?;
    found.map(|()| held).ok_or(StoreError::UnknownMessage(id))
}

/// The body rows (source files) below the rows `filter` picks, to remove once the batch commits.
fn drop_bodies(tx: &Transaction<'_>, filter: &str, id: i64, files: &mut Files) -> Result<(), StoreError> {
    let sql = format!("SELECT body.id FROM body JOIN message ON message.id = body.message WHERE {filter}");
    let mut statement = tx.prepare(&sql)?;
    for row in statement.query_map([id], |row| row.get(0))? {
        files.dropped.push(row?);
    }
    Ok(())
}

fn change(tx: &Transaction<'_>, sources: &Path, change: &Change, files: &mut Files) -> Result<(), StoreError> {
    match change {
        Change::PutAccount(account) => {
            tx.execute(
                "INSERT INTO account (id, record) VALUES (?1, ?2) ON CONFLICT (id) DO UPDATE SET record = excluded.record",
                params![key(account.id().get())?, json(account)?],
            )?;
        }
        Change::RemoveAccount(id) => {
            let id = key(id.get())?;
            drop_bodies(tx, "message.account = ?1", id, files)?;
            tx.execute("DELETE FROM account WHERE id = ?1", [id])?;
        }
        Change::PutFolder(folder) => put_folder(tx, folder)?,
        Change::RemoveFolder(id) => {
            let id = key(id.get())?;
            drop_bodies(tx, "message.folder = ?1", id, files)?;
            tx.execute("DELETE FROM folder WHERE id = ?1", [id])?;
        }
        Change::SetFolderState(id, state) => {
            if tx.execute("UPDATE folder SET state = ?2 WHERE id = ?1", params![key(id.get())?, json(state)?])? == 0 {
                return Err(StoreError::UnknownFolder(*id));
            }
        }
        Change::PutMessage(message) => put_message(tx, message)?,
        Change::SetFlags(id, flags) => {
            let id_key = key(id.get())?;
            let held: Option<String> = tx.query_row("SELECT record FROM message WHERE id = ?1", [id_key], record).optional()?;
            let message: Message = parse(&held.ok_or(StoreError::UnknownMessage(*id))?)?;
            tx.execute("UPDATE message SET record = ?2 WHERE id = ?1", params![id_key, json(&with_flags(&message, flags))?])?;
        }
        Change::RemoveMessage(id) => {
            let id = key(id.get())?;
            drop_bodies(tx, "message.id = ?1", id, files)?;
            tx.execute("DELETE FROM message WHERE id = ?1", [id])?;
        }
        Change::PutBody(id, body) => put_body(tx, sources, exists(tx, *id)?, body, files)?,
        Change::RemoveBody(id) => {
            let id = key(id.get())?;
            drop_bodies(tx, "message.id = ?1", id, files)?;
            tx.execute("DELETE FROM body WHERE message = ?1", [id])?;
        }
        Change::MarkOpened(id) => {
            tx.execute("UPDATE message SET opened = 1 WHERE id = ?1", [exists(tx, *id)?])?;
        }
    }
    Ok(())
}

fn put_folder(tx: &Transaction<'_>, folder: &Folder) -> Result<(), StoreError> {
    let account = key(folder.account().get())?;
    if tx.query_row("SELECT id FROM account WHERE id = ?1", [account], |row| row.get::<_, i64>(0)).optional()?.is_none() {
        return Err(StoreError::UnknownAccount(folder.account()));
    }
    if folder_account(tx, folder.id())?.is_some_and(|held| held != account) {
        return Err(StoreError::WrongAccount { folder: folder.id(), account: folder.account() });
    }
    tx.execute(
        "INSERT INTO folder (id, account, record) VALUES (?1, ?2, ?3) ON CONFLICT (id) DO UPDATE SET record = excluded.record",
        params![key(folder.id().get())?, account, json(folder)?],
    )?;
    Ok(())
}

fn put_message(tx: &Transaction<'_>, message: &Message) -> Result<(), StoreError> {
    let folder = message.folder();
    match folder_account(tx, folder)? {
        None => return Err(StoreError::UnknownFolder(folder)),
        Some(account) if account != key(message.account().get())? => {
            return Err(StoreError::WrongAccount { folder, account: message.account() });
        }
        Some(_) => {}
    }
    let (id, uid) = (key(message.id().get())?, message.summary().uid());
    let other: Option<i64> =
        tx.query_row("SELECT id FROM message WHERE folder = ?1 AND uid = ?2", params![key(folder.get())?, uid.get()], |row| row.get(0)).optional()?;
    if other.is_some_and(|other| other != id) {
        return Err(StoreError::DuplicateUid { folder, uid });
    }
    let fields = Fields::of(message);
    tx.execute(
        "INSERT INTO message (id, account, folder, uid, date, from_text, to_text, subject_text, record)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT (id) DO UPDATE SET account = excluded.account, folder = excluded.folder, uid = excluded.uid,
             date = excluded.date, from_text = excluded.from_text, to_text = excluded.to_text,
             subject_text = excluded.subject_text, record = excluded.record",
        params![
            id,
            key(message.account().get())?,
            key(folder.get())?,
            uid.get(),
            fields.date,
            fields.from,
            fields.to,
            fields.subject,
            json(message)?
        ],
    )?;
    Ok(())
}

fn put_body(tx: &Transaction<'_>, sources: &Path, message: i64, body: &Body, files: &mut Files) -> Result<(), StoreError> {
    drop_bodies(tx, "message.id = ?1", message, files)?;
    tx.execute("DELETE FROM body WHERE message = ?1", [message])?;
    let length = i64::try_from(body.source().len()).map_err(|_| StoreError::TooLarge)?;
    tx.execute("INSERT INTO body (message, length) VALUES (?1, ?2)", [message, length])?;
    let id = tx.last_insert_rowid();
    files.written.push(id);
    sources::write(sources, id, body.source())?;
    tx.execute("INSERT INTO body_text (rowid, text) VALUES (?1, ?2)", params![message, body.text()])?;
    Ok(())
}

impl Storage for Sqlite {
    fn config(&self) -> StoreConfig {
        self.config
    }

    fn apply(&mut self, batch: &Batch) -> Result<(), StoreError> {
        let mut files = Files::default();
        let sources = &self.sources;
        let applied = self.conn.transaction().map_err(StoreError::from).and_then(|tx| {
            for each in batch.changes() {
                change(&tx, sources, each, &mut files)?;
            }
            Ok(tx.commit()?)
        });
        let (remove, result) = match applied {
            Ok(()) => (files.dropped, Ok(())),
            Err(error) => (files.written, Err(error)),
        };
        for id in remove {
            sources::remove(sources, id);
        }
        result
    }

    fn accounts(&self) -> Result<Vec<Account>, StoreError> {
        let mut statement = self.conn.prepare("SELECT record FROM account ORDER BY id")?;
        let rows = statement.query_map([], record)?;
        rows.map(|row| parse(&row?)).collect()
    }

    fn folders(&self, account: AccountId) -> Result<Vec<Folder>, StoreError> {
        let mut statement = self.conn.prepare("SELECT record FROM folder WHERE account = ?1 ORDER BY id")?;
        let rows = statement.query_map([key(account.get())?], record)?;
        rows.map(|row| parse(&row?)).collect()
    }

    fn folder_state(&self, folder: FolderId) -> Result<Option<FolderState>, StoreError> {
        let state: Option<Option<String>> =
            self.conn.query_row("SELECT state FROM folder WHERE id = ?1", [key(folder.get())?], |row| row.get(0)).optional()?;
        state.flatten().map(|state| parse(&state)).transpose()
    }

    fn messages(&self, folder: FolderId) -> Result<Vec<Entry>, StoreError> {
        self.entries("folder = ?1", key(folder.get())?)
    }

    fn message(&self, id: MessageId) -> Result<Option<Entry>, StoreError> {
        Ok(self.entries("id = ?1", key(id.get())?)?.into_iter().next())
    }

    fn body(&self, id: MessageId) -> Result<Option<Body>, StoreError> {
        let row: Option<(i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT body.id, body.length, body_text.text FROM body JOIN body_text ON body_text.rowid = body.message WHERE body.message = ?1",
                [key(id.get())?],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((file, length, text)) = row else { return Ok(None) };
        Ok(Some(Body::new(sources::read(&self.sources, file, length)?, &text)?))
    }

    fn search(&self, search: &Search) -> Result<Vec<MessageId>, StoreError> {
        let account = search.account().map(|account| key(account.get())).transpose()?;
        let folder = search.folder().map(|folder| key(folder.get())).transpose()?;
        let text = (!search.text().is_empty()).then(|| format!("\"{}\"", search.text().join(" ")));
        let limit = i64::try_from(search.limit()).unwrap_or(i64::MAX);
        self.ids(
            "SELECT id FROM message
             WHERE (?1 IS NULL OR account = ?1) AND (?2 IS NULL OR folder = ?2)
               AND (?3 IS NULL OR instr(from_text, ?3) > 0) AND (?4 IS NULL OR instr(to_text, ?4) > 0)
               AND (?5 IS NULL OR instr(subject_text, ?5) > 0)
               AND (?6 IS NULL OR date >= ?6) AND (?7 IS NULL OR date < ?7)
               AND (?8 IS NULL OR id IN (SELECT rowid FROM body_text WHERE body_text MATCH ?8))
             ORDER BY date IS NULL, date DESC, id DESC LIMIT ?9",
            params![account, folder, search.from(), search.to(), search.subject(), search.since(), search.before(), text, limit],
        )
    }

    fn stale_bodies(&self, cutoff: Timestamp) -> Result<Vec<MessageId>, StoreError> {
        self.ids(
            "SELECT message.id FROM message JOIN body ON body.message = message.id
             WHERE message.opened = 0 AND message.date < ?1 ORDER BY message.id",
            [cutoff.unix_seconds()],
        )
    }

    fn missing_bodies(&self, folder: FolderId, cutoff: Timestamp) -> Result<Vec<MessageId>, StoreError> {
        self.ids(
            "SELECT id FROM message
             WHERE folder = ?1 AND NOT EXISTS (SELECT 1 FROM body WHERE body.message = message.id)
               AND (opened = 1 OR date IS NULL OR date >= ?2)
             ORDER BY uid",
            params![key(folder.get())?, cutoff.unix_seconds()],
        )
    }
}
