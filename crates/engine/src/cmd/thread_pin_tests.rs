// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The answers of `mail.why`, `mail.thread`, `mail.open` and `mail.list` over the test mailbox and over a hostile one
//! (a reference loop, a thread of 10 000, references to messages not there, Message-IDs given twice), pinned as they
//! stood before #406 read one thread without grouping the store: each thread is the one `group_threads` finds over
//! every message, and the answers hash to the digest written down then.

use inboxcraft_model::{AccountId, Envelope, FolderId, Message, MessageId, MessageRef, MessageState, Summary, Uid, group_threads};
use inboxcraft_store::{Batch, Change, Memory, Sqlite, Storage, StoreConfig};
use std::cell::Cell;
use std::rc::Rc;

use serde_json::{Value, json};

use crate::test_support::{NOW, address, at, clock, mailbox, session_with};
use crate::cmd::id_range_tests::copy;
use crate::{Registry, Session};

/// The first id of the long thread, and how many it holds.
const CHAIN: u64 = 1000;
const CHAIN_LEN: u64 = 10_000;

fn link(id: &str) -> MessageRef {
    MessageRef::new(id).unwrap()
}

/// A message `id` of `account` in `folder`, named `own` (its Message-ID, if any), answering `answers` and referring to
/// `refers`, `received` seconds after `NOW`.
fn hostile(id: u64, account: u64, folder: u64, own: Option<&str>, answers: &[&str], refers: &[&str], received: i64) -> Change {
    let envelope = Envelope::new(&format!("Message {id}"))
        .with_from(vec![address("Someone", "someone@example.org")])
        .with_to(vec![address("Anna", "anna@example.org")])
        .with_message_id(own.map(link))
        .with_in_reply_to(answers.iter().map(|id| link(id)).collect())
        .with_references(refers.iter().map(|id| link(id)).collect());
    let uid = Uid::new(u32::try_from(id).unwrap()).unwrap();
    let summary = Summary::new(uid, envelope).with_received(Some(at(NOW + received)));
    Change::PutMessage(Message::new(MessageId::new(id), AccountId::new(account), FolderId::new(folder), summary))
}

/// The test mailbox with the hostile messages besides: the ids worth asking about come with it.
fn hostile_store() -> (Memory, Vec<u64>) {
    let mut store = mailbox();
    let mut batch = Batch::new()
        // A loop: each refers to the next, the last to the first; and one that answers itself.
        .with(hostile(100, 1, 1, Some("loop-1@example.org"), &[], &["loop-2@example.org"], 10))
        .with(hostile(101, 1, 1, Some("loop-2@example.org"), &[], &["loop-3@example.org"], 20))
        .with(hostile(102, 2, 4, Some("loop-3@example.org"), &[], &["loop-1@example.org"], 30))
        .with(hostile(103, 1, 1, Some("self@example.org"), &["self@example.org"], &["self@example.org"], 40))
        // References to messages not there: the two that name the same missing one share a thread.
        .with(hostile(110, 1, 1, None, &["ghost-1@example.org"], &[], 50))
        .with(hostile(111, 1, 3, None, &[], &["ghost-1@example.org", "ghost-2@example.org"], 60))
        .with(hostile(112, 2, 4, None, &[], &["ghost-2@example.org"], 70))
        .with(hostile(113, 1, 1, None, &[], &["nobody@example.org"], 80))
        // One Message-ID given twice, an answer to it, and a copy in Sent.
        .with(hostile(120, 1, 1, Some("dup@example.org"), &[], &[], 90))
        .with(hostile(121, 1, 1, Some("dup@example.org"), &[], &[], 90))
        .with(hostile(122, 1, 1, None, &["dup@example.org"], &[], 100))
        .with(hostile(123, 1, 2, Some("dup@example.org"), &[], &[], 110));
    for n in 0..CHAIN_LEN {
        let own = format!("chain-{n}@example.org");
        let previous = n.checked_sub(1).map(|p| format!("chain-{p}@example.org"));
        let answers: Vec<&str> = previous.as_deref().into_iter().collect();
        let refers: Vec<&str> = if n > 1 { vec!["chain-0@example.org"] } else { Vec::new() };
        batch.push(hostile(CHAIN + n, 1, 3, Some(&own), &answers, &refers, 200 + i64::try_from(n).unwrap()));
    }
    store.apply(&batch).unwrap();
    let probes = vec![1, 2, 3, 4, 5, 100, 101, 102, 103, 110, 111, 112, 113, 120, 121, 122, 123, CHAIN, CHAIN + 1, CHAIN + 4321, CHAIN + CHAIN_LEN - 1];
    (store, probes)
}

