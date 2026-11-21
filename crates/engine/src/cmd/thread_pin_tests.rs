// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The answers of `mail.why`, `mail.thread`, `mail.open` and `mail.list` over the test mailbox and over a hostile one
//! (a reference loop, a thread of 10 000, references to messages not there, Message-IDs given twice), pinned as they
//! stood before #406 read one thread without grouping the store: each thread is the one `group_threads` finds over
//! every message, and the answers hash to the digest written down then.

use inboxcraft_model::{AccountId, Envelope, FolderId, Message, MessageId, MessageRef, Summary, Uid, group_threads};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use serde_json::{Value, json};

use crate::test_support::{NOW, address, at, mailbox, session_with};
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

#[test]
fn the_threads_are_the_ones_grouping_every_message_finds() {
    let (store, probes) = hostile_store();
    let mut session = session_with(store, Registry::builtin().unwrap());
    threads_are_the_grouped_ones(&mut session, &probes);
    let mut session = session_with(mailbox(), Registry::builtin().unwrap());
    threads_are_the_grouped_ones(&mut session, &[1, 2, 3, 4, 5]);
}

#[test]
fn the_answers_over_the_test_mailbox_are_as_before() {
    let mut session = session_with(mailbox(), Registry::builtin().unwrap());
    assert_eq!(digest(&mut session, &[1, 2, 3, 4, 5]), 4_121_917_223_662_759_176);
}

#[test]
fn the_answers_over_a_hostile_mailbox_are_as_before() {
    let (store, probes) = hostile_store();
    let mut session = session_with(store, Registry::builtin().unwrap());
    assert_eq!(digest(&mut session, &probes), 15_720_900_201_687_580_080);
}
