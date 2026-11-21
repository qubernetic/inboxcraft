// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `mail.row`: one message as a list shows it, read by its id alone, whatever the store's size and however long the
//! thread it is in.

use std::cell::Cell;
use std::rc::Rc;

use inboxcraft_model::{AccountId, Draft, DraftId, Envelope, FolderId, Message, MessageId, MessageRef, MessageState, Summary, Uid};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use serde_json::json;

use super::MAX_LIST;
use crate::test_support::{NOW, address, at, change, mailbox, sample_session};
use crate::{AgentClass, Caller, EngineError, Registry, Session};

#[test]
fn mail_row_is_safe_and_answers_the_row_the_list_gives() {
    let registry = Registry::builtin().unwrap();
    assert_eq!(registry.get("mail.row").and_then(|spec| spec.class), Some(AgentClass::Safe));
    let mut session = sample_session();
    let first = session.run("mail.list", &json!({ "role": "inbox" })).unwrap()["messages"][0]["id"].as_u64().unwrap();
    // The triage state the list carries comes with the row.
    let state = MessageState { pinned: true, follow_up: Some(at(NOW + 3600)), ..MessageState::default() };
    change(&mut session, Change::SetMessageState(MessageId::new(first), state));
    let listed = session.run("mail.list", &json!({ "role": "inbox" })).unwrap();
    let rows = listed["messages"].as_array().unwrap();
    assert!(!rows.is_empty());
    for listed in rows {
        let id = listed["id"].as_u64().unwrap();
        let answer = session.run_as(Caller::Agent, "mail.row", &json!({ "id": id })).unwrap();
        assert_eq!(answer["message"], *listed, "message {id}");
    }
    let pinned = session.run("mail.row", &json!({ "id": first })).unwrap();
    assert_eq!(pinned["message"]["pinned"], true);
    assert!(pinned["message"]["follow_up"].is_string(), "{pinned}");
}

#[test]
fn an_unknown_message_is_not_found_and_junk_is_refused() {
    let mut session = sample_session();
    assert_eq!(session.run("mail.row", &json!({ "id": 9999 })).unwrap_err(), EngineError::NotFound { what: "message", id: 9999 });
    for params in [json!({}), json!({ "id": "1" }), json!({ "id": -1 }), json!({ "id": 1, "folder": 1 })] {
        assert_eq!(session.run("mail.row", &params).unwrap_err().code(), "bad_params", "{params}");
    }
}

/// A store that counts how often a whole folder, every message state or every draft was read.
#[derive(Debug)]
struct Counting(Memory, Rc<Cell<u32>>);

impl Counting {
    fn count(&self) {
        self.1.set(self.1.get().saturating_add(1));
    }
}

