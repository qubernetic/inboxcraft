// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The store's error type.

use std::fmt;

use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, DraftId, FolderId, MessageId, Uid};

/// Why the store refused a change or could not answer.
///
/// A refused [`Batch`](crate::Batch) changes nothing. No variant carries message text.
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
    /// A change names a calendar the store does not hold.
    UnknownCalendar(CalendarId),
    /// A change names a calendar event the store does not hold.
    UnknownCalendarEvent(CalendarEventId),
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
    /// The database engine failed; its own message.
    Database(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAccount(id) => write!(f, "no account {id} in the store"),
            Self::UnknownFolder(id) => write!(f, "no folder {id} in the store"),
            Self::UnknownMessage(id) => write!(f, "no message {id} in the store"),
            Self::UnknownDraft(id) => write!(f, "no draft {id} in the store"),
            Self::UnknownCalendar(id) => write!(f, "no calendar {id} in the store"),
            Self::UnknownCalendarEvent(id) => write!(f, "no calendar event {id} in the store"),
            Self::WrongAccount { folder, account } => write!(f, "folder {folder} does not belong to account {account}"),
            Self::DuplicateUid { folder, uid } => write!(f, "folder {folder} already holds UID {uid}"),
            Self::TooLarge => write!(f, "the message source is too large to keep"),
            Self::NewerSchema { found, known } => {
                write!(f, "the store has schema version {found}, newer than this build's {known}")
            }
            Self::Corrupt(what) => write!(f, "the store is damaged: {what}"),
            Self::Io(kind) => write!(f, "a store file operation failed: {kind}"),
            Self::Database(message) => write!(f, "the store database failed: {message}"),
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
