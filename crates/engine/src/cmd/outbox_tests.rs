// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `outbox.list` and `outbox.retry`: each outbox item with where it stands, a refusal with the server's reply and its
//! enhanced status code, and a refused draft sent again by command.

use inboxcraft_model::{AccountId, DraftId, ErrorKind, Event, EventKind, JobKind, OutboxState};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use super::send_tests::store;
use crate::test_support::{NOW, at, send_now};
use crate::{AgentClass, Caller, EngineError, Session};

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (Session::new(Box::new(store()), Box::new(clock.clone())).unwrap(), clock)
}

/// A draft of account 1 to Béla with the subject `subject`, sent; its id.
fn sent(session: &mut Session, clock: &FakeClock, subject: &str) -> u64 {
    let id = session.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": id, "to": ["Béla <bela@example.org>"], "subject": subject })).unwrap();
    send_now(session, clock, "mail.send", id);
    session.take_jobs();
    id
}

fn failed(session: &mut Session, draft: u64, kind: ErrorKind, reply: Option<&str>) {
    let kind = EventKind::SendFailed { draft: DraftId::new(draft), kind, reply: reply.map(str::to_owned) };
    session.apply(&Event::new(AccountId::new(1), None, kind)).unwrap();
}

fn items(session: &mut Session) -> Vec<Value> {
    session.run("outbox.list", &json!({})).unwrap()["items"].as_array().unwrap().clone()
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

#[test]
fn listing_is_safe_and_a_retry_is_for_the_owner() {
    let (session, _) = session();
    let class = |id: &str| session.registry().get(id).map(|spec| spec.class);
    assert_eq!(class("outbox.list"), Some(Some(AgentClass::Safe)));
    assert_eq!(class("outbox.retry"), Some(Some(AgentClass::Owner)));
}

#[test]
fn the_outbox_lists_each_item_with_where_it_stands() {
    let (mut session, clock) = session();
    assert_eq!(items(&mut session), Vec::<Value>::new());
    let first = sent(&mut session, &clock, "Budget");
    let second = sent(&mut session, &clock, "Lunch");
    failed(&mut session, second, ErrorKind::MessageRefused, Some("550 5.1.1 No such user"));
    let listed = items(&mut session);
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert_eq!(listed[0]["draft"], json!(first));
    assert_eq!(listed[0]["state"], json!("sending"));
    assert_eq!(listed[0]["subject"], json!("Budget"));
    assert_eq!(listed[0]["to"], json!(["Béla <bela@example.org>"]));
    assert_eq!(listed[0]["reply"], Value::Null);
    assert_eq!(listed[0]["code"], Value::Null);
    assert_eq!(listed[1]["draft"], json!(second));
    assert_eq!(listed[1]["state"], json!("refused"));
    assert_eq!(listed[1]["reply"], json!("550 5.1.1 No such user"));
    assert_eq!(listed[1]["code"], json!("5.1.1"));
    assert_eq!(listed[1]["permanent"], json!(true));
}

#[test]
fn a_refusal_without_an_enhanced_code_has_none() {
    let (mut session, clock) = session();
    let id = sent(&mut session, &clock, "Budget");
    failed(&mut session, id, ErrorKind::MessageRefused, Some("554 Rejected"));
    let listed = items(&mut session);
    assert_eq!((&listed[0]["reply"], &listed[0]["code"]), (&json!("554 Rejected"), &Value::Null));
    assert_eq!(listed[0]["permanent"], json!(true));
    // A temporary refusal can be tried again as it is.
    let later = sent(&mut session, &clock, "Later");
    failed(&mut session, later, ErrorKind::MessageRefused, Some("451 4.3.0 Try again later"));
    let listed = items(&mut session);
    assert_eq!((&listed[1]["code"], &listed[1]["permanent"]), (&json!("4.3.0"), &json!(false)));
    // A failure the server did not answer says its cause and has no code.
    let cut = sent(&mut session, &clock, "Cut");
    failed(&mut session, cut, ErrorKind::TimedOut, None);
    let listed = items(&mut session);
    assert_eq!(listed[2]["reply"], json!(ErrorKind::TimedOut.to_string()));
    assert_eq!((&listed[2]["code"], &listed[2]["permanent"]), (&Value::Null, &json!(false)));
}

#[test]
fn a_refused_draft_is_sent_again_by_command() {
    let (mut session, clock) = session();
    let id = sent(&mut session, &clock, "Budget");
    // Not refused: there is nothing to try again.
    assert!(bad(session.run("outbox.retry", &json!({ "id": id }))));
    failed(&mut session, id, ErrorKind::MessageRefused, Some("550 5.1.1 No such user"));
    // An agent may not send it.
    assert!(matches!(session.run_as(Caller::Agent, "outbox.retry", &json!({ "id": id })), Err(EngineError::NotAllowed { .. })));
    assert!(session.take_jobs().is_empty());
    let answer = send_now(&mut session, &clock, "outbox.retry", id);
    assert_eq!((&answer["id"], &answer["state"]), (&json!(id), &json!("waiting")));
    let jobs = session.take_jobs();
    assert!(matches!(jobs.as_slice(), [job] if matches!(&job.kind, JobKind::Send { draft, .. } if *draft == DraftId::new(id))), "{jobs:?}");
    assert_eq!(session.store().outbox().unwrap()[0].state, OutboxState::Sending);
    // No such item, and junk.
    assert!(matches!(session.run("outbox.retry", &json!({ "id": 999 })), Err(EngineError::NotFound { .. })));
    assert!(bad(session.run("outbox.retry", &json!({ "id": "1" }))));
    assert!(bad(session.run("outbox.retry", &json!({ "id": id, "to": "x" }))));
}
