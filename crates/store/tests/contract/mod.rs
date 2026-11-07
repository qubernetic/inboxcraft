// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The behaviour every backend shares. `contract_tests!(make)` writes one test per function here, each with a fresh
//! store from `make()`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use inboxcraft_model::{
    Account, AccountId, Address, EmailAddress, Envelope, Flag, Flags, Folder, FolderId, FolderListing, FolderPath, FolderState, Host, Message,
    MessageId, ModSeq, Security, Server, Summary, Uid, UidValidity,
};
use inboxcraft_store::{Batch, Body, Change, Search, Storage, StoreError};
use inboxcraft_time::Timestamp;

mod local;
#[allow(unused_imports)]
pub use local::*;

pub const DAY: i64 = 86_400;

pub fn account(id: u64) -> Account {
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    Account::new(AccountId::new(id), &format!("Account {id}"), EmailAddress::new(&format!("owner{id}@example.org")).unwrap(), "owner", imap).unwrap()
}

pub fn folder(id: u64, account: u64, name: &str) -> Folder {
    Folder::new(FolderId::new(id), AccountId::new(account), FolderListing::new(FolderPath::new(name, Some('/')).unwrap(), vec![], true))
}

pub fn at(days: i64) -> Timestamp {
    Timestamp::from_unix(days * DAY, 0).unwrap()
}

fn address(name: Option<&str>, email: &str) -> Address {
    Address::new(name, EmailAddress::new(email).unwrap())
}

/// Message `id` in folder `folder` of account `account` with UID `uid`, from Anna, to Bela, dated `days` after 1970.
pub fn message(id: u64, account: u64, folder: u64, uid: u32, subject: &str, days: Option<i64>) -> Message {
    let envelope = Envelope::new(subject)
        .with_from(vec![address(Some("Anna Kovacs"), "anna@example.org")])
        .with_to(vec![address(None, "bela@example.org")])
        .with_cc(vec![address(Some("Cili"), "cili@example.net")])
        .with_date(days.map(at));
    let summary = Summary::new(Uid::new(uid).unwrap(), envelope).with_received(days.map(at));
    Message::new(MessageId::new(id), AccountId::new(account), FolderId::new(folder), summary)
}

pub fn body(text: &str) -> Body {
    Body::new(format!("Subject: x\r\n\r\n{text}\r\n").into_bytes(), text).unwrap()
}

pub fn m(id: u64) -> MessageId {
    MessageId::new(id)
}

/// Account 1 with INBOX (folder 1) and Archive (folder 2); account 2 with INBOX (folder 3).
pub fn two_accounts<S: Storage>(store: &mut S) {
    let batch = Batch::new()
        .with(Change::PutAccount(account(1)))
        .with(Change::PutAccount(account(2)))
        .with(Change::PutFolder(folder(1, 1, "INBOX")))
        .with(Change::PutFolder(folder(2, 1, "Archive")))
        .with(Change::PutFolder(folder(3, 2, "INBOX")));
    store.apply(&batch).unwrap();
}

fn put<S: Storage>(store: &mut S, changes: Vec<Change>) -> Result<(), StoreError> {
    let mut batch = Batch::new();
    for change in changes {
        batch.push(change);
    }
    store.apply(&batch)
}

fn ids<S: Storage>(store: &S, folder: u64) -> Vec<u64> {
    store.messages(FolderId::new(folder)).unwrap().iter().map(|entry| entry.message().id().get()).collect()
}

pub fn a_new_store_is_empty<S: Storage>(store: S) {
    assert!(store.accounts().unwrap().is_empty());
    assert!(store.folders(AccountId::new(1)).unwrap().is_empty());
    assert!(store.messages(FolderId::new(1)).unwrap().is_empty());
    assert_eq!(store.message(m(1)).unwrap(), None);
    assert_eq!(store.body(m(1)).unwrap(), None);
    assert_eq!(store.folder_state(FolderId::new(1)).unwrap(), None);
    assert!(store.search(&Search::new()).unwrap().is_empty());
}

