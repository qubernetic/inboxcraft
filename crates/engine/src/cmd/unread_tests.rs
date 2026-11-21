// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! "Unread only": `mail.list` with `unread` lists the unread messages alone, but the one `keep` names, and counts
//! them so.

use serde_json::{Value, json};

use crate::test_support::session;
use crate::{EngineError, Session};

fn ids(answer: &Value) -> Vec<u64> {
    answer["messages"].as_array().unwrap().iter().map(|row| row["id"].as_u64().unwrap()).collect()
}

fn read(session: &mut Session, id: u64) {
    session.run("mail.seen", &json!({ "id": id, "seen": true })).unwrap();
}

#[test]
fn unread_lists_the_unread_messages_alone_and_counts_them() {
    let mut session = session();
    let all = session.run("mail.list", &json!({ "role": "inbox" })).unwrap();
    assert_eq!(ids(&all), vec![2, 3, 1, 4], "every message of the inboxes, newest first");
    read(&mut session, 3);
    let unread = session.run("mail.list", &json!({ "role": "inbox", "unread": true })).unwrap();
    assert_eq!(ids(&unread), vec![2, 1, 4], "the read one is left out");
    assert_eq!(unread["total"], json!(3), "the total counts what is listed");
    let off = session.run("mail.list", &json!({ "role": "inbox", "unread": false })).unwrap();
    assert_eq!(ids(&off), vec![2, 3, 1, 4], "off, every message again");
}

#[test]
fn unread_pages_through_the_unread_messages() {
    let mut session = session();
    read(&mut session, 2);
    let page = session.run("mail.list", &json!({ "role": "inbox", "unread": true, "offset": 1, "limit": 1 })).unwrap();
    assert_eq!(ids(&page), vec![1], "the second unread one, not the second message");
    assert_eq!(page["total"], json!(3));
}

#[test]
fn keep_holds_one_read_message_in_its_place_and_adds_none() {
    let mut session = session();
    read(&mut session, 3);
    read(&mut session, 1);
    let kept = session.run("mail.list", &json!({ "role": "inbox", "unread": true, "keep": 3 })).unwrap();
    assert_eq!(ids(&kept), vec![2, 3, 4], "the kept one stays where it stood");
    // Message 5 is in Sent: keep never brings in a message the place does not hold.
    let elsewhere = session.run("mail.list", &json!({ "role": "inbox", "unread": true, "keep": 5 })).unwrap();
    assert_eq!(ids(&elsewhere), vec![2, 4]);
    for junk in [json!(u64::MAX), json!(0)] {
        let answer = session.run("mail.list", &json!({ "role": "inbox", "unread": true, "keep": junk })).unwrap();
        assert_eq!(ids(&answer), vec![2, 4], "{junk}");
    }
}

#[test]
fn unread_and_keep_refuse_what_is_not_their_kind() {
    let mut session = session();
    for params in [json!({ "unread": "yes" }), json!({ "unread": 1 }), json!({ "keep": -1 }), json!({ "keep": "3" })] {
        let error = session.run("mail.list", &params).unwrap_err();
        assert!(matches!(error, EngineError::BadParams { .. }), "{params}: {error:?}");
    }
}
