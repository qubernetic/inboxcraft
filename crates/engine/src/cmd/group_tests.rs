// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The grouped inbox's commands: the group of each row, a sender moved to another group with undo, grouping per
//! account, and the counts per group.

use inboxcraft_model::{AccountId, Event, EventKind, FolderId, Group, Signs, Summary};
use serde_json::{Value, json};

use crate::arrive::screener::tests::{arrive, arrived, from, sender};
use crate::cmd::act_tests::{session, uid, validity};
use crate::{Caller, EngineError, Session};

/// A message from `sender` with UID `n` and the header fields `fields`.
fn with(n: u32, sender: &str, fields: &[(&str, &str)]) -> Summary {
    let summary = from(n, sender);
    let envelope = summary.envelope().clone().with_signs(Signs::read(fields.iter().copied()));
    summary.with_envelope(envelope)
}

/// The test mailbox with a newsletter (UID 30), an update (31) and a person's mail (32) arrived in account 1's inbox.
fn grouped() -> Session {
    let mut session = session();
    arrive(
        &mut session,
        1,
        vec![
            with(30, "news@example.com", &[("List-Id", "<news.example.com>")]),
            with(31, "no-reply@example.com", &[]),
            with(32, "friend@example.com", &[]),
        ],
    );
    session
}

/// The id of the message with UID `n` in account 1.
fn id(session: &Session, n: u32) -> u64 {
    arrived(session, n).id().get()
}

/// The rows of `mail.list` with `params`: (id, group).
fn rows(session: &mut Session, params: &Value) -> Vec<(u64, Value)> {
    let answer = session.run("mail.list", params).unwrap();
    answer["messages"].as_array().unwrap().iter().map(|row| (row["id"].as_u64().unwrap(), row["group"].clone())).collect()
}

fn group_of(session: &mut Session, id: u64) -> Value {
    rows(session, &json!({ "limit": 1000 })).into_iter().find(|(row, _)| *row == id).map(|(_, group)| group).unwrap()
}

#[test]
fn every_row_of_a_grouped_account_says_its_group() {
    let mut session = grouped();
    let (news, update, friend) = (id(&session, 30), id(&session, 31), id(&session, 32));
    assert_eq!(group_of(&mut session, news), json!("newsletters"));
    assert_eq!(group_of(&mut session, update), json!("updates"));
    assert_eq!(group_of(&mut session, friend), json!("people"));
    // The test mailbox's own mail is from the owner's other address: people.
    assert_eq!(group_of(&mut session, 1), json!("people"));
}

#[test]
fn mail_list_lists_one_group() {
    let mut session = grouped();
    let news = id(&session, 30);
    assert_eq!(rows(&mut session, &json!({ "group": "newsletters" })), [(news, json!("newsletters"))]);
    let answer = session.run("mail.list", &json!({ "group": "newsletters" })).unwrap();
    assert_eq!(answer["total"], json!(1));
    assert_eq!(answer["group"], json!("newsletters"));
    let people: Vec<u64> = rows(&mut session, &json!({ "group": "people", "account": 1 })).into_iter().map(|(id, _)| id).collect();
    assert_eq!(people.len(), 3, "{people:?}");
    assert!(rows(&mut session, &json!({ "group": "none" })).is_empty(), "every account is grouped");
    assert!(rows(&mut session, &json!({ "group": "invitations" })).is_empty());
    for bad in [json!("friends"), json!(3), json!("")] {
        assert_eq!(session.run("mail.list", &json!({ "group": bad })).unwrap_err().code(), "bad_params", "{bad}");
    }
}

#[test]
fn a_sender_moved_to_another_group_takes_all_its_mail_and_its_future_mail() {
    let mut session = grouped();
    let news = id(&session, 30);
    let answer = session.run("group.set", &json!({ "address": " News@Example.com ", "group": "people" })).unwrap();
    assert_eq!(answer, json!({ "address": "news@example.com", "group": "people" }));
    assert_eq!(sender(&session, "news@example.com").unwrap().group, Some(Group::People));
    assert_eq!(group_of(&mut session, news), json!("people"));
    arrive(&mut session, 1, vec![with(33, "news@example.com", &[("List-Id", "<news.example.com>")])]);
    let later = id(&session, 33);
    assert_eq!(group_of(&mut session, later), json!("people"), "future mail follows the sender");
    let screened = sender(&session, "news@example.com").unwrap();
    assert!(screened.known, "the screener's knowledge is kept beside the group");
    // `auto` hands the sender back to the classifier.
    let answer = session.run("group.set", &json!({ "address": "news@example.com", "group": "auto" })).unwrap();
    assert_eq!(answer["group"], Value::Null);
    assert_eq!(group_of(&mut session, news), json!("newsletters"));
}

#[test]
fn undo_puts_the_sender_back_in_its_group() {
    let mut session = grouped();
    let update = id(&session, 31);
    session.run("group.set", &json!({ "address": "no-reply@example.com", "group": "invitations" })).unwrap();
    session.run("group.set", &json!({ "address": "no-reply@example.com", "group": "newsletters" })).unwrap();
    assert_eq!(group_of(&mut session, update), json!("newsletters"));
    let undone = session.run("history.undo", &json!({})).unwrap();
    assert_eq!(undone, json!({ "command": "group.set", "address": "no-reply@example.com", "undone": true }));
    assert_eq!(group_of(&mut session, update), json!("invitations"));
    session.run("history.undo", &json!({})).unwrap();
    assert_eq!(group_of(&mut session, update), json!("updates"));
    let known = sender(&session, "no-reply@example.com").unwrap();
    assert_eq!(known.group, None, "the sender is as it was before the first choice");
}