/// FNV-1a over `bytes`, on from `hash`.
fn fnv(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

/// Every answer the test asks for, in order, hashed: `mail.why` and `mail.thread` of each probe, the lists, then
/// `mail.open` of each probe and the lists again.
fn digest(session: &mut Session, probes: &[u64]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    let mut ask = |session: &mut Session, command: &str, params: Value| {
        let answer = session.run(command, &params).unwrap();
        hash = fnv(hash, format!("{command} {params} {answer}\n").as_bytes());
    };
    let lists = [json!({}), json!({ "role": "sent" }), json!({ "folder": 3, "limit": 1000 }), json!({ "folder": 4 })];
    for id in probes {
        ask(session, "mail.why", json!({ "id": id }));
        ask(session, "mail.thread", json!({ "id": id }));
    }
    for list in &lists {
        ask(session, "mail.list", list.clone());
    }
    for id in probes {
        ask(session, "mail.open", json!({ "id": id }));
    }
    for list in &lists {
        ask(session, "mail.list", list.clone());
    }
    hash
}

/// Every message of `store`, its threads as `group_threads` makes them.
fn threads(store: &dyn Storage) -> Vec<Vec<MessageId>> {
    let mut messages = Vec::new();
    for account in store.accounts().unwrap() {
        for folder in store.folders(account.id()).unwrap() {
            messages.extend(store.messages(folder.id()).unwrap().into_iter().map(|entry| entry.message().clone()));
        }
    }
    messages.sort_by_key(Message::id);
    group_threads(&messages)
}

/// `mail.thread` and the thread count `mail.why` gives of each probe are the thread `group_threads` finds.
fn threads_are_the_grouped_ones(session: &mut Session, probes: &[u64]) {
    let grouped = threads(session.store());
    for id in probes {
        let thread = grouped.iter().find(|thread| thread.contains(&MessageId::new(*id))).unwrap();
        let answer = session.run("mail.thread", &json!({ "id": id })).unwrap();
        assert_eq!(answer["total"], json!(thread.len()), "{id}");
        let shown: Vec<u64> = answer["messages"].as_array().unwrap().iter().map(|message| message["id"].as_u64().unwrap()).collect();
        let newest: Vec<u64> = thread.iter().skip(thread.len().saturating_sub(1000)).map(|id| id.get()).collect();
        assert_eq!(shown, newest, "{id}");
        let why = session.run("mail.why", &json!({ "id": id })).unwrap();
        let said = why["reasons"].as_array().unwrap().iter().find(|reason| reason["kind"] == "thread").map(|reason| reason["args"]["messages"].clone());
        assert_eq!(said, (thread.len() > 1).then(|| json!(thread.len())), "{id}");
    }
}

/// `store` copied into a new SQLite store, and its directory.
fn on_sqlite(store: &dyn Storage) -> (Sqlite, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut sqlite = Sqlite::open(dir.path(), StoreConfig::default()).unwrap();
    copy(store, &mut sqlite);
    (sqlite, dir)
}

#[test]
fn the_threads_are_the_ones_grouping_every_message_finds() {
    let (store, probes) = hostile_store();
    let mut session = session_with(store, Registry::builtin().unwrap());
    threads_are_the_grouped_ones(&mut session, &probes);
    let mut session = session_with(mailbox(), Registry::builtin().unwrap());
    threads_are_the_grouped_ones(&mut session, &[1, 2, 3, 4, 5]);
}

#[test]
fn on_sqlite_the_threads_are_the_ones_grouping_every_message_finds() {
    let (store, probes) = hostile_store();
    let (store, _dir) = on_sqlite(&store);
    let mut session = Session::with_registry(Box::new(store), Box::new(clock()), Registry::builtin().unwrap()).unwrap();
    threads_are_the_grouped_ones(&mut session, &probes);
    let (store, _dir) = on_sqlite(&mailbox());
    let mut session = Session::with_registry(Box::new(store), Box::new(clock()), Registry::builtin().unwrap()).unwrap();
    threads_are_the_grouped_ones(&mut session, &[1, 2, 3, 4, 5]);
}

const MAILBOX_DIGEST: u64 = 4_121_917_223_662_759_176;
const HOSTILE_DIGEST: u64 = 15_720_900_201_687_580_080;

#[test]
fn the_answers_over_the_test_mailbox_are_as_before() {
    let mut session = session_with(mailbox(), Registry::builtin().unwrap());
    assert_eq!(digest(&mut session, &[1, 2, 3, 4, 5]), MAILBOX_DIGEST);
}

#[test]
fn the_answers_over_a_hostile_mailbox_are_as_before() {
    let (store, probes) = hostile_store();
    let mut session = session_with(store, Registry::builtin().unwrap());
    assert_eq!(digest(&mut session, &probes), HOSTILE_DIGEST);
}

/// The same answers, word for word, from the SQLite store the app runs on.
#[test]
fn on_sqlite_the_answers_are_the_same() {
    let (store, _dir) = on_sqlite(&mailbox());
    let mut session = Session::with_registry(Box::new(store), Box::new(clock()), Registry::builtin().unwrap()).unwrap();
    assert_eq!(digest(&mut session, &[1, 2, 3, 4, 5]), MAILBOX_DIGEST);
    let (store, probes) = hostile_store();
    let (store, _dir) = on_sqlite(&store);
    let mut session = Session::with_registry(Box::new(store), Box::new(clock()), Registry::builtin().unwrap()).unwrap();
    assert_eq!(digest(&mut session, &probes), HOSTILE_DIGEST);
}

/// A store that counts the folders read whole (`.1`), the links looked up (`.2`) and the messages those lookups
/// gave back (`.3`).
#[derive(Debug)]
struct Reads<S>(S, Counter, Counter, Counter);

#[rustfmt::skip]
impl<S: Storage> Storage for Reads<S> {
    fn config(&self) -> inboxcraft_store::StoreConfig { self.0.config() }
    fn apply(&mut self, batch: &Batch) -> Result<(), inboxcraft_store::StoreError> { self.0.apply(batch) }
    fn accounts(&self) -> Result<Vec<inboxcraft_model::Account>, inboxcraft_store::StoreError> { self.0.accounts() }
    fn folders(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Folder>, inboxcraft_store::StoreError> { self.0.folders(a) }
    fn folder_state(&self, f: FolderId) -> Result<Option<inboxcraft_model::FolderState>, inboxcraft_store::StoreError> { self.0.folder_state(f) }
    fn messages(&self, f: FolderId) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.1.set(self.1.get().saturating_add(1)); self.0.messages(f) }
    fn message(&self, m: MessageId) -> Result<Option<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.message(m) }
    fn linked(&self, l: &MessageRef, k: &std::collections::BTreeSet<MessageId>) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> {
        self.2.set(self.2.get().saturating_add(1));
        let found = self.0.linked(l, k)?;
        self.3.set(self.3.get().saturating_add(u32::try_from(found.len()).unwrap()));
        Ok(found)
    }
    fn body(&self, m: MessageId) -> Result<Option<inboxcraft_store::Body>, inboxcraft_store::StoreError> { self.0.body(m) }
    fn search(&self, s: &inboxcraft_store::Search) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.search(s) }
    fn queued_jobs(&self) -> Result<Vec<inboxcraft_model::Job>, inboxcraft_store::StoreError> { self.0.queued_jobs() }
    fn drafts(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Draft>, inboxcraft_store::StoreError> { self.0.drafts(a) }
    fn newest_drafts(&self, a: AccountId, n: usize) -> Result<(u64, Vec<inboxcraft_model::Draft>), inboxcraft_store::StoreError> { self.0.newest_drafts(a, n) }
    fn draft(&self, d: inboxcraft_model::DraftId) -> Result<Option<inboxcraft_model::Draft>, inboxcraft_store::StoreError> { self.0.draft(d) }
    fn outbox(&self) -> Result<Vec<inboxcraft_model::Outgoing>, inboxcraft_store::StoreError> { self.0.outbox() }
    fn schedule(&self) -> Result<Vec<inboxcraft_model::Scheduled>, inboxcraft_store::StoreError> { self.0.schedule() }
    fn sender(&self, e: &inboxcraft_model::EmailAddress) -> Result<Option<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.0.sender(e) }
    fn senders(&self) -> Result<Vec<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.0.senders() }
    fn message_state(&self, m: MessageId) -> Result<MessageState, inboxcraft_store::StoreError> { self.0.message_state(m) }
    fn message_states(&self) -> Result<Vec<(MessageId, MessageState)>, inboxcraft_store::StoreError> { self.0.message_states() }
    fn calendars(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Calendar>, inboxcraft_store::StoreError> { self.0.calendars(a) }
    fn sync_token(&self, c: inboxcraft_model::CalendarId) -> Result<Option<String>, inboxcraft_store::StoreError> { self.0.sync_token(c) }
    fn calendar_events(&self, c: inboxcraft_model::CalendarId) -> Result<Vec<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_events(c) }
    fn calendar_event(&self, e: inboxcraft_model::CalendarEventId) -> Result<Option<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_event(e) }
    fn alerts(&self, e: inboxcraft_model::CalendarEventId) -> Result<Vec<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alerts(e) }
    fn alert(&self, a: inboxcraft_model::AlertId) -> Result<Option<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alert(a) }
    fn stale_bodies(&self, t: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.stale_bodies(t) }
    fn missing_bodies(&self, f: FolderId, t: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.missing_bodies(f, t) }
}

