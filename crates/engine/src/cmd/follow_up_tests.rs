// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Follow-up reminders: `mail.follow_up` and `mail.cancel_follow_up` on a sent message, a synced reply that cancels
//! the reminder, and the reminder that fires on a fake clock.

use std::time::Duration;

use inboxcraft_model::{AccountId, Address, Due, Envelope, Event, EventKind, FolderId, Message, MessageId, MessageRef, MessageState, Summary};
use inboxcraft_store::{Change, Memory};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use crate::cmd::act_tests::{store, uid, validity};
use crate::notify::{About, Notice};
use crate::test_support::{NOW, address, at, change};
use crate::{AgentClass, Caller, EngineError, Registry, Session};

/// 2026-11-06T09:00:00Z, a day after [`NOW`].
const TOMORROW: i64 = NOW + 86_400;

/// The sent message of the test store: account 1's Sent folder holds message 5.
const SENT: u64 = 5;

/// The `Message-ID` of the sent message.
const SENT_ID: &str = "budget-2026@example.org";

fn id(text: &str) -> MessageRef {
    MessageRef::new(text).unwrap()
}

fn anna() -> Address {
    address("Anna Example", "anna@example.org")
}

fn bob() -> Address {
    address("Bob", "bob@example.net")
}

/// Someone the owner wrote to; Bob is the owner too (account 2).
fn dora() -> Address {
    address("Dora", "dora@example.com")
}

/// The sent message: from Anna to Dora, with [`SENT_ID`].
fn sent() -> Message {
    let envelope = Envelope::new("Re: Budget for 2026").with_from(vec![anna()]).with_to(vec![dora()]).with_message_id(Some(id(SENT_ID)));
    Message::new(MessageId::new(SENT), AccountId::new(1), FolderId::new(2), Summary::new(uid(3), envelope))
}

/// The act tests' store with message 5 as [`sent`] and message 3 from someone else.
fn mailbox() -> Memory {
    use inboxcraft_store::{Batch, Storage};
    let mut store = store();
    let received = Envelope::new("Garden party").with_from(vec![address("Cecil", "cecil@example.com")]).with_to(vec![bob()]);
    let garden = Message::new(MessageId::new(3), AccountId::new(2), FolderId::new(4), Summary::new(uid(7), received));
    store.apply(&Batch::new().with(Change::PutMessage(sent())).with(Change::PutMessage(garden))).unwrap();
    store
}

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    let session = Session::with_registry(Box::new(mailbox()), Box::new(clock.clone()), Registry::builtin().unwrap()).unwrap();
    (session, clock)
}

/// A message from `from` that names `in_reply_to` and `references`.
fn reply(n: u32, from: Address, in_reply_to: &[&str], references: &[&str]) -> Summary {
    let envelope = Envelope::new("Re: Budget for 2026")
        .with_from(vec![from])
        .with_to(vec![anna()])
        .with_message_id(Some(id(&format!("reply-{n}@example.net"))))
        .with_in_reply_to(in_reply_to.iter().map(|text| id(text)).collect())
        .with_references(references.iter().map(|text| id(text)).collect());
    Summary::new(uid(n), envelope)
}

/// `messages` arrive in account 1's folder `folder`.
fn arrive(session: &mut Session, folder: u64, messages: Vec<Summary>) {
    let added = EventKind::MessagesAdded { folder: FolderId::new(folder), uid_validity: validity(), messages };
    session.apply(&Event::new(AccountId::new(1), None, added)).unwrap();
}

fn reminders(session: &mut Session) -> Vec<Notice> {
    session.take_notices().into_iter().filter(|notice| matches!(notice, Notice::Reminder { .. })).collect()
}

fn follow_up(session: &mut Session, id: u64, until: &str) -> Value {
    session.run("mail.follow_up", &json!({ "id": id, "until": until })).unwrap()
}

fn state(session: &Session, id: u64) -> MessageState {
    session.store().message_state(MessageId::new(id)).unwrap()
}

fn schedule(session: &Session) -> Vec<(i64, Due)> {
    session.store().schedule().unwrap().into_iter().map(|entry| (entry.due.unix_seconds(), entry.what)).collect()
}

#[test]
fn follow_up_is_safe_for_an_agent() {
    let registry = Registry::builtin().unwrap();
    for id in ["mail.follow_up", "mail.cancel_follow_up"] {
        assert_eq!(registry.lookup(id).map(|(_, class)| class), Some(AgentClass::Safe), "{id}");
    }
    let (mut session, _) = session();
    let answer = session.run_as(Caller::Agent, "mail.follow_up", &json!({ "id": SENT, "until": "2026-11-06T09:00:00Z" })).unwrap();
    assert_eq!(answer["changed"], json!(true));
}

