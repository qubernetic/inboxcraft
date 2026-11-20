// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The native backend: SQLite with a full-text index, the message sources as files next to it.

mod keeper;
mod local;
mod private;
mod schema;
mod sources;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::KeepWake;
use inboxcraft_model::{
    Account, AccountId, Alert, AlertId, Calendar, CalendarEvent, CalendarEventId, CalendarId, Draft, DraftId, EmailAddress, Folder, FolderId,
    FolderState, Job, Message, MessageId, MessageState, Outgoing, Scheduled, Sender,
};
use inboxcraft_time::Timestamp;
use keeper::Keeper;
#[cfg(any(test, feature = "test-hooks"))]
pub use keeper::{KeepHold, KeeperHooks};
use rusqlite::config::DbConfig;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};
pub use schema::SCHEMA_VERSION;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::memory::{sender_key, with_flags};
use crate::search::Fields;
use crate::{Batch, Body, Change, Entry, Search, Storage, StoreConfig, StoreError};

/// The database file inside the store directory.
pub const DATABASE_FILE: &str = "store.sqlite3";

/// The directory of the message sources inside the store directory.
pub const SOURCES_DIR: &str = "sources";

/// A store in a directory: [`DATABASE_FILE`] (SQLite, WAL, `synchronous=FULL`, foreign keys on) and the message
/// sources in [`SOURCES_DIR`], all of it readable by the owner's account alone (directories `0700`, files `0600` on unix).
///
/// One process at a time may open a directory; the store takes no lock, the app's single instance guard is to see to it.
#[derive(Debug)]
pub struct Sqlite {
    conn: Connection,
    sources: PathBuf,
    config: StoreConfig,
    /// The database file, for the draft keeper's own connection.
    database: PathBuf,
    /// The draft keeper, once [`Sqlite::keep_drafts_behind`] started it.
    keeper: Option<Keeper>,
}

/// How long a batch that is not only drafts kept waits for the drafts the keeper still writes: they come first.
const SETTLE_WAIT: Duration = Duration::from_secs(3);

/// A connection to `database` as the store keeps it: no URI names, defensive, the schema not trusted (nothing the file
/// holds may call a function with side effects or write the schema: the file is outside data), WAL,
/// `synchronous=FULL`, foreign keys on, removed pages zeroed.
fn connect(database: &Path) -> Result<Connection, StoreError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(database, flags)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
    conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON;")?;
    Ok(conn)
}

/// A database failure by its kind, told apart by the database's result code, never by its words; the words are kept,
/// cut, as the detail.
impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode;
        let detail: String = error.to_string().chars().filter(|c| !c.is_control()).take(crate::MAX_ERROR_DETAIL).collect();
        match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Self::DatabaseBusy(detail),
            Some(ErrorCode::DiskFull) => Self::DatabaseFull(detail),
            Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => Self::DatabaseDamaged(detail),
            Some(ErrorCode::ConstraintViolation) => Self::DatabaseRefused(detail),
            _ => Self::Database(detail),
        }
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(_: serde_json::Error) -> Self {
        Self::Corrupt("a record")
    }
}

/// The source files a batch wrote (removed if it fails) and dropped (removed once it committed), and whether it
/// removed mail from the database.
#[derive(Default)]
struct Files {
    written: Vec<i64>,
    dropped: Vec<i64>,
    removed: bool,
}