/// One count of [`Reads`].
type Counter = Rc<Cell<u32>>;

/// `store` behind a counting [`Reads`], in a session, and its counters.
fn counted<S: Storage + 'static>(store: S) -> (Session, Counter, Counter, Counter) {
    let (folders, links, returned) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let store = Reads(store, folders.clone(), links.clone(), returned.clone());
    let session = Session::with_registry(Box::new(store), Box::new(clock()), Registry::builtin().unwrap()).unwrap();
    (session, folders, links, returned)
}

/// One message's thread reads no folder whole: in a store of over 10 000 messages, `mail.thread` and `mail.why` look
/// up each link of the thread once, a loop of references included, and the thread of 10 000 is found whole.
#[test]
fn a_thread_is_found_by_its_links_not_by_reading_the_store() {
    let (store, _) = hostile_store();
    let (mut session, folders, links, returned) = counted(store);
    for (id, total, looked_up) in [(100, 3, 3), (103, 1, 1), (111, 3, 2), (113, 1, 1), (122, 4, 1), (1, 1, 0), (CHAIN + 5, CHAIN_LEN, CHAIN_LEN)] {
        for command in ["mail.thread", "mail.why"] {
            folders.set(0);
            links.set(0);
            returned.set(0);
            let answer = session.run(command, &json!({ "id": id })).unwrap();
            assert_eq!(folders.get(), 0, "{command} {id} read a folder whole");
            assert_eq!(u64::from(links.get()), looked_up, "{command} {id}");
            assert_eq!(u64::from(returned.get()), total - 1, "{command} {id}: a message read twice");
            if command == "mail.thread" {
                assert_eq!(answer["total"], total, "{id}");
            }
        }
    }
}