#[test]
fn a_follow_up_sets_the_time_and_the_schedule() {
    let (mut session, _) = session();
    let answer = follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    assert_eq!(answer, json!({ "id": SENT, "until": "2026-11-06T09:00:00Z", "changed": true }));
    assert_eq!(state(&session, SENT), MessageState { follow_up: Some(at(TOMORROW)), ..MessageState::default() });
    assert_eq!(schedule(&session), [(TOMORROW, Due::FollowUp(MessageId::new(SENT)))]);
    let listed = session.run("mail.list", &json!({ "role": "sent" })).unwrap();
    assert_eq!(listed["messages"][0]["follow_up"], json!("2026-11-06T09:00:00Z"));
    assert!(session.take_jobs().is_empty(), "a reminder is ours alone: nothing goes to the server");
}

#[test]
fn a_preset_names_the_time() {
    let (mut session, _) = session();
    let answer = session.run("mail.follow_up", &json!({ "id": SENT, "preset": "tomorrow_morning" })).unwrap();
    assert_eq!(answer["until"], json!("2026-11-06T08:00:00Z"));
}

#[test]
fn only_a_sent_message_gets_a_reminder_and_only_ahead() {
    let (mut session, _) = session();
    let bad = |session: &mut Session, params: Value| matches!(session.run("mail.follow_up", &params), Err(EngineError::BadParams { .. }));
    assert!(bad(&mut session, json!({ "id": 3, "until": "2026-11-06T09:00:00Z" })), "a message from someone else");
    assert!(bad(&mut session, json!({ "id": SENT, "until": "2026-11-05T09:00:00Z" })), "now is not ahead");
    assert!(bad(&mut session, json!({ "id": SENT })), "no time");
    assert!(bad(&mut session, json!({ "id": SENT, "until": "2026-11-06T09:00:00Z", "preset": "this_evening" })), "two times");
    assert!(matches!(session.run("mail.follow_up", &json!({ "id": 99, "until": "2026-11-06T09:00:00Z" })), Err(EngineError::NotFound { .. })));
    assert_eq!(schedule(&session), []);
    let mine = session.run("mail.follow_up", &json!({ "id": 1, "until": "2026-11-06T09:00:00Z" })).unwrap();
    assert_eq!(mine["changed"], json!(true), "a message from the owner's own address counts as sent, wherever it is");
}

#[test]
fn a_new_time_replaces_the_old_and_the_same_time_changes_nothing() {
    let (mut session, _) = session();
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    assert_eq!(follow_up(&mut session, SENT, "2026-11-06T09:00:00Z")["changed"], json!(false));
    follow_up(&mut session, SENT, "2026-11-07T09:00:00Z");
    assert_eq!(schedule(&session), [(TOMORROW + 86_400, Due::FollowUp(MessageId::new(SENT)))]);
}

#[test]
fn at_the_due_minute_the_reminder_fires_and_opens_the_sent_message() {
    let (mut session, clock) = session();
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    clock.advance(Duration::from_secs(86_400 - 1));
    assert_eq!(session.tick().unwrap(), []);
    assert_eq!(reminders(&mut session), []);
    clock.advance(Duration::from_secs(1));
    let fired = session.tick().unwrap();
    assert_eq!(fired.len(), 1);
    assert_eq!((fired[0].entry.what, fired[0].missed), (Due::FollowUp(MessageId::new(SENT)), false));
    let about = About { account: AccountId::new(1), message: MessageId::new(SENT), who: "Dora".into(), subject: "Re: Budget for 2026".into() };
    let told = reminders(&mut session);
    assert_eq!(told, [Notice::Reminder { about, missed: false }]);
    assert_eq!(told[0].message(), Some(MessageId::new(SENT)), "a click opens the sent message");
    assert_eq!(state(&session, SENT), MessageState::default());
    assert_eq!(schedule(&session), []);
    arrive(&mut session, 1, vec![reply(40, dora(), &[SENT_ID], &[])]);
    assert_eq!(state(&session, SENT), MessageState::default(), "a reply after the reminder changes nothing");
}

#[test]
fn a_synced_reply_before_the_due_time_cancels_the_reminder() {
    let (mut session, clock) = session();
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    clock.advance(Duration::from_secs(3600));
    session.tick().unwrap();
    arrive(&mut session, 1, vec![reply(40, dora(), &[SENT_ID], &[])]);
    assert_eq!(state(&session, SENT), MessageState::default());
    assert_eq!(schedule(&session), []);
    clock.advance(Duration::from_secs(86_400));
    assert_eq!(session.tick().unwrap(), []);
    assert_eq!(reminders(&mut session), []);
}

