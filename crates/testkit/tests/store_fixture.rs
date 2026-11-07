// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fixture store: a small invented mailbox on disk, the same every time.

use inboxcraft_model::{AccountId, Flag, FolderId, MessageId};
use inboxcraft_store::{Sqlite, Storage, StoreConfig};
use inboxcraft_testkit::store;

#[test]
fn the_fixture_holds_one_account_three_folders_and_four_messages() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    store::fixture(dir.path())?;
    let store = Sqlite::open(dir.path(), StoreConfig::default())?;
    let accounts = store.accounts()?;
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts.first().map(|account| account.address().as_str()), Some("fixture@example.org"));
    let folders = store.folders(AccountId::new(store::ACCOUNT))?;
    assert_eq!(folders.iter().map(|folder| folder.id().get()).collect::<Vec<_>>(), [store::INBOX, store::SENT, store::ARCHIVE]);
    assert_eq!(store.messages(FolderId::new(store::INBOX))?.len(), 2);
    let lunch = store.message(MessageId::new(store::LUNCH))?.ok_or("no lunch message")?;
    assert!(!lunch.message().summary().flags().contains(&Flag::Seen));
    assert!(store.body(MessageId::new(store::LUNCH))?.is_some());
    assert!(store.body(MessageId::new(store::RECEIPT))?.is_none());
    Ok(())
}

#[test]
fn the_fixture_refuses_a_directory_that_already_holds_a_store() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    store::fixture(dir.path())?;
    assert!(store::fixture(dir.path()).is_err());
    Ok(())
}
