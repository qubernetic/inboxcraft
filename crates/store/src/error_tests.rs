// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, DraftId, FolderId, MessageId, Uid};

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
        (StoreError::WrongAccount { folder: FolderId::new(2), account: AccountId::new(1) }, "folder f2 does not belong to account a1"),
        (StoreError::DuplicateUid { folder: FolderId::new(2), uid: Uid::new(5).unwrap() }, "folder f2 already holds UID 5"),
        (StoreError::TooLarge, "the message source is too large to keep"),
        (StoreError::NewerSchema { found: 9, known: 2 }, "the store has schema version 9, newer than this build's 2"),
        (StoreError::Corrupt("a record"), "the store is damaged: a record"),
        (StoreError::Io(std::io::ErrorKind::NotFound), "a store file operation failed: entity not found"),
        (StoreError::Database("disk I/O error".to_owned()), "the store database failed: disk I/O error"),
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