pub fn accounts_and_folders_read_back_in_id_order<S: Storage>(mut store: S) {
    put(&mut store, vec![Change::PutAccount(account(2)), Change::PutAccount(account(1))]).unwrap();
    put(
        &mut store,
        vec![Change::PutFolder(folder(5, 1, "Sent")), Change::PutFolder(folder(4, 1, "INBOX")), Change::PutFolder(folder(6, 2, "INBOX"))],
    )
    .unwrap();
    assert_eq!(store.accounts().unwrap(), [account(1), account(2)]);
    assert_eq!(store.folders(AccountId::new(1)).unwrap(), [folder(4, 1, "INBOX"), folder(5, 1, "Sent")]);
    assert_eq!(store.folders(AccountId::new(2)).unwrap(), [folder(6, 2, "INBOX")]);
}

pub fn a_put_replaces_the_record_with_the_same_id<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let renamed =
        Account::new(AccountId::new(1), "Work", EmailAddress::new("owner1@example.org").unwrap(), "owner", account(1).imap().clone()).unwrap();
    put(&mut store, vec![Change::PutAccount(renamed.clone()), Change::PutFolder(folder(2, 1, "Old"))]).unwrap();
    assert_eq!(store.accounts().unwrap(), [renamed, account(2)]);
    assert_eq!(store.folders(AccountId::new(1)).unwrap(), [folder(1, 1, "INBOX"), folder(2, 1, "Old")]);
}

pub fn a_folder_needs_its_account<S: Storage>(mut store: S) {
    assert_eq!(put(&mut store, vec![Change::PutFolder(folder(1, 9, "INBOX"))]), Err(StoreError::UnknownAccount(AccountId::new(9))));
    two_accounts(&mut store);
    assert_eq!(
        put(&mut store, vec![Change::PutFolder(folder(1, 2, "INBOX"))]),
        Err(StoreError::WrongAccount { folder: FolderId::new(1), account: AccountId::new(2) })
    );
}

pub fn a_refused_batch_changes_nothing<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let refused = put(
        &mut store,
        vec![
            Change::PutAccount(account(3)),
            Change::PutMessage(message(1, 1, 1, 1, "first", Some(1))),
            Change::RemoveFolder(FolderId::new(2)),
            Change::PutMessage(message(2, 1, 9, 1, "nowhere", Some(1))),
        ],
    );
    assert_eq!(refused, Err(StoreError::UnknownFolder(FolderId::new(9))));
    assert_eq!(store.accounts().unwrap(), [account(1), account(2)]);
    assert_eq!(store.folders(AccountId::new(1)).unwrap().len(), 2);
    assert_eq!(store.message(m(1)).unwrap(), None);
}

pub fn an_empty_batch_is_fine<S: Storage>(mut store: S) {
    store.apply(&Batch::new()).unwrap();
    assert!(store.accounts().unwrap().is_empty());
}

pub fn folder_state_is_kept_per_folder<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let state = FolderState::new(UidValidity::new(7).unwrap()).with_uid_next(Uid::new(42)).with_highest_modseq(ModSeq::new(9_000_000_000));
    put(&mut store, vec![Change::SetFolderState(FolderId::new(1), state)]).unwrap();
    assert_eq!(store.folder_state(FolderId::new(1)).unwrap(), Some(state));
    assert_eq!(store.folder_state(FolderId::new(2)).unwrap(), None);
    put(&mut store, vec![Change::PutFolder(folder(1, 1, "Inbox"))]).unwrap();
    assert_eq!(store.folder_state(FolderId::new(1)).unwrap(), Some(state), "a renamed folder keeps its state");
    assert_eq!(put(&mut store, vec![Change::SetFolderState(FolderId::new(9), state)]), Err(StoreError::UnknownFolder(FolderId::new(9))));
}

pub fn messages_read_back_in_uid_order<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(
        &mut store,
        vec![
            Change::PutMessage(message(1, 1, 1, 30, "c", Some(3))),
            Change::PutMessage(message(2, 1, 1, 10, "a", Some(1))),
            Change::PutMessage(message(3, 1, 2, 20, "b", Some(2))),
        ],
    )
    .unwrap();
    assert_eq!(ids(&store, 1), [2, 1]);
    assert_eq!(ids(&store, 2), [3]);
    let entry = store.message(m(2)).unwrap().unwrap();
    assert_eq!(entry.message(), &message(2, 1, 1, 10, "a", Some(1)));
    assert!(!entry.opened());
    assert!(!entry.has_body());
}