#[test]
fn a_reply_further_down_the_thread_cancels_it_too() {
    let (mut session, _) = session();
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    arrive(&mut session, 1, vec![reply(40, dora(), &["other@example.net"], &["first@example.net", SENT_ID])]);
    assert_eq!(schedule(&session), []);
}

#[test]
fn two_replies_at_once_cancel_it_once() {
    let (mut session, _) = session();
    change(&mut session, Change::SetMessageState(MessageId::new(SENT), MessageState { pinned: true, ..MessageState::default() }));
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    arrive(&mut session, 1, vec![reply(40, dora(), &[SENT_ID], &[]), reply(41, address("Cecil", "cecil@example.com"), &[SENT_ID], &[])]);
    assert_eq!(state(&session, SENT), MessageState { pinned: true, ..MessageState::default() }, "the rest of the state stays");
    assert_eq!(schedule(&session), []);
}

#[test]
fn what_is_no_reply_leaves_the_reminder() {
    let (mut session, _) = session();
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    let waiting = [(TOMORROW, Due::FollowUp(MessageId::new(SENT)))];
    arrive(&mut session, 1, vec![reply(40, dora(), &["other@example.net"], &["first@example.net"])]);
    assert_eq!(schedule(&session), waiting, "a reply to another message");
    arrive(&mut session, 1, vec![reply(41, dora(), &[], &[])]);
    assert_eq!(schedule(&session), waiting, "a message that answers nothing");
    arrive(&mut session, 1, vec![reply(42, address("Anna", "ANNA@example.org"), &[SENT_ID], &[])]);
    assert_eq!(schedule(&session), waiting, "the owner's own answer in the thread");
    arrive(&mut session, 1, vec![reply(43, bob(), &[SENT_ID], &[])]);
    assert_eq!(schedule(&session), waiting, "an answer from another of the owner's addresses");
    arrive(&mut session, 2, vec![reply(44, dora(), &[SENT_ID], &[])]);
    assert_eq!(schedule(&session), waiting, "a copy in the Sent folder");
    assert_eq!(state(&session, SENT).follow_up, Some(at(TOMORROW)));
}

#[test]
fn without_a_message_id_no_reply_can_be_told() {
    let (mut session, _) = session();
    let bare =
        Message::new(MessageId::new(SENT), AccountId::new(1), FolderId::new(2), Summary::new(uid(3), Envelope::new("Hello").with_from(vec![anna()])));
    change(&mut session, Change::PutMessage(bare));
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    arrive(&mut session, 1, vec![reply(40, dora(), &[SENT_ID], &[])]);
    assert_eq!(schedule(&session), [(TOMORROW, Due::FollowUp(MessageId::new(SENT)))]);
}

#[test]
fn cancel_takes_the_reminder_off_and_undo_takes_each_back() {
    let (mut session, _) = session();
    let cancel = |session: &mut Session| session.run("mail.cancel_follow_up", &json!({ "id": SENT })).unwrap();
    assert_eq!(cancel(&mut session), json!({ "id": SENT, "changed": false }));
    follow_up(&mut session, SENT, "2026-11-06T09:00:00Z");
    assert_eq!(cancel(&mut session), json!({ "id": SENT, "changed": true }));
    assert_eq!(state(&session, SENT), MessageState::default());
    assert_eq!(schedule(&session), []);
    session.run("history.undo", &json!({})).unwrap();
    assert_eq!(state(&session, SENT).follow_up, Some(at(TOMORROW)), "the cancel is taken back");
    assert_eq!(schedule(&session), [(TOMORROW, Due::FollowUp(MessageId::new(SENT)))]);
    session.run("history.undo", &json!({})).unwrap();
    assert_eq!(state(&session, SENT), MessageState::default(), "the reminder is taken back");
    assert_eq!(schedule(&session), []);
}

#[test]
fn undo_puts_back_a_snooze_and_a_reminder_together() {
    let (mut session, _) = session();
    let both = MessageState { snoozed_until: Some(at(TOMORROW)), follow_up: Some(at(TOMORROW)), ..MessageState::default() };
    change(&mut session, Change::SetMessageState(MessageId::new(SENT), both.clone()));
    session.run("mail.pin", &json!({ "id": SENT, "pinned": true })).unwrap();
    session.run("history.undo", &json!({})).unwrap();
    assert_eq!(state(&session, SENT), both);
    let mut due: Vec<Due> = session.store().schedule().unwrap().into_iter().map(|entry| entry.what).collect();
    due.sort_by_key(|what| matches!(what, Due::FollowUp(_)));
    assert_eq!(due, [Due::Unsnooze(MessageId::new(SENT)), Due::FollowUp(MessageId::new(SENT))], "two entries, two ids");
}