#[rustfmt::skip]
impl Storage for Counting {
    fn config(&self) -> inboxcraft_store::StoreConfig { self.0.config() }
    fn apply(&mut self, batch: &Batch) -> Result<(), inboxcraft_store::StoreError> { self.0.apply(batch) }
    fn accounts(&self) -> Result<Vec<inboxcraft_model::Account>, inboxcraft_store::StoreError> { self.0.accounts() }
    fn folders(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Folder>, inboxcraft_store::StoreError> { self.0.folders(a) }
    fn folder_state(&self, f: FolderId) -> Result<Option<inboxcraft_model::FolderState>, inboxcraft_store::StoreError> { self.0.folder_state(f) }
    fn messages(&self, f: FolderId) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.count(); self.0.messages(f) }
    fn message(&self, m: MessageId) -> Result<Option<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.message(m) }
    fn linked(&self, l: &inboxcraft_model::MessageRef, k: &std::collections::BTreeSet<MessageId>) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.linked(l, k) }
    fn body(&self, m: MessageId) -> Result<Option<inboxcraft_store::Body>, inboxcraft_store::StoreError> { self.0.body(m) }
    fn search(&self, s: &inboxcraft_store::Search) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.count(); self.0.search(s) }
    fn queued_jobs(&self) -> Result<Vec<inboxcraft_model::Job>, inboxcraft_store::StoreError> { self.0.queued_jobs() }
    fn drafts(&self, a: AccountId) -> Result<Vec<Draft>, inboxcraft_store::StoreError> { self.count(); self.0.drafts(a) }
    fn newest_drafts(&self, a: AccountId, n: usize) -> Result<(u64, Vec<Draft>), inboxcraft_store::StoreError> { self.count(); self.0.newest_drafts(a, n) }
    fn draft(&self, d: DraftId) -> Result<Option<Draft>, inboxcraft_store::StoreError> { self.0.draft(d) }
    fn outbox(&self) -> Result<Vec<inboxcraft_model::Outgoing>, inboxcraft_store::StoreError> { self.0.outbox() }
    fn schedule(&self) -> Result<Vec<inboxcraft_model::Scheduled>, inboxcraft_store::StoreError> { self.0.schedule() }
    fn sender(&self, e: &inboxcraft_model::EmailAddress) -> Result<Option<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.0.sender(e) }
    fn senders(&self) -> Result<Vec<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.count(); self.0.senders() }
    fn message_state(&self, m: MessageId) -> Result<MessageState, inboxcraft_store::StoreError> { self.0.message_state(m) }
    fn message_states(&self) -> Result<Vec<(MessageId, MessageState)>, inboxcraft_store::StoreError> { self.count(); self.0.message_states() }
    fn calendars(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Calendar>, inboxcraft_store::StoreError> { self.0.calendars(a) }
    fn sync_token(&self, c: inboxcraft_model::CalendarId) -> Result<Option<String>, inboxcraft_store::StoreError> { self.0.sync_token(c) }
    fn calendar_events(&self, c: inboxcraft_model::CalendarId) -> Result<Vec<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_events(c) }
    fn calendar_event(&self, e: inboxcraft_model::CalendarEventId) -> Result<Option<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_event(e) }
    fn alerts(&self, e: inboxcraft_model::CalendarEventId) -> Result<Vec<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alerts(e) }
    fn alert(&self, a: inboxcraft_model::AlertId) -> Result<Option<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alert(a) }
    fn stale_bodies(&self, t: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.stale_bodies(t) }
    fn missing_bodies(&self, f: FolderId, t: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.missing_bodies(f, t) }
}

/// Message `id` of account 1 in its inbox, `minutes` old, with `envelope`.
fn message(id: u64, minutes: i64, envelope: Envelope) -> Change {
    let envelope = envelope.with_from(vec![address("Someone", "someone@example.org")]).with_to(vec![address("Anna", "anna@example.org")]);
    let summary = Summary::new(Uid::new(u32::try_from(id).unwrap()).unwrap(), envelope).with_received(Some(at(NOW - minutes * 60)));
    Change::PutMessage(Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(1), summary))
}

/// The test mailbox with message 100 and the `answers` messages that answer it, every one newer.
fn long_thread(answers: u64) -> Memory {
    let mut store = mailbox();
    let mut batch = Batch::new().with(message(100, 100_000, Envelope::new("Plans").with_message_id(MessageRef::new("<plans@example.org>"))));
    for n in 1..=answers {
        let answering = Envelope::new("Re: Plans").with_in_reply_to(vec![MessageRef::new("<plans@example.org>").unwrap()]);
        batch = batch.with(message(100 + n, 100_000 - i64::try_from(n).unwrap(), answering));
    }
    store.apply(&batch).unwrap();
    store
}

/// The oldest message of a thread longer than a list stays readable: the thread gives its newest only.
#[test]
fn mail_row_reads_a_message_a_long_thread_leaves_out() {
    let mut session = Session::new(Box::new(long_thread(MAX_LIST)), Box::new(inboxcraft_time::FakeClock::new(at(NOW)))).unwrap();
    let thread = session.run("mail.thread", &json!({ "id": 100 })).unwrap();
    assert!(thread["messages"].as_array().unwrap().iter().all(|message| message["id"] != 100), "the thread leaves the oldest out");
    assert_eq!(session.run("mail.row", &json!({ "id": 100 })).unwrap()["message"]["id"], 100);
}

/// Reading one row reads no folder, no list of states and no drafts: its cost does not grow with the store.
#[test]
fn mail_row_reads_one_message_whatever_the_store_s_size() {
    for size in [1, 200] {
        let reads = Rc::new(Cell::new(0));
        let store = Counting(long_thread(size), reads.clone());
        let mut session = Session::new(Box::new(store), Box::new(inboxcraft_time::FakeClock::new(at(NOW)))).unwrap();
        reads.set(0);
        assert_eq!(session.run("mail.row", &json!({ "id": 100 })).unwrap()["message"]["id"], 100);
        assert_eq!(reads.get(), 0, "no whole read for one row in a store of {size}");
        session.run("mail.thread", &json!({ "id": 100 })).unwrap();
        assert!(reads.get() > 0, "the thread reads the store, which the counter sees");
    }
}