pub fn a_message_needs_its_folder_and_account<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    assert_eq!(put(&mut store, vec![Change::PutMessage(message(1, 1, 9, 1, "x", None))]), Err(StoreError::UnknownFolder(FolderId::new(9))));
    assert_eq!(
        put(&mut store, vec![Change::PutMessage(message(1, 2, 1, 1, "x", None))]),
        Err(StoreError::WrongAccount { folder: FolderId::new(1), account: AccountId::new(2) })
    );
}

pub fn a_uid_belongs_to_one_message_per_folder<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(&mut store, vec![Change::PutMessage(message(1, 1, 1, 5, "x", None))]).unwrap();
    assert_eq!(
        put(&mut store, vec![Change::PutMessage(message(2, 1, 1, 5, "y", None))]),
        Err(StoreError::DuplicateUid { folder: FolderId::new(1), uid: Uid::new(5).unwrap() })
    );
    put(&mut store, vec![Change::PutMessage(message(2, 1, 2, 5, "same uid elsewhere", None))]).unwrap();
    put(&mut store, vec![Change::RemoveMessage(m(1)), Change::PutMessage(message(3, 1, 1, 5, "reused", None))]).unwrap();
    assert_eq!(ids(&store, 1), [3]);
}

pub fn a_replaced_message_keeps_its_body_and_opened_mark<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(&mut store, vec![Change::PutMessage(message(1, 1, 1, 5, "x", None)), Change::PutBody(m(1), body("hello")), Change::MarkOpened(m(1))])
        .unwrap();
    put(&mut store, vec![Change::PutMessage(message(1, 1, 2, 8, "moved", None))]).unwrap();
    assert!(ids(&store, 1).is_empty());
    assert_eq!(ids(&store, 2), [1]);
    let entry = store.message(m(1)).unwrap().unwrap();
    assert!(entry.opened() && entry.has_body());
    assert_eq!(entry.message().summary().envelope().subject(), "moved");
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
    put(&mut store, vec![Change::PutMessage(message(2, 1, 1, 5, "uid 5 is free again", None))]).unwrap();
}

pub fn flags_are_set_on_the_held_message<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let mut thread_message = message(1, 1, 1, 5, "x", None);
    thread_message = thread_message.with_thread(Some(inboxcraft_model::ThreadId::new(4)));
    put(&mut store, vec![Change::PutMessage(thread_message)]).unwrap();
    let mut flags = Flags::default();
    flags.insert(Flag::Seen);
    flags.insert(Flag::parse("$Forwarded").unwrap());
    put(&mut store, vec![Change::SetFlags(m(1), flags.clone())]).unwrap();
    let held = store.message(m(1)).unwrap().unwrap();
    assert_eq!(held.message().summary().flags(), &flags);
    assert_eq!(held.message().thread(), Some(inboxcraft_model::ThreadId::new(4)));
    assert_eq!(put(&mut store, vec![Change::SetFlags(m(9), flags)]), Err(StoreError::UnknownMessage(m(9))));
}

pub fn bodies_are_kept_replaced_and_removed<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(&mut store, vec![Change::PutMessage(message(1, 1, 1, 5, "x", None)), Change::PutBody(m(1), body("first"))]).unwrap();
    assert_eq!(store.body(m(1)).unwrap(), Some(body("first")));
    put(&mut store, vec![Change::PutBody(m(1), body("second")), Change::PutBody(m(1), body("third"))]).unwrap();
    assert_eq!(store.body(m(1)).unwrap(), Some(body("third")));
    put(&mut store, vec![Change::RemoveBody(m(1)), Change::RemoveBody(m(9))]).unwrap();
    assert_eq!(store.body(m(1)).unwrap(), None);
    assert!(!store.message(m(1)).unwrap().unwrap().has_body());
    assert_eq!(put(&mut store, vec![Change::PutBody(m(9), body("x"))]), Err(StoreError::UnknownMessage(m(9))));
    assert_eq!(put(&mut store, vec![Change::MarkOpened(m(9))]), Err(StoreError::UnknownMessage(m(9))));
}

