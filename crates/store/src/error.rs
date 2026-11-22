// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The store's error type.

use std::fmt;

use inboxcraft_model::{AccountId, AlertId, CalendarEventId, CalendarId, DraftId, FolderId, MessageId, Uid};

/// Why the store refused a change or could not answer.
///
/// A refused [`Batch`](crate::Batch) changes nothing. No variant carries message text.
///
/// A database failure displays as one fixed sentence for its kind, told apart by the database's result code: no
/// table, column, statement, file name or path reaches whoever reads the sentence. The database's own words stay in
/// the value, cut to [`MAX_ERROR_DETAIL`] characters without control characters, for the app's own log and for tests:
/// [`StoreError::detail`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// A change names an account the store does not hold.
    UnknownAccount(AccountId),
    /// A change names a folder the store does not hold.
    UnknownFolder(FolderId),
    /// A change names a message the store does not hold.
    UnknownMessage(MessageId),
    /// A change names a draft the store does not hold.
    UnknownDraft(DraftId),
    /// A change names a draft whose newest text the store could not write yet: the store keeps it and tries again,
    /// and refuses the change until that text is on the disk.
    DraftNotKept(DraftId),
    /// A change names a calendar the store does not hold.
    UnknownCalendar(CalendarId),
    /// A change names a calendar event the store does not hold.
    UnknownCalendarEvent(CalendarEventId),
    /// A change names an alert the store does not hold.
    UnknownAlert(AlertId),
    /// A folder or message belongs to another account than the change says.
    WrongAccount {
        /// The folder.
        folder: FolderId,
        /// The account the change gives.
        account: AccountId,
    },
    /// Another message of the folder already has this UID.
    DuplicateUid {
        /// The folder.
        folder: FolderId,
        /// The UID.
        uid: Uid,
    },
    /// A record's own id is past `i64::MAX`, the largest key the database takes; nothing is kept.
    IdOutOfRange(u64),
    /// A message source over [`MAX_SOURCE_BYTES`](crate::MAX_SOURCE_BYTES).
    TooLarge,
    /// The database was written by a newer InboxCraft; it is left untouched.
    NewerSchema {
        /// The schema version found.
        found: u32,
        /// The newest version this build knows.
        known: u32,
    },
    /// What is on disk does not read back: a record, a source file name or a source file.
    Corrupt(&'static str),
    /// A file operation failed.
    Io(std::io::ErrorKind),
    /// The database could not be read or written, for a reason without a kind of its own below (a missing table, a
    /// read-only or vanished file, an I/O failure); the database's own words.
    Database(String),
    /// Another connection holds the database (busy or locked); trying again later may work. The database's own words.
    DatabaseBusy(String),
    /// The disk (or the database's page limit) is full. The database's own words.
    DatabaseFull(String),
    /// The database file is damaged or is no database. The database's own words.
    DatabaseDamaged(String),
    /// The database refused a change that breaks one of its constraints (a trigger's refusal is one). The database's own
    /// words.
    DatabaseRefused(String),
    /// A migration of the schema to this build's version failed for a reason without a kind of its own; the database
    /// is left as it was. The database's own words.
    Migration(String),
}

/// The most characters of the database's own words a [`StoreError`] keeps.
pub const MAX_ERROR_DETAIL: usize = 256;

impl StoreError {
    /// The database's own words behind a database failure, for the app's own log and for tests; `None` for every
    /// other error. Never shown to a caller outside the app: they can name tables, columns, a statement, and a
    /// value or a refusal's words from a damaged or crafted file.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Database(detail)
            | Self::DatabaseBusy(detail)
            | Self::DatabaseFull(detail)
            | Self::DatabaseDamaged(detail)
            | Self::DatabaseRefused(detail)
            | Self::Migration(detail) => Some(detail),
            _ => None,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAccount(id) => write!(f, "no account {id} in the store"),
            Self::UnknownFolder(id) => write!(f, "no folder {id} in the store"),
            Self::UnknownMessage(id) => write!(f, "no message {id} in the store"),
            Self::UnknownDraft(id) => write!(f, "no draft {id} in the store"),
            Self::DraftNotKept(id) => {
                write!(f, "draft {id}'s latest change is not on the disk yet: the store is trying again; try later")
            }
            Self::UnknownCalendar(id) => write!(f, "no calendar {id} in the store"),
            Self::UnknownCalendarEvent(id) => write!(f, "no calendar event {id} in the store"),
            Self::UnknownAlert(id) => write!(f, "no alert {id} in the store"),
            Self::WrongAccount { folder, account } => write!(f, "folder {folder} does not belong to account {account}"),
            Self::DuplicateUid { folder, uid } => write!(f, "folder {folder} already holds UID {uid}"),
            Self::IdOutOfRange(id) => write!(f, "id {id} is past the largest the store can keep"),
            Self::TooLarge => write!(f, "the message source is too large to keep"),
            Self::NewerSchema { found, known } => {
                write!(f, "the store has schema version {found}, newer than this build's {known}")
            }
            Self::Corrupt(what) => write!(f, "the store is damaged: {what}"),
            Self::Io(kind) => write!(f, "a store file operation failed: {kind}"),
            Self::Database(_) => f.write_str("the store could not be read or written"),
            Self::DatabaseBusy(_) => f.write_str("the store is busy, try again"),
            Self::DatabaseFull(_) => f.write_str("the store is full: there is no room left on the disk"),
            Self::DatabaseDamaged(_) => f.write_str("the store is damaged"),
            Self::DatabaseRefused(_) => f.write_str("the store refused the change"),
            Self::Migration(_) => f.write_str("the store could not be brought up to this version"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.kind())
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
