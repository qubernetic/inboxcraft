// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests: an invented mailbox on `example.org`.

use inboxcraft_model::{
    Account, AccountId, Address, EmailAddress, Envelope, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId, Security, Server,
    SpecialUse, Summary, Uid,
};
use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::{Registry, Session};

/// 2026-11-05T09:00:00Z.
pub const NOW: i64 = 1_793_869_200;

pub fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(seconds, 0).unwrap()
}

pub fn clock() -> FakeClock {
    FakeClock::new(at(NOW))
}

pub fn account(id: u64, address: &str) -> Account {
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    Account::new(AccountId::new(id), &format!("Account {id}"), EmailAddress::new(address).unwrap(), address, imap).unwrap()
}

pub fn folder(id: u64, account: u64, name: &str, special_use: Vec<SpecialUse>) -> Folder {
    Folder::new(FolderId::new(id), AccountId::new(account), FolderListing::new(FolderPath::new(name, Some('/')).unwrap(), special_use, true))
}

pub fn address(name: &str, email: &str) -> Address {
    Address::new(Some(name), EmailAddress::new(email).unwrap())
}

/// A message `id` in `folder` of `account` with UID `uid`, received at `received` seconds.
pub fn message(id: u64, account: u64, folder: u64, uid: u32, subject: &str, received: Option<i64>) -> Message {
    let envelope =
        Envelope::new(subject).with_from(vec![address("Anna Example", "anna@example.org")]).with_to(vec![address("Bob", "bob@example.net")]);
    let summary =
        Summary::new(Uid::new(uid).unwrap(), envelope).with_received(received.map(at)).with_size(1200).with_preview(Some("A short preview"));
    Message::new(MessageId::new(id), AccountId::new(account), FolderId::new(folder), summary)
}

/// Two accounts: 1 (`anna@example.org`: INBOX 1, Sent 2, Archive 3) and 2 (`bob@example.net`: INBOX 4, Sent 5), and
/// a few messages in both inboxes and the first Sent.
pub fn mailbox() -> Memory {
    let mut store = Memory::new(StoreConfig::default());
    let batch = Batch::new()
        .with(Change::PutAccount(account(1, "anna@example.org")))
        .with(Change::PutAccount(account(2, "bob@example.net")))
        .with(Change::PutFolder(folder(1, 1, "INBOX", vec![])))
        .with(Change::PutFolder(folder(2, 1, "Sent", vec![SpecialUse::Sent])))
        .with(Change::PutFolder(folder(3, 1, "Archive", vec![])))
        .with(Change::PutFolder(folder(4, 2, "INBOX", vec![])))
        .with(Change::PutFolder(folder(5, 2, "Elküldött", vec![])))
        .with(Change::PutMessage(message(1, 1, 1, 10, "Budget for 2026", Some(NOW - 3600))))
        .with(Change::PutMessage(message(2, 1, 1, 11, "Lunch on Friday", Some(NOW - 60))))
        .with(Change::PutMessage(message(3, 2, 4, 7, "Garden party", Some(NOW - 1800))))
        .with(Change::PutMessage(message(4, 2, 4, 8, "No date at all", None)))
        .with(Change::PutMessage(message(5, 1, 2, 3, "Re: Budget for 2026", Some(NOW - 7200))));
    store.apply(&batch).unwrap();
    store
}

pub fn session_with(store: Memory, registry: Registry) -> Session {
    Session::with_registry(Box::new(store), Box::new(clock()), registry).unwrap()
}

pub fn session() -> Session {
    session_with(mailbox(), Registry::builtin().unwrap())
}