pub fn an_empty_and_a_binary_body_read_back<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let binary = Body::new((0..=255).collect(), "").unwrap();
    put(&mut store, vec![Change::PutMessage(message(1, 1, 1, 5, "x", None)), Change::PutBody(m(1), binary.clone())]).unwrap();
    assert_eq!(store.body(m(1)).unwrap(), Some(binary));
    let empty = Body::new(Vec::new(), "").unwrap();
    put(&mut store, vec![Change::PutBody(m(1), empty.clone())]).unwrap();
    assert_eq!(store.body(m(1)).unwrap(), Some(empty));
}

pub fn removals_take_what_hangs_below<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(
        &mut store,
        vec![
            Change::PutMessage(message(1, 1, 1, 1, "a", None)),
            Change::PutMessage(message(2, 1, 2, 1, "b", None)),
            Change::PutMessage(message(3, 2, 3, 1, "c", None)),
            Change::PutBody(m(1), body("one")),
            Change::PutBody(m(2), body("two")),
            Change::PutBody(m(3), body("three")),
        ],
    )
    .unwrap();
    put(&mut store, vec![Change::RemoveFolder(FolderId::new(2)), Change::RemoveFolder(FolderId::new(9))]).unwrap();
    assert_eq!(store.message(m(2)).unwrap(), None);
    assert_eq!(store.body(m(2)).unwrap(), None);
    assert_eq!(store.folders(AccountId::new(1)).unwrap(), [folder(1, 1, "INBOX")]);
    put(&mut store, vec![Change::RemoveAccount(AccountId::new(1)), Change::RemoveAccount(AccountId::new(9)), Change::RemoveMessage(m(9))]).unwrap();
    assert_eq!(store.accounts().unwrap(), [account(2)]);
    assert!(store.folders(AccountId::new(1)).unwrap().is_empty());
    assert_eq!(store.message(m(1)).unwrap(), None);
    assert_eq!(store.body(m(1)).unwrap(), None);
    assert_eq!(store.body(m(3)).unwrap(), Some(body("three")));
    assert_eq!(store.search(&Search::new().with_text("one")).unwrap(), []);
    assert_eq!(store.search(&Search::new()).unwrap(), [m(3)]);
}

pub fn a_removed_folder_frees_its_ids<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(&mut store, vec![Change::PutMessage(message(1, 1, 1, 1, "a", None))]).unwrap();
    put(&mut store, vec![Change::RemoveAccount(AccountId::new(1)), Change::PutAccount(account(1)), Change::PutFolder(folder(1, 1, "INBOX"))])
        .unwrap();
    assert!(store.messages(FolderId::new(1)).unwrap().is_empty());
    assert_eq!(store.folder_state(FolderId::new(1)).unwrap(), None);
    put(&mut store, vec![Change::PutMessage(message(1, 1, 1, 1, "again", None))]).unwrap();
    assert!(!store.message(m(1)).unwrap().unwrap().has_body());
}

/// Three messages in account 1 and one in account 2, for the search tests.
fn searchable<S: Storage>(store: &mut S) {
    two_accounts(store);
    let mut late = message(4, 2, 3, 1, "Lunch on Friday", Some(20));
    let envelope = late.summary().envelope().clone().with_from(vec![address(None, "dora@example.net")]).with_cc(vec![]);
    late = Message::new(m(4), AccountId::new(2), FolderId::new(3), Summary::new(Uid::new(1).unwrap(), envelope).with_received(Some(at(20))));
    put(
        store,
        vec![
            Change::PutMessage(message(1, 1, 1, 1, "Budget 2026", Some(10))),
            Change::PutMessage(message(2, 1, 1, 2, "Re: Budget 2026", Some(12))),
            Change::PutMessage(message(3, 1, 2, 1, "Holiday photos", None)),
            Change::PutMessage(late),
            Change::PutBody(m(1), body("Please find the budget draft attached.")),
            Change::PutBody(m(2), body("The DRAFT budget looks fine, thanks!")),
            Change::PutBody(m(3), body("Árvíztűrő tükörfúrógép: photos from the lake")),
        ],
    )
    .unwrap();
}

fn found<S: Storage>(store: &S, search: Search) -> Vec<u64> {
    store.search(&search).unwrap().iter().map(|id| id.get()).collect()
}