/// The first id of the thread written as a mailing list writes it, and how many it holds.
const LIST: u64 = 20_000;
const LIST_LEN: u64 = 10_000;

/// The test mailbox and a thread of [`LIST_LEN`] as RFC 5322 asks a reply to be written: each answers the one before
/// and refers to what that one referred to and to it, the newest 64 kept, so a message is named by up to 66 others.
fn list_store() -> Memory {
    let mut store = mailbox();
    let mut batch = Batch::new();
    let mut refers: Vec<String> = Vec::new();
    for n in 0..LIST_LEN {
        let own = format!("list-{n}@example.org");
        let previous = n.checked_sub(1).map(|p| format!("list-{p}@example.org"));
        let answers: Vec<&str> = previous.as_deref().into_iter().collect();
        let named: Vec<&str> = refers.iter().map(String::as_str).collect();
        batch.push(hostile(LIST + n, 1, 3, Some(&own), &answers, &named, 200 + i64::try_from(n).unwrap()));
        refers.push(own);
        if refers.len() > 64 {
            refers.remove(0);
        }
    }
    store.apply(&batch).unwrap();
    store
}

/// A mailing list's thread of 10 000 costs one record per message, not one per link that names it (up to 66 here),
/// on both stores: the messages the lookups give back are the thread less the one asked about.
#[test]
fn each_message_of_a_list_thread_is_read_once() {
    let memory = list_store();
    let (sqlite, _dir) = on_sqlite(&memory);
    let sessions = [counted(memory), counted(sqlite)];
    for (backend, (mut session, folders, links, returned)) in ["memory", "sqlite"].into_iter().zip(sessions) {
        for id in [LIST, LIST + 1, LIST + 4321, LIST + LIST_LEN - 1] {
            for command in ["mail.thread", "mail.why"] {
                folders.set(0);
                links.set(0);
                returned.set(0);
                let answer = session.run(command, &json!({ "id": id })).unwrap();
                assert_eq!(folders.get(), 0, "{backend} {command} {id} read a folder whole");
                assert_eq!(u64::from(links.get()), LIST_LEN, "{backend} {command} {id}");
                assert_eq!(u64::from(returned.get()), LIST_LEN - 1, "{backend} {command} {id}: a message read more than once");
                if command == "mail.thread" {
                    assert_eq!(answer["total"], LIST_LEN, "{backend} {id}");
                }
            }
        }
    }
}

/// How long `mail.thread` and `mail.why` take on the mailing list's thread of 10 000, on both stores: a timing, run
/// by hand in release (`cargo test --release -p inboxcraft-engine --lib list_thread_timing -- --ignored --nocapture`).
#[test]
#[ignore = "a timing, run by hand in release"]
#[allow(clippy::disallowed_methods, reason = "a measurement of the wall time")]
fn list_thread_timing() {
    let memory = list_store();
    let (sqlite, _dir) = on_sqlite(&memory);
    let sessions: [(&str, Session); 2] = [
        ("memory", session_with(memory, Registry::builtin().unwrap())),
        ("sqlite", Session::with_registry(Box::new(sqlite), Box::new(clock()), Registry::builtin().unwrap()).unwrap()),
    ];
    for (backend, mut session) in sessions {
        for command in ["mail.thread", "mail.why"] {
            let mut times: Vec<f64> = (0..15)
                .map(|_| {
                    let start = std::time::Instant::now();
                    session.run(command, &json!({ "id": LIST + 4321 })).unwrap();
                    start.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            times.sort_by(f64::total_cmp);
            println!("{backend} {command}: median {:.3} ms, max {:.3} ms", times[7], times[14]);
        }
    }
}
