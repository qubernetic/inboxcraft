// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{AccountId, AlertId, CalendarEventId, CalendarId, DraftId, FolderId, MessageId, Uid};

use super::StoreError;

#[test]
fn every_error_says_what_failed() {
    let cases = [
        (StoreError::UnknownAccount(AccountId::new(1)), "no account a1 in the store"),
        (StoreError::UnknownFolder(FolderId::new(2)), "no folder f2 in the store"),
        (StoreError::UnknownMessage(MessageId::new(3)), "no message m3 in the store"),
        (StoreError::UnknownDraft(DraftId::new(4)), "no draft d4 in the store"),
        (StoreError::UnknownCalendar(CalendarId::new(5)), "no calendar c5 in the store"),
        (StoreError::UnknownCalendarEvent(CalendarEventId::new(6)), "no calendar event e6 in the store"),
        (StoreError::UnknownAlert(AlertId::new(7)), "no alert l7 in the store"),
        (StoreError::WrongAccount { folder: FolderId::new(2), account: AccountId::new(1) }, "folder f2 does not belong to account a1"),
        (StoreError::DuplicateUid { folder: FolderId::new(2), uid: Uid::new(5).unwrap() }, "folder f2 already holds UID 5"),
        (StoreError::TooLarge, "the message source is too large to keep"),
        (StoreError::NewerSchema { found: 9, known: 2 }, "the store has schema version 9, newer than this build's 2"),
        (StoreError::Corrupt("a record"), "the store is damaged: a record"),
        (StoreError::Io(std::io::ErrorKind::NotFound), "a store file operation failed: entity not found"),
        (StoreError::Database("no such table: body".to_owned()), "the store could not be read or written"),
        (StoreError::DatabaseBusy("database is locked".to_owned()), "the store is busy, try again"),
        (StoreError::DatabaseFull("database or disk is full".to_owned()), "the store is full: there is no room left on the disk"),
        (StoreError::DatabaseDamaged("file is not a database".to_owned()), "the store is damaged"),
        (StoreError::DatabaseRefused("UNIQUE constraint failed: message.folder, message.uid".to_owned()), "the store refused the change"),
        (StoreError::Migration("table meta already exists".to_owned()), "the store could not be brought up to this version"),
    ];
    for (error, text) in cases {
        assert_eq!(error.to_string(), text);
    }
}

#[test]
fn an_io_error_keeps_its_kind() {
    let error: StoreError = std::io::Error::from(std::io::ErrorKind::PermissionDenied).into();
    assert_eq!(error, StoreError::Io(std::io::ErrorKind::PermissionDenied));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn only_a_database_failure_has_a_detail() {
    assert_eq!(StoreError::DatabaseRefused("words".to_owned()).detail(), Some("words"));
    assert_eq!(StoreError::Migration("step".to_owned()).detail(), Some("step"));
    assert_eq!(StoreError::Corrupt("a record").detail(), None);
    assert_eq!(StoreError::TooLarge.detail(), None);
}