pub fn search_gives_everything_newest_first_undated_last<S: Storage>(mut store: S) {
    searchable(&mut store);
    assert_eq!(found(&store, Search::new()), [4, 2, 1, 3]);
    assert_eq!(found(&store, Search::new().with_limit(2)), [4, 2]);
    assert_eq!(found(&store, Search::new().with_limit(0)), [4]);
}

pub fn search_by_account_and_folder<S: Storage>(mut store: S) {
    searchable(&mut store);
    assert_eq!(found(&store, Search::new().with_account(AccountId::new(1))), [2, 1, 3]);
    assert_eq!(found(&store, Search::new().with_folder(FolderId::new(2))), [3]);
    assert_eq!(found(&store, Search::new().with_account(AccountId::new(2)).with_folder(FolderId::new(1))), Vec::<u64>::new());
}

pub fn search_by_from_to_and_subject_without_case<S: Storage>(mut store: S) {
    searchable(&mut store);
    assert_eq!(found(&store, Search::new().with_from("KOVACS")), [2, 1, 3]);
    assert_eq!(found(&store, Search::new().with_from("dora@")), [4]);
    assert_eq!(found(&store, Search::new().with_to("cili")), [2, 1, 3], "Cc counts as to");
    assert_eq!(found(&store, Search::new().with_to("bela@example.org")), [4, 2, 1, 3]);
    assert_eq!(found(&store, Search::new().with_subject("budget")), [2, 1]);
    assert_eq!(found(&store, Search::new().with_subject("re: b")), [2]);
    assert_eq!(found(&store, Search::new().with_subject("budget").with_from("dora")), Vec::<u64>::new());
    assert_eq!(found(&store, Search::new().with_subject("")), [4, 2, 1, 3]);
    assert_eq!(found(&store, Search::new().with_subject("%")), Vec::<u64>::new(), "no wildcards");
}

pub fn search_by_date<S: Storage>(mut store: S) {
    searchable(&mut store);
    assert_eq!(found(&store, Search::new().with_since(at(12))), [4, 2]);
    assert_eq!(found(&store, Search::new().with_before(at(12))), [1]);
    assert_eq!(found(&store, Search::new().with_since(at(10)).with_before(at(20))), [2, 1]);
}

pub fn search_by_body_words<S: Storage>(mut store: S) {
    searchable(&mut store);
    assert_eq!(found(&store, Search::new().with_text("budget")), [2, 1]);
    assert_eq!(found(&store, Search::new().with_text("draft budget")), [2], "words in this order");
    assert_eq!(found(&store, Search::new().with_text("budget draft")), [1]);
    assert_eq!(found(&store, Search::new().with_text("tükörfúrógép")), [3]);
    assert_eq!(found(&store, Search::new().with_text("TÜKÖRFÚRÓGÉP photos")), [3]);
    assert_eq!(found(&store, Search::new().with_text("budg")), Vec::<u64>::new(), "whole words only");
    assert_eq!(found(&store, Search::new().with_text("lunch")), Vec::<u64>::new(), "no body kept");
    assert_eq!(found(&store, Search::new().with_text("\"budget\" OR NEAR(x) *")), Vec::<u64>::new());
    assert_eq!(found(&store, Search::new().with_text("budget OR")), Vec::<u64>::new());
    assert_eq!(found(&store, Search::new().with_text("...")), [4, 2, 1, 3], "no words, no criterion");
    assert_eq!(found(&store, Search::new().with_text("budget").with_since(at(11))), [2]);
}

pub fn search_reads_operators_as_words<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let words = "cats or dogs near the barn, not here and there";
    put(&mut store, vec![Change::PutMessage(message(1, 1, 1, 1, "pets", Some(1))), Change::PutBody(m(1), body(words))]).unwrap();
    for text in ["cats OR dogs", "NEAR the", "NOT here AND", "\"or dogs\"", "barn NOT", "AND there"] {
        assert_eq!(found(&store, Search::new().with_text(text)), [1], "{text}");
    }
    for text in ["cats AND dogs", "dogs OR cats", "NEAR(cats dogs)", "cats NOT dogs"] {
        assert_eq!(found(&store, Search::new().with_text(text)), Vec::<u64>::new(), "{text}");
    }
}

