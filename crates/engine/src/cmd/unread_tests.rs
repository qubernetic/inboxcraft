// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! "Unread only": `mail.list` with `unread` lists the unread messages alone, but the one `keep` names, and counts
//! them so; `ui.list.unread` (`safe`) asks the window for it and changes nothing in the store.

use serde_json::{Value, json};

use crate::shown::{Asked, UnreadAsked};
use crate::test_support::session;
use crate::{AgentClass, Caller, EngineError, Session};

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

#[test]
fn unread_only_is_safe_and_asks_the_window_by_whoever_ran_it() {
    let mut session = session();
    assert_eq!(session.registry().get("ui.list.unread").unwrap().class, Some(AgentClass::Safe));
    for (params, on) in [(json!({}), None), (json!(null), None), (json!({ "on": true }), Some(true)), (json!({ "on": false }), Some(false))] {
        assert_eq!(session.run_as(Caller::Agent, "ui.list.unread", &params).unwrap(), json!({ "on": on, "window": false }), "{params}");
        assert_eq!(session.take_asked(), vec![Asked::Unread(UnreadAsked { on, by_agent: true })], "{params}");
    }
    session.set_window(true);
    session.run_as(Caller::Owner, "ui.list.unread", &json!({ "on": true })).unwrap();
    assert_eq!(session.take_asked(), vec![Asked::Unread(UnreadAsked { on: Some(true), by_agent: false })], "the owner's own");
}

#[test]
fn unread_only_changes_nothing_in_the_store() {
    let mut session = session();
    let before = session.run("mail.list", &json!({ "role": "inbox" })).unwrap();
    session.run_as(Caller::Agent, "ui.list.unread", &json!({ "on": true })).unwrap();
    assert_eq!(session.run("mail.list", &json!({ "role": "inbox" })).unwrap(), before);
}

#[test]
fn unread_only_refuses_other_parameters_without_echoing_them() {
    let mut session = session();
    let hostile = "\u{202e}\r\n\u{0}evil";
    for params in [json!({ hostile: 1 }), json!({ "on": "yes" }), json!({ "on": 1 }), json!([hostile]), json!(hostile), json!(1)] {
        let error = session.run_as(Caller::Agent, "ui.list.unread", &params).unwrap_err();
        assert!(matches!(error, EngineError::BadParams { .. }), "{params}: {error:?}");
        assert!(!error.to_string().contains("evil"), "the junk is not echoed: {error}");
    }
    assert!(session.take_asked().is_empty(), "a refused call asks for nothing");
}

#[test]
fn unread_crosses_one_folder_and_the_group_filter() {
    let mut session = session();
    read(&mut session, 1);
    let folder = session.run("mail.list", &json!({ "folder": 1, "unread": true })).unwrap();
    assert_eq!(ids(&folder), vec![2], "INBOX of account 1 holds 2 and 1; 1 is read");
    assert_eq!(folder["total"], json!(1));
    // The group of message 2, as the list says it: `unread` cuts the group's list as it cuts the whole.
    let all = session.run("mail.list", &json!({ "role": "inbox" })).unwrap();
    let group = |id: u64| all["messages"].as_array().unwrap().iter().find(|row| row["id"] == json!(id)).unwrap()["group"].clone();
    let name = group(2).as_str().unwrap_or("none").to_owned();
    let same: Vec<u64> = ids(&all).into_iter().filter(|id| group(*id) == group(2)).collect();
    assert!(same.contains(&1), "1 is in 2's group too: {same:?}");
    let grouped = session.run("mail.list", &json!({ "role": "inbox", "group": name, "unread": true })).unwrap();
    assert_eq!(ids(&grouped), same.into_iter().filter(|id| *id != 1).collect::<Vec<_>>());
}

#[test]
fn unread_crosses_show_done_and_the_park_pile() {
    let mut session = crate::cmd::act_tests::session();
    // Account 1 has an archive, where done files a message.
    let first = ids(&session.run("mail.list", &json!({ "role": "inbox", "account": 1 })).unwrap());
    let [unread_done, read_done, ..] = first[..] else { panic!("two messages in account 1's inbox: {first:?}") };
    let second = ids(&session.run("mail.list", &json!({ "role": "inbox", "account": 2 })).unwrap());
    let Some(&parked) = second.first() else { panic!("a message in account 2's inbox") };
    // Two done messages, one read and one unread.
    session.run("mail.done", &json!({ "id": unread_done, "done": true })).unwrap();
    session.run("mail.seen", &json!({ "id": unread_done, "seen": false })).unwrap();
    session.run("mail.done", &json!({ "id": read_done, "done": true })).unwrap();
    read(&mut session, read_done);
    let done_rows = ids(&session.run("mail.list", &json!({ "role": "inbox", "done": true })).unwrap());
    assert!(done_rows.contains(&unread_done) && done_rows.contains(&read_done), "show done lists both: {done_rows:?}");
    let shown = ids(&session.run("mail.list", &json!({ "role": "inbox", "done": true, "unread": true })).unwrap());
    assert!(shown.contains(&unread_done), "a done unread message is listed with show done: {shown:?}");
    assert!(!shown.contains(&read_done), "a done read one is not: {shown:?}");
    session.run("mail.park", &json!({ "id": parked, "parked": true })).unwrap();
    let pile = session.run("mail.list", &json!({ "role": "inbox", "parked": true, "unread": true })).unwrap();
    assert!(ids(&pile).contains(&parked), "an unread parked message is listed with the pile");
    read(&mut session, parked);
    let pile = session.run("mail.list", &json!({ "role": "inbox", "parked": true, "unread": true })).unwrap();
    assert!(!ids(&pile).contains(&parked), "read, it is not");
    let off = session.run("mail.list", &json!({ "role": "inbox", "unread": true })).unwrap();
    assert_eq!(off["parked"], json!(1), "the pile's count counts it, read or not");
}
