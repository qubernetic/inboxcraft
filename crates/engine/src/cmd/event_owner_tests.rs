// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Whose event it is: `event.list` names an event's organiser and says whether the event is the owner's own (no
//! organiser, or the account's own address) or someone else's, which the editors open read-only.

use inboxcraft_model::{AccountId, CalendarId, CalendarReport, Event, EventKind, FetchedEvent};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use super::calendar_tests::{WORK, object, session_at};
use crate::Session;
use crate::test_support::{NOW, at};

fn session() -> Session {
    session_at(&FakeClock::new(at(NOW)))
}

/// The row of the event called `summary` in the week from Thursday 5 November 2026.
fn row(session: &mut Session, summary: &str) -> Value {
    let list = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12" })).unwrap();
    list["events"].as_array().unwrap().iter().find(|row| row["summary"] == json!(summary)).unwrap().clone()
}

/// A meeting on Friday 6 November 2026 in account 1's Work calendar, with `organizer` as its `ORGANIZER` line.
fn synced(session: &mut Session, uid: &str, organizer: &str) {
    let ical = object(&format!(
        "BEGIN:VEVENT\r\nUID:{uid}@example.org\r\nDTSTART:20261106T140000Z\r\nDTEND:20261106T150000Z\r\nSUMMARY:{uid}\r\n\
         {organizer}\r\nATTENDEE;CN=Anna:mailto:anna@example.org\r\nEND:VEVENT\r\n"
    ));
    let fetched = FetchedEvent { url: format!("{WORK}{uid}.ics"), etag: Some("\"k1\"".to_owned()), ical };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
}

#[test]
fn an_event_someone_else_organises_is_not_the_owners_own() {
    let mut session = session();
    synced(&mut session, "Review", "ORGANIZER;CN=Teodor Lisk:mailto:teodor@example.org");
    let row = row(&mut session, "Review");
    assert_eq!(row["own"], json!(false), "the organiser is not the account's owner");
    assert_eq!(row["organizer"], json!({ "address": "teodor@example.org", "name": "Teodor Lisk" }));
}

#[test]
fn an_event_the_owner_organises_or_that_names_no_organiser_is_their_own() {
    let mut session = session();
    synced(&mut session, "Planning", "ORGANIZER:MAILTO:Anna@Example.ORG");
    let planning = row(&mut session, "Planning");
    assert_eq!(planning["own"], json!(true), "the owner's address in any letter case is the owner");
    assert_eq!(planning["organizer"]["address"], json!("Anna@Example.ORG"));
    let lunch = row(&mut session, "Lunch");
    assert_eq!(lunch["own"], json!(true), "lunch is organised by anna@example.org, the account's own address");
    let days_off = row(&mut session, "Days off");
    assert_eq!(days_off["own"], json!(true), "an event without an organiser is the owner's");
    assert_eq!(days_off["organizer"], Value::Null);
}

#[test]
fn a_new_event_is_the_owners_own() {
    let mut session = session();
    session.run("event.create", &json!({ "calendar": 1, "summary": "Kiln", "start": "2026-11-06T14:00:00Z" })).unwrap();
    assert_eq!(row(&mut session, "Kiln")["own"], json!(true));
}