pub fn retention_drops_old_unopened_bodies<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(
        &mut store,
        vec![
            Change::PutMessage(message(1, 1, 1, 1, "old", Some(100))),
            Change::PutMessage(message(2, 1, 1, 2, "old but opened", Some(100))),
            Change::PutMessage(message(3, 1, 1, 3, "recent", Some(171))),
            Change::PutMessage(message(4, 1, 1, 4, "undated", None)),
            Change::PutMessage(message(5, 1, 2, 1, "old elsewhere", Some(100))),
            Change::PutBody(m(1), body("a")),
            Change::PutBody(m(2), body("b")),
            Change::PutBody(m(3), body("c")),
            Change::PutBody(m(4), body("d")),
            Change::PutBody(m(5), body("e")),
            Change::MarkOpened(m(2)),
        ],
    )
    .unwrap();
    assert_eq!(store.prune_bodies(at(200)).unwrap(), 2);
    assert_eq!(store.body(m(1)).unwrap(), None);
    assert_eq!(store.body(m(5)).unwrap(), None);
    for kept in [2, 3, 4] {
        assert!(store.body(m(kept)).unwrap().is_some(), "message {kept}");
    }
    assert_eq!(store.prune_bodies(at(200)).unwrap(), 0);
}

pub fn retention_names_the_bodies_to_fetch<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(
        &mut store,
        vec![
            Change::PutMessage(message(1, 1, 1, 9, "recent", Some(171))),
            Change::PutMessage(message(2, 1, 1, 3, "old", Some(100))),
            Change::PutMessage(message(3, 1, 1, 5, "old but opened", Some(100))),
            Change::PutMessage(message(4, 1, 1, 7, "undated", None)),
            Change::PutMessage(message(5, 1, 1, 1, "recent, kept", Some(190))),
            Change::PutMessage(message(6, 1, 2, 1, "elsewhere", Some(190))),
            Change::MarkOpened(m(3)),
            Change::PutBody(m(5), body("kept")),
        ],
    )
    .unwrap();
    assert_eq!(store.bodies_to_fetch(FolderId::new(1), at(200)).unwrap(), [m(3), m(4), m(1)]);
    assert_eq!(store.bodies_to_fetch(FolderId::new(9), at(200)).unwrap(), []);
    assert_eq!(store.config().body_days, 30);
}

/// One test per contract function. `$make` gives the store and what has to live as long as the test (its directory).
#[macro_export]
macro_rules! contract_tests {
    ($make:expr) => {
        contract_tests!(
            $make;
            a_new_store_is_empty,
            accounts_and_folders_read_back_in_id_order,
            a_put_replaces_the_record_with_the_same_id,
            a_folder_needs_its_account,
            a_refused_batch_changes_nothing,
            an_empty_batch_is_fine,
            folder_state_is_kept_per_folder,
            messages_read_back_in_uid_order,
            a_message_needs_its_folder_and_account,
            a_uid_belongs_to_one_message_per_folder,
            a_replaced_message_keeps_its_body_and_opened_mark,
            flags_are_set_on_the_held_message,
            bodies_are_kept_replaced_and_removed,
            an_empty_and_a_binary_body_read_back,
            removals_take_what_hangs_below,
            a_removed_folder_frees_its_ids,
            search_gives_everything_newest_first_undated_last,
            search_by_account_and_folder,
            search_by_from_to_and_subject_without_case,
            search_by_date,
            search_by_body_words,
            search_reads_operators_as_words,
            retention_drops_old_unopened_bodies,
            retention_names_the_bodies_to_fetch,
            the_local_tables_start_empty,
            jobs_wait_in_the_order_they_came,
            a_job_needs_its_account,
            drafts_read_back_per_account,
            a_draft_needs_its_account,
            the_outbox_follows_its_draft,
            the_schedule_reads_back_by_due_time,
            a_schedule_entry_needs_what_it_names,
            a_schedule_entry_leaves_with_what_it_names,
            a_removed_event_takes_the_entries_of_its_alerts,
            senders_are_one_per_address_without_case,
            message_state_is_kept_with_its_message,
            calendars_hold_events_and_alerts,
            a_removed_account_takes_its_local_records,
            a_refused_batch_changes_no_local_record,
        );
    };
    ($make:expr; $($name:ident),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                let (store, _keep) = $make;
                contract::$name(store);
            }
        )+
    };
}