/// Moves the write-ahead log into the database and empties it, so removed mail, whose pages `secure_delete` zeroes,
/// leaves no older copy in the log.
fn scrub(conn: &Connection) -> Result<(), StoreError> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
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
        private::create_dir(&sources)?;
        let database = dir.join(DATABASE_FILE);
        let (wal, shm) = (dir.join(format!("{DATABASE_FILE}-wal")), dir.join(format!("{DATABASE_FILE}-shm")));
        private::tighten(&[dir, &sources], &[&database, &wal, &shm])?;
        drop(private::create_file(&database, false)?);
        let mut conn = connect(&database)?;
        schema::migrate(&mut conn)?;
        scrub(&conn)?;
        let kept = conn.prepare("SELECT id FROM body")?.query_map([], |row| row.get(0))?.collect::<Result<BTreeSet<i64>, _>>()?;
        sources::sweep(&sources, &kept)?;
        Ok(Self { conn, sources, config, database, keeper: None })
    }

    /// From now on a batch that only changes drafts the store holds, each still from its account, is written behind
    /// by the draft keeper, a thread with a connection of its own, so that [`Storage::apply`] returns at once: in
    /// order, the newest of each draft, one write at a time, each with `synchronous=FULL`. The store's reads see a
    /// draft kept from the moment it was kept. A write that fails keeps the draft's newest text in memory, is told
    /// once by [`Storage::failed_keep`], listed by [`Storage::unkept`] and tried again 1 s, 2 s, 4 s … at most 30 s
    /// apart until it lands or the draft is removed; meanwhile every other batch that names the draft is refused
    /// ([`StoreError::DraftNotKept`]), except one that removes it. Any other batch first waits (at most 3 s) for the
    /// keeper, so it comes after every keep. The end of the store waits at most 3 s for what is still to be written,
    /// unless [`Storage::settle`] already waited. A batch that makes a draft, moves it to another account or does
    /// anything else is written at once, as before. If the keeper's thread ever ends, the store writes what it held
    /// and every later draft on the caller's thread, and says so once on standard error.
    ///
    /// # Errors
    ///
    /// The keeper's connection or thread could not be made; the store then writes everything at once.
    pub fn keep_drafts_behind(&mut self) -> Result<(), StoreError> {
        if self.keeper.is_none() {
            self.keeper = Some(Keeper::start(&self.database)?);
        }
        Ok(())
    }

    /// Holds the draft keeper until the hold is dropped: what is kept meanwhile waits (tests). `None` without a
    /// keeper.
    #[cfg(any(test, feature = "test-hooks"))]
    #[must_use]
    pub fn hold_keeps(&self) -> Option<KeepHold> {
        self.keeper.as_ref().map(|keeper| keeper.hooks().hold())
    }

    /// The draft keeper's test hooks, as a handle that outlives a borrow of the store (tests). `None` without a
    /// keeper.
    #[cfg(any(test, feature = "test-hooks"))]
    #[must_use]
    pub fn keeper_hooks(&self) -> Option<KeeperHooks> {
        self.keeper.as_ref().map(Keeper::hooks)
    }

    /// Makes every write of the draft keeper fail while `refuse` is on, as a full or failing disk would (tests).
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn refuse_keeps(&self, refuse: bool) {
        if let Some(keeper) = &self.keeper {
            keeper.hooks().refuse(refuse);
        }
    }

    /// How many writes the draft keeper started (tests); 0 without a keeper.
    #[cfg(any(test, feature = "test-hooks"))]
    #[must_use]
    pub fn keeper_tries(&self) -> u64 {
        self.keeper.as_ref().map_or(0, Keeper::tries)
    }

    /// Ends the draft keeper's thread as if it had died (tests).
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn stop_keeper(&self) {
        if let Some(keeper) = &self.keeper {
            keeper.stop();
        }
    }

    /// A keeper whose thread ended: what it held is written on this connection, and the store writes every draft at
    /// once from now on, said once on standard error. Until what it held is written the keeper stays, its drafts
    /// with it.
    fn outlive_keeper(&mut self) -> Result<(), StoreError> {
        if !self.write_what_a_gone_keeper_held()? {
            return Ok(());
        }
        eprintln!("inboxcraft: the draft keeper stopped; drafts are written on the window's thread from now on");
        self.keeper = None;
        Ok(())
    }

    /// When the keeper's thread ended, writes what it held on this connection and forgets it there; whether it ended.
    ///
    /// # Errors
    ///
    /// What it held could not be written: the keeper keeps it, and the disk holds those drafts as they were before.
    fn write_what_a_gone_keeper_held(&self) -> Result<bool, StoreError> {
        let Some(keeper) = self.keeper.as_ref().filter(|keeper| keeper.is_gone()) else { return Ok(false) };
        let held = keeper.all_held();
        if !held.is_empty() {
            let tx = self.conn.unchecked_transaction()?;
            for draft in &held {
                local::put_draft(&tx, draft)?;
            }
            tx.commit()?;
            for draft in &held {
                keeper.forget(draft.id);
            }
        }
        Ok(true)
    }

    /// The drafts of `batch` when the keeper may write it behind: only drafts, every one held by the store from the
    /// same account.
    fn behind(&self, batch: &Batch) -> Result<Option<Vec<Arc<Draft>>>, StoreError> {
        if self.keeper.is_none() || batch.is_empty() {
            return Ok(None);
        }
        let mut drafts = Vec::new();
        for each in batch.changes() {
            let Change::PutDraft(draft) = each else { return Ok(None) };
            let Some(id) = key(draft.id.get()) else { return Ok(None) };
            let held: Option<i64> = self.conn.query_row("SELECT account FROM draft WHERE id = ?1", [id], |row| row.get(0)).optional()?;
            if held.is_none() || held != key(draft.account.get()) {
                return Ok(None);
            }
            drafts.push(Arc::new(draft.clone()));
        }
        Ok(Some(drafts))
    }

    /// The draft `id` as the store holds it: the keep that waits for the disk, or the database's.
    fn held_draft(&self, id: DraftId) -> Result<Option<Draft>, StoreError> {
        if let Some(draft) = self.keeper.as_ref().and_then(|keeper| keeper.held(id)) {
            return Ok(Some(Draft::clone(&draft)));
        }
        self.one("SELECT record FROM draft WHERE id = ?1", [key(id.get())])
    }

    /// The schema version of the open database.
    ///
    /// # Errors
    ///
    /// A database failure.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        Ok(self.conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    /// The store's own id: 32 lower-case hex digits, random, made once with the store (schema version 4) and kept for
    /// good. What lives outside the store and belongs to it, such as its accounts' passwords in the OS credential
    /// store, is named with it, so two stores on one computer never share it.
    ///
    /// # Errors
    ///
    /// [`StoreError::Corrupt`] when the id is gone; a database failure.
    pub fn id(&self) -> Result<String, StoreError> {
        let id: Option<String> = self.conn.query_row("SELECT value FROM meta WHERE key = 'store'", [], |row| row.get(0)).optional()?;
        id.ok_or(StoreError::Corrupt("the store's id"))
    }

    fn entries(&self, filter: &str, key: Option<i64>) -> Result<Vec<Entry>, StoreError> {
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

    /// The records `sql` picks, its first column each.
    fn records<T: DeserializeOwned>(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<T>, StoreError> {
        let mut statement = self.conn.prepare(sql)?;
        let rows = statement.query_map(params, record)?;
        rows.map(|row| parse(&row?)).collect()
    }

    /// The one record `sql` picks, if any.
    fn one<T: DeserializeOwned>(&self, sql: &str, params: impl rusqlite::Params) -> Result<Option<T>, StoreError> {
        let held: Option<String> = self.conn.query_row(sql, params, record).optional()?;
        held.map(|text| parse(&text)).transpose()
    }

    fn ids(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<MessageId>, StoreError> {
        let mut statement = self.conn.prepare(sql)?;
        let rows = statement.query_map(params, |row| row.get::<_, i64>(0))?;
        rows.map(|row| u64::try_from(row?).map(MessageId::new).map_err(|_| StoreError::Corrupt("a message id"))).collect()
    }
}

/// The FTS5 query for `words` in this order: one string in double quotes, a `"` inside doubled, so FTS5 reads every
/// word, `OR`, `NEAR` and `*` too, as a word to find. No words, no query.
fn phrase(words: &[String]) -> Option<String> {
    (!words.is_empty()).then(|| format!("\"{}\"", words.join(" ").replace('"', "\"\"")))
}

/// The draft a change names, if one.
fn drafted(change: &Change) -> Option<DraftId> {
    match change {
        Change::PutDraft(draft) => Some(draft.id),
        Change::RemoveDraft(id) | Change::RemoveOutgoing(id) => Some(*id),
        Change::PutOutgoing(outgoing) => Some(outgoing.draft),
        Change::PutScheduled(Scheduled { what: inboxcraft_model::Due::Send(id), .. }) => Some(*id),
        _ => None,
    }
}

/// Whether `batch` removes draft `id`, itself or with its account, as `keeper` holds it.
fn removes(batch: &Batch, id: DraftId, keeper: &Keeper) -> bool {
    let account = keeper.held(id).map(|draft| draft.account);
    batch.changes().iter().any(|change| match change {
        Change::RemoveDraft(removed) => *removed == id,
        Change::RemoveAccount(removed) => Some(*removed) == account,
        _ => false,
    })
}

/// The newest drafts of account `?1`, at most `?2`, in the order of the `draft_newest` index.
const NEWEST_DRAFTS: &str = "SELECT record FROM draft WHERE account = ?1 ORDER BY saved DESC, saved_nanos DESC, id DESC LIMIT ?2";

/// An id as SQLite looks it up: `None` past `i64::MAX`, which no key can be. `None` binds as NULL, and `= NULL` is
/// true for no row whatever the file holds (a negative key in a damaged file too), so the lookup finds nothing, as for
/// an unknown id: a read gives nothing, a change that needs the thing gives the kind's `Unknown…` error and a removal
/// removes nothing. No caller has to tell the two apart.
///
/// Bind it only in `= ?n`: `?n IS NULL` reads NULL as "no filter" ([`Storage::search`] answers such a filter
/// itself). A value written as a reference to another row is the key read back from that row by the existence check
/// before the write ([`exists`], `local::held`, [`folder_account`]), never this.
fn key(id: u64) -> Option<i64> {
    i64::try_from(id).ok()
}

/// A search filter on `id`: `None` when it can match nothing (an id no key can be), else the value for
/// `?n IS NULL OR column = ?n`, `Some(None)` for no filter.
fn filter(id: Option<u64>) -> Option<Option<i64>> {
    match id {
        None => Some(None),
        Some(id) => key(id).map(Some),
    }
}

/// A record's own id as SQLite stores it; [`StoreError::IdOutOfRange`] past `i64::MAX`.
fn stored(id: u64) -> Result<i64, StoreError> {
    i64::try_from(id).map_err(|_| StoreError::IdOutOfRange(id))
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
    Ok(tx.query_row("SELECT account FROM folder WHERE id = ?1", [key(folder.get())], |row| row.get(0)).optional()?)
}

/// The key of a held message, as its row holds it.
fn exists(tx: &Transaction<'_>, id: MessageId) -> Result<i64, StoreError> {
    let found = tx.query_row("SELECT id FROM message WHERE id = ?1", [key(id.get())], |row| row.get(0)).optional()?;
    found.ok_or(StoreError::UnknownMessage(id))
}

/// The body rows (source files) below the rows `filter` picks, to remove once the batch commits.
fn drop_bodies(tx: &Transaction<'_>, filter: &str, id: Option<i64>, files: &mut Files) -> Result<(), StoreError> {
    let sql = format!("SELECT body.id FROM body JOIN message ON message.id = body.message WHERE {filter}");
    let mut statement = tx.prepare(&sql)?;
    for row in statement.query_map([id], |row| row.get(0))? {
        files.dropped.push(row?);
    }
    Ok(())
}

fn change(tx: &Transaction<'_>, sources: &Path, change: &Change, files: &mut Files) -> Result<(), StoreError> {
    files.removed |= matches!(
        change,
        Change::RemoveAccount(_)
            | Change::RemoveFolder(_)
            | Change::RemoveMessage(_)
            | Change::RemoveBody(_)
            | Change::RemoveDraft(_)
            | Change::RemoveOutgoing(_)
            | Change::RemoveSender(_)
            | Change::RemoveCalendar(_)
            | Change::SetSyncToken(_, None)
            | Change::RemoveCalendarEvent(_)
    );
    match change {
        Change::PutAccount(account) => {
            tx.execute(
                "INSERT INTO account (id, record) VALUES (?1, ?2) ON CONFLICT (id) DO UPDATE SET record = excluded.record",
                params![stored(account.id().get())?, json(account)?],
            )?;
        }
        Change::RemoveAccount(id) => {
            let id = key(id.get());
            drop_bodies(tx, "message.account = ?1", id, files)?;
            tx.execute("DELETE FROM account WHERE id = ?1", [id])?;
        }
        Change::PutFolder(folder) => put_folder(tx, folder)?,
        Change::RemoveFolder(id) => {
            let id = key(id.get());
            drop_bodies(tx, "message.folder = ?1", id, files)?;
            tx.execute("DELETE FROM folder WHERE id = ?1", [id])?;
        }
        Change::SetFolderState(id, state) => {
            if tx.execute("UPDATE folder SET state = ?2 WHERE id = ?1", params![key(id.get()), json(state)?])? == 0 {
                return Err(StoreError::UnknownFolder(*id));
            }
        }
        Change::PutMessage(message) => put_message(tx, message)?,
        Change::SetFlags(id, flags) => {
            let held: Option<(i64, String)> =
                tx.query_row("SELECT id, record FROM message WHERE id = ?1", [key(id.get())], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
            let (row, text) = held.ok_or(StoreError::UnknownMessage(*id))?;
            let message: Message = parse(&text)?;
            tx.execute("UPDATE message SET record = ?2 WHERE id = ?1", params![row, json(&with_flags(&message, flags))?])?;
        }
        Change::RemoveMessage(id) => {
            let id = key(id.get());
            drop_bodies(tx, "message.id = ?1", id, files)?;
            tx.execute("DELETE FROM message WHERE id = ?1", [id])?;
        }
        Change::PutBody(id, body) => put_body(tx, sources, exists(tx, *id)?, body, files)?,
        Change::RemoveBody(id) => {
            let id = key(id.get());
            drop_bodies(tx, "message.id = ?1", id, files)?;
            tx.execute("DELETE FROM body WHERE message = ?1", [id])?;
        }
        Change::MarkOpened(id) => {
            tx.execute("UPDATE message SET opened = 1 WHERE id = ?1", [exists(tx, *id)?])?;
        }
        Change::QueueJob(job) => local::queue_job(tx, job)?,
        Change::RemoveJob(id) => local::remove(tx, "job", "id", id.get())?,
        Change::PutDraft(draft) => local::put_draft(tx, draft)?,
        Change::RemoveDraft(id) => local::remove(tx, "draft", "id", id.get())?,
        Change::PutOutgoing(outgoing) => local::put_outgoing(tx, outgoing)?,
        Change::RemoveOutgoing(id) => local::remove(tx, "outbox", "draft", id.get())?,
        Change::PutScheduled(entry) => local::put_scheduled(tx, entry)?,
        Change::RemoveScheduled(id) => local::remove(tx, "schedule", "id", id.get())?,
        Change::PutSender(sender) => local::put_sender(tx, sender)?,
        Change::RemoveSender(address) => {
            tx.execute("DELETE FROM sender WHERE address = ?1", [sender_key(address)])?;
        }
        Change::SetMessageState(id, state) => local::set_message_state(tx, *id, state)?,
        Change::PutCalendar(calendar) => local::put_calendar(tx, calendar)?,
        Change::RemoveCalendar(id) => local::remove(tx, "calendar", "id", id.get())?,
        Change::SetSyncToken(id, token) => local::set_sync_token(tx, *id, token.as_deref())?,
        Change::PutCalendarEvent(event) => local::put_calendar_event(tx, event)?,
        Change::RemoveCalendarEvent(id) => local::remove(tx, "calendar_event", "id", id.get())?,
        Change::PutAlert(alert) => local::put_alert(tx, alert)?,
        Change::RemoveAlert(id) => local::remove(tx, "alert", "id", id.get())?,
    }
    Ok(())
}

fn put_folder(tx: &Transaction<'_>, folder: &Folder) -> Result<(), StoreError> {
    let held = tx.query_row("SELECT id FROM account WHERE id = ?1", [key(folder.account().get())], |row| row.get::<_, i64>(0)).optional()?;
    let Some(account) = held else { return Err(StoreError::UnknownAccount(folder.account())) };
    if folder_account(tx, folder.id())?.is_some_and(|held| held != account) {
        return Err(StoreError::WrongAccount { folder: folder.id(), account: folder.account() });
    }
    tx.execute(
        "INSERT INTO folder (id, account, record) VALUES (?1, ?2, ?3) ON CONFLICT (id) DO UPDATE SET record = excluded.record",
        params![stored(folder.id().get())?, account, json(folder)?],
    )?;
    Ok(())
}

fn put_message(tx: &Transaction<'_>, message: &Message) -> Result<(), StoreError> {
    let folder = message.folder();
    let account = match folder_account(tx, folder)? {
        None => return Err(StoreError::UnknownFolder(folder)),
        Some(account) if Some(account) != key(message.account().get()) => {
            return Err(StoreError::WrongAccount { folder, account: message.account() });
        }
        Some(account) => account,
    };
    let (id, folder_key, uid) = (stored(message.id().get())?, stored(folder.get())?, message.summary().uid());
    let other: Option<i64> =
        tx.query_row("SELECT id FROM message WHERE folder = ?1 AND uid = ?2", params![folder_key, uid.get()], |row| row.get(0)).optional()?;
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
        params![id, account, folder_key, uid.get(), fields.date, fields.from, fields.to, fields.subject, json(message)?],
    )?;
    Ok(())
}

fn put_body(tx: &Transaction<'_>, sources: &Path, message: i64, body: &Body, files: &mut Files) -> Result<(), StoreError> {
    drop_bodies(tx, "message.id = ?1", Some(message), files)?;
    files.removed |= !files.dropped.is_empty();
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
        self.outlive_keeper()?;
        if let Some(drafts) = self.behind(batch)?
            && let Some(keeper) = &self.keeper
        {
            keeper.keep(drafts);
            return Ok(());
        }
        // Every keep comes before this batch, and no new try of a failed one starts while it is written: a send or a
        // discard after a keep finds the draft as kept, and a discarded draft is never written again.
        let gate = match &self.keeper {
            Some(keeper) => {
                let named: Vec<DraftId> = batch.changes().iter().filter_map(drafted).collect();
                // A batch on a draft whose write failed first gives that write one more try.
                keeper.retry(&named);
                let gate = keeper.gate(SETTLE_WAIT)?;
                // Still not on the disk: the batch would act on the draft as it was before. A batch that removes the
                // draft (a discard, its account removed) goes on.
                if let Some(id) = named.iter().copied().find(|id| keeper.failing(*id).is_some() && !removes(batch, *id, keeper)) {
                    return Err(StoreError::DraftNotKept(id));
                }
                Some(gate)
            }
            None => None,
        };
        let mut files = Files::default();
        let sources = &self.sources;
        let applied = self.conn.transaction().map_err(StoreError::from).and_then(|tx| {
            for each in batch.changes() {
                change(&tx, sources, each, &mut files)?;
            }
            Ok(tx.commit()?)
        });
        let (remove, result) = match applied {
            Ok(()) => {
                if files.removed {
                    // The batch is committed whatever the checkpoint does; a failed one is done again at the next open.
                    let _ = scrub(&self.conn);
                }
                (files.dropped, Ok(()))
            }
            Err(error) => (files.written, Err(error)),
        };
        for id in remove {
            sources::remove(sources, id);
        }
        if result.is_ok()
            && let Some(keeper) = &self.keeper
        {
            for draft in keeper.all_held() {
                if removes(batch, draft.id, keeper) {
                    keeper.forget(draft.id);
                }
            }
        }
        drop(gate);
        result
    }

    fn accounts(&self) -> Result<Vec<Account>, StoreError> {
        let mut statement = self.conn.prepare("SELECT record FROM account ORDER BY id")?;
        let rows = statement.query_map([], record)?;
        rows.map(|row| parse(&row?)).collect()
    }

    fn folders(&self, account: AccountId) -> Result<Vec<Folder>, StoreError> {
        let mut statement = self.conn.prepare("SELECT record FROM folder WHERE account = ?1 ORDER BY id")?;
        let rows = statement.query_map([key(account.get())], record)?;
        rows.map(|row| parse(&row?)).collect()
    }

    fn folder_state(&self, folder: FolderId) -> Result<Option<FolderState>, StoreError> {
        let state: Option<Option<String>> =
            self.conn.query_row("SELECT state FROM folder WHERE id = ?1", [key(folder.get())], |row| row.get(0)).optional()?;
        state.flatten().map(|state| parse(&state)).transpose()
    }

    fn messages(&self, folder: FolderId) -> Result<Vec<Entry>, StoreError> {
        self.entries("folder = ?1", key(folder.get()))
    }

    fn message(&self, id: MessageId) -> Result<Option<Entry>, StoreError> {
        Ok(self.entries("id = ?1", key(id.get()))?.into_iter().next())
    }

    fn body(&self, id: MessageId) -> Result<Option<Body>, StoreError> {
        let row: Option<(i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT body.id, body.length, body_text.text FROM body JOIN body_text ON body_text.rowid = body.message WHERE body.message = ?1",
                [key(id.get())],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((file, length, text)) = row else { return Ok(None) };
        Ok(Some(Body::new(sources::read(&self.sources, file, length)?, &text)?))
    }

    fn search(&self, search: &Search) -> Result<Vec<MessageId>, StoreError> {
        // A filter on an id no key can be matches nothing; bound as NULL it would match everything.
        let (Some(account), Some(folder)) = (filter(search.account().map(AccountId::get)), filter(search.folder().map(FolderId::get))) else {
            return Ok(Vec::new());
        };
        let text = phrase(search.text());
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

    fn queued_jobs(&self) -> Result<Vec<Job>, StoreError> {
        self.records("SELECT record FROM job ORDER BY seq", [])
    }

    fn drafts(&self, account: AccountId) -> Result<Vec<Draft>, StoreError> {
        let mut drafts: Vec<Draft> = self.records("SELECT record FROM draft WHERE account = ?1 ORDER BY id", [key(account.get())])?;
        // A keep still on its way takes its draft's place: only a draft the store holds is kept behind.
        if let Some(keeper) = &self.keeper {
            for held in keeper.held_of(account) {
                if let Some(draft) = drafts.iter_mut().find(|draft| draft.id == held.id) {
                    *draft = Draft::clone(&held);
                }
            }
        }
        Ok(drafts)
    }

    fn newest_drafts(&self, of: AccountId, limit: usize) -> Result<(u64, Vec<Draft>), StoreError> {
        let account = key(of.get());
        let total: i64 = self.conn.query_row("SELECT count(*) FROM draft WHERE account = ?1", [account], |row| row.get(0))?;
        let total = u64::try_from(total).unwrap_or(0);
        let held = self.keeper.as_ref().map(|keeper| keeper.held_of(of)).unwrap_or_default();
        // The keeps on their way are newer than the disk says: as many more rows as there are keeps hold every draft
        // that is not one of them and is among the newest `limit`.
        let wanted = limit.saturating_add(held.len());
        // `draft_newest` holds this order, so the scan stops after `wanted` rows.
        let mut drafts: Vec<Draft> = self.records(NEWEST_DRAFTS, params![account, i64::try_from(wanted).unwrap_or(i64::MAX)])?;
        if held.is_empty() {
            return Ok((total, drafts));
        }
        drafts.retain(|draft| !held.iter().any(|kept| kept.id == draft.id));
        drafts.extend(held.iter().map(|kept| Draft::clone(kept)));
        drafts.sort_by(|a, b| b.saved.cmp(&a.saved).then(b.id.cmp(&a.id)));
        drafts.truncate(limit);
        Ok((total, drafts))
    }

    fn draft(&self, id: DraftId) -> Result<Option<Draft>, StoreError> {
        self.held_draft(id)
    }

    fn settle(&self, within: Duration) -> Result<(), StoreError> {
        // A keeper whose thread ended writes nothing more: what it held is written here, or the wait says why not.
        if self.write_what_a_gone_keeper_held()? {
            return Ok(());
        }
        self.keeper.as_ref().map_or(Ok(()), |keeper| keeper.settle_all(within))
    }

    fn failed_keep(&self) -> Option<(DraftId, StoreError)> {
        self.keeper.as_ref().and_then(Keeper::take_failed)
    }

    fn unkept(&self) -> Vec<(DraftId, StoreError)> {
        self.keeper.as_ref().map(Keeper::unkept).unwrap_or_default()
    }

    fn keeps(&self) -> u64 {
        self.keeper.as_ref().map_or(0, Keeper::keeps)
    }

    fn wake_on_keep(&self, wake: KeepWake) {
        if let Some(keeper) = &self.keeper {
            keeper.wake_on(wake);
        }
    }

    fn outbox(&self) -> Result<Vec<Outgoing>, StoreError> {
        self.records("SELECT record FROM outbox ORDER BY draft", [])
    }

    fn schedule(&self) -> Result<Vec<Scheduled>, StoreError> {
        self.records("SELECT record FROM schedule ORDER BY due, id", [])
    }

    fn sender(&self, address: &EmailAddress) -> Result<Option<Sender>, StoreError> {
        self.one("SELECT record FROM sender WHERE address = ?1", [sender_key(address)])
    }

    fn senders(&self) -> Result<Vec<Sender>, StoreError> {
        self.records("SELECT record FROM sender ORDER BY address", [])
    }

    fn message_state(&self, id: MessageId) -> Result<MessageState, StoreError> {
        Ok(self.one("SELECT record FROM message_state WHERE message = ?1", [key(id.get())])?.unwrap_or_default())
    }

    fn message_states(&self) -> Result<Vec<(MessageId, MessageState)>, StoreError> {
        let mut statement = self.conn.prepare("SELECT message, record FROM message_state ORDER BY message")?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
        rows.map(|row| {
            let (id, state) = row?;
            let id = u64::try_from(id).map(MessageId::new).map_err(|_| StoreError::Corrupt("a message id"))?;
            Ok((id, parse(&state)?))
        })
        .collect()
    }

    fn calendars(&self, account: AccountId) -> Result<Vec<Calendar>, StoreError> {
        self.records("SELECT record FROM calendar WHERE account = ?1 ORDER BY id", [key(account.get())])
    }

    fn sync_token(&self, calendar: CalendarId) -> Result<Option<String>, StoreError> {
        let token: Option<Option<String>> =
            self.conn.query_row("SELECT sync_token FROM calendar WHERE id = ?1", [key(calendar.get())], |row| row.get(0)).optional()?;
        Ok(token.flatten())
    }

    fn calendar_events(&self, calendar: CalendarId) -> Result<Vec<CalendarEvent>, StoreError> {
        self.records("SELECT record FROM calendar_event WHERE calendar = ?1 ORDER BY id", [key(calendar.get())])
    }

    fn calendar_event(&self, id: CalendarEventId) -> Result<Option<CalendarEvent>, StoreError> {
        self.one("SELECT record FROM calendar_event WHERE id = ?1", [key(id.get())])
    }

    fn alerts(&self, event: CalendarEventId) -> Result<Vec<Alert>, StoreError> {
        self.records("SELECT record FROM alert WHERE event = ?1 ORDER BY at, id", [key(event.get())])
    }

    fn alert(&self, id: AlertId) -> Result<Option<Alert>, StoreError> {
        self.one("SELECT record FROM alert WHERE id = ?1", [key(id.get())])
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
            params![key(folder.get()), cutoff.unix_seconds()],
        )
    }
}