#[test]
fn moving_a_sender_is_the_owners_and_refuses_what_does_not_fit() {
    let mut session = grouped();
    let refused = session.run_as(Caller::Agent, "group.set", &json!({ "address": "news@example.com", "group": "people" }));
    assert!(matches!(refused, Err(EngineError::NotAllowed { .. })));
    for params in [
        json!({ "address": "news@example.com", "group": "friends" }),
        json!({ "address": "not an address", "group": "people" }),
        json!({ "address": "news@example.com" }),
        json!({ "address": "news@example.com", "group": "people", "also": 1 }),
    ] {
        assert_eq!(session.run("group.set", &params).unwrap_err().code(), "bad_params", "{params}");
    }
}

#[test]
fn grouping_is_on_or_off_per_account_and_kept_with_the_account() {
    let mut session = grouped();
    let news = id(&session, 30);
    assert_eq!(session.run("group.account", &json!({ "account": 1, "grouped": false })).unwrap(), json!({ "account": 1, "grouped": false }));
    assert!(!session.store().accounts().unwrap().iter().find(|account| account.id() == AccountId::new(1)).unwrap().grouped());
    assert_eq!(group_of(&mut session, news), Value::Null);
    let ungrouped: Vec<u64> = rows(&mut session, &json!({ "group": "none" })).into_iter().map(|(id, _)| id).collect();
    assert!(ungrouped.contains(&news));
    assert!(rows(&mut session, &json!({ "group": "newsletters" })).is_empty());
    let mut people: Vec<u64> = rows(&mut session, &json!({ "group": "people" })).into_iter().map(|(id, _)| id).collect();
    people.sort_unstable();
    assert_eq!(people, [3, 4], "account 2 stays grouped");
    session.run("group.account", &json!({ "account": 1, "grouped": true })).unwrap();
    assert_eq!(group_of(&mut session, news), json!("newsletters"));
    let refused = session.run_as(Caller::Agent, "group.account", &json!({ "account": 1, "grouped": false }));
    assert!(matches!(refused, Err(EngineError::NotAllowed { .. })));
    assert_eq!(
        session.run("group.account", &json!({ "account": 9, "grouped": false })).unwrap_err(),
        EngineError::NotFound { what: "account", id: 9 }
    );
    assert_eq!(session.run("group.account", &json!({ "account": 1 })).unwrap_err().code(), "bad_params");
}

#[test]
fn account_list_says_whether_each_account_is_grouped() {
    let mut session = grouped();
    let listed = |session: &mut Session| -> Vec<(u64, bool)> {
        let answer = session.run("account.list", &json!({})).unwrap();
        answer["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|account| (account["id"].as_u64().unwrap(), account["grouped"].as_bool().unwrap()))
            .collect()
    };
    assert_eq!(listed(&mut session), [(1, true), (2, true)]);
    session.run("group.account", &json!({ "account": 1, "grouped": false })).unwrap();
    assert_eq!(listed(&mut session), [(1, false), (2, true)]);
}

#[test]
fn group_list_counts_each_group_of_the_inbox() {
    let mut session = grouped();
    let answer = session.run("group.list", &json!({})).unwrap();
    assert_eq!(
        answer,
        json!({
            "account": null,
            "groups": [
                { "group": "people", "total": 5, "unread": 5 },
                { "group": "updates", "total": 1, "unread": 1 },
                { "group": "newsletters", "total": 1, "unread": 1 },
                { "group": "invitations", "total": 0, "unread": 0 },
            ],
            "ungrouped": { "total": 0, "unread": 0 },
        })
    );
    session.run("group.account", &json!({ "account": 2, "grouped": false })).unwrap();
    let answer = session.run("group.list", &json!({ "account": 1 })).unwrap();
    assert_eq!(answer["account"], json!(1));
    assert_eq!(answer["groups"][0], json!({ "group": "people", "total": 3, "unread": 3 }));
    let answer = session.run("group.list", &json!({})).unwrap();
    assert_eq!(answer["ungrouped"], json!({ "total": 2, "unread": 2 }));
    assert_eq!(session.run_as(Caller::Agent, "group.list", &json!({})).unwrap()["groups"][1]["total"], json!(1), "reading is safe");
    assert_eq!(session.run("group.list", &json!({ "account": 9 })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
}

#[test]
fn an_invitation_found_in_a_kept_body_moves_to_invitations() {
    let mut session = grouped();
    let friend = id(&session, 32);
    let raw = b"From: friend@example.com\r\n\
        Content-Type: multipart/alternative; boundary=\"b\"\r\n\r\n\
        --b\r\n\
        Content-Type: text/plain\r\n\r\n\
        Dinner on Friday?\r\n\
        --b\r\n\
        Content-Type: text/calendar; method=REQUEST\r\n\r\n\
        BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n\
        --b--\r\n";
    let fetched = EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity(), uid: uid(32), raw: raw.to_vec() };
    session.apply(&Event::new(AccountId::new(1), None, fetched)).unwrap();
    assert!(session.store().body(inboxcraft_model::MessageId::new(friend)).unwrap().is_some());
    assert!(arrived(&session, 32).summary().envelope().signs().calendar());
    assert_eq!(group_of(&mut session, friend), json!("invitations"));
    // A body without one changes nothing.
    let news = id(&session, 30);
    let plain = EventKind::BodyFetched {
        folder: FolderId::new(1),
        uid_validity: validity(),
        uid: uid(30),
        raw: b"From: news@example.com\r\n\r\nHi\r\n".to_vec(),
    };
    session.apply(&Event::new(AccountId::new(1), None, plain)).unwrap();
    assert!(!arrived(&session, 30).summary().envelope().signs().calendar());
    assert_eq!(group_of(&mut session, news), json!("newsletters"));
}
