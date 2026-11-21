// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use inboxcraft_model::{
    Account, AccountId, Draft, DraftId, EmailAddress, Envelope, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId, MessageState,
    Security, Server, Summary, Uid,
};
use inboxcraft_time::Timestamp;

use super::Memory;
use crate::{Batch, Change, Storage, StoreConfig, StoreError};

fn account(id: u64) -> Account {
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    Account::new(AccountId::new(id), "Plot", EmailAddress::new("owner@example.org").unwrap(), "owner", imap).unwrap()
}

fn draft(id: u64, account: u64, text: &str) -> Draft {
    Draft { text: text.to_owned(), ..Draft::new(DraftId::new(id), AccountId::new(account), Timestamp::from_unix(0, 0).unwrap()) }
}

/// Keeping a draft while the person types copies the drafts, never the messages or the other tables: the composer's
/// keep was a copy of the whole mailbox before (#243).
#[test]
fn a_kept_draft_copies_the_drafts_and_shares_every_other_table() {
    let mut store = Memory::new(StoreConfig::default());
    store.apply(&Batch::new().with(Change::PutAccount(account(1))).with(Change::PutDraft(draft(1, 1, "Plot 14")))).unwrap();
    let (messages, accounts, drafts) = (Arc::clone(&store.state.messages.0), Arc::clone(&store.state.accounts.0), Arc::clone(&store.state.drafts.0));
    store.apply(&Batch::new().with(Change::PutDraft(draft(1, 1, "Plot 14, at ten")))).unwrap();
    assert!(Arc::ptr_eq(&messages, &store.state.messages.0), "the messages were copied");
    assert!(Arc::ptr_eq(&accounts, &store.state.accounts.0), "the accounts were copied");
    assert!(!Arc::ptr_eq(&drafts, &store.state.drafts.0), "the drafts changed in place of a copy");
    assert_eq!(store.draft(DraftId::new(1)).unwrap().map(|draft| draft.text), Some("Plot 14, at ten".to_owned()));
    assert_eq!(drafts.get(&DraftId::new(1)).map(|draft| draft.text.as_str()), Some("Plot 14"), "the old table is left as it was");
}

/// Pinning, parking or snoozing a message writes its state, never the messages: the state change checked the message
/// with a write borrow before and so copied the whole messages table (#243).
#[test]
fn a_message_state_copies_the_states_and_shares_the_messages() {
    let mut store = Memory::new(StoreConfig::default());
    let folder = Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", Some('/')).unwrap(), vec![], true));
    let message = Message::new(MessageId::new(1), AccountId::new(1), FolderId::new(1), Summary::new(Uid::new(1).unwrap(), Envelope::new("Plot 14")));
    store.apply(&Batch::new().with(Change::PutAccount(account(1))).with(Change::PutFolder(folder)).with(Change::PutMessage(message))).unwrap();
    let (messages, states) = (Arc::clone(&store.state.messages.0), Arc::clone(&store.state.states.0));
    let pinned = MessageState { pinned: true, ..MessageState::default() };
    store.apply(&Batch::new().with(Change::SetMessageState(MessageId::new(1), pinned.clone()))).unwrap();
    assert!(Arc::ptr_eq(&messages, &store.state.messages.0), "the messages were copied");
    assert!(!Arc::ptr_eq(&states, &store.state.states.0), "the states changed in place of a copy");
    assert_eq!(store.message_state(MessageId::new(1)).unwrap(), pinned);
    let refused = store.apply(&Batch::new().with(Change::SetMessageState(MessageId::new(9), pinned)));
    assert_eq!(refused, Err(StoreError::UnknownMessage(MessageId::new(9))));
}

/// A batch refused halfway leaves the tables it wrote to as they were, also while they are shared.
#[test]
fn a_refused_batch_leaves_a_shared_table_as_it_was() {
    let mut store = Memory::new(StoreConfig::default());
    store.apply(&Batch::new().with(Change::PutAccount(account(1))).with(Change::PutDraft(draft(1, 1, "Plot 14")))).unwrap();
    let refused = store.apply(&Batch::new().with(Change::PutDraft(draft(1, 1, "Changed"))).with(Change::PutDraft(draft(2, 9, "Nobody's"))));
    assert_eq!(refused, Err(StoreError::UnknownAccount(AccountId::new(9))));
    assert_eq!(store.draft(DraftId::new(1)).unwrap().map(|draft| draft.text), Some("Plot 14".to_owned()));
    assert_eq!(store.draft(DraftId::new(2)).unwrap(), None);
}

/// Marking one message read or flagging it copies that message and the map of handles, never the other messages: the
/// copy of every message was most of the cost of opening one in a mailbox of 10 000 (#406). The copy it leaves
/// behind is as it was.
#[test]
fn an_act_on_one_message_copies_that_message_only() {
    let mut store = Memory::new(StoreConfig::default());
    let folder = Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", Some('/')).unwrap(), vec![], true));
    let message = |id: u64| {
        let uid = Uid::new(u32::try_from(id).unwrap()).unwrap();
        Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(1), Summary::new(uid, Envelope::new("Plot 14")))
    };
    let batch = Batch::new().with(Change::PutAccount(account(1))).with(Change::PutFolder(folder));
    store.apply(&batch.with(Change::PutMessage(message(1))).with(Change::PutMessage(message(2)))).unwrap();
    let before = store.clone();
    store.apply(&Batch::new().with(Change::MarkOpened(MessageId::new(1)))).unwrap();
    let held = |store: &Memory, id: u64| Arc::clone(store.state.messages.get(&MessageId::new(id)).unwrap());
    assert!(Arc::ptr_eq(&held(&before, 2), &held(&store, 2)), "the other message was copied");
    assert!(!Arc::ptr_eq(&held(&before, 1), &held(&store, 1)), "the message changed in place of a copy");
    assert!(store.message(MessageId::new(1)).unwrap().unwrap().opened());
    assert!(!before.message(MessageId::new(1)).unwrap().unwrap().opened(), "the copy left behind changed");
}
