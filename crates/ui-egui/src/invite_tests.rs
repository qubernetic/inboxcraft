// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invitation card's data: `invite.show` read into fields, and the lines the card says, on invented events.

use serde_json::{Value, json};

use super::{Invite, Method, Rsvp};

/// What `invite.show` answers for the update of a lunch moved from 11:00 to 12:30, answered Going before.
fn update() -> Value {
    json!({ "invite": {
        "message": 7,
        "method": "request",
        "summary": "Lunch with Odile",
        "location": "The Gull café",
        "local_start": "2026-11-07T12:30:00+01:00",
        "local_end": "2026-11-07T13:30:00+01:00",
        "all_day": false,
        "organizer": { "address": "odile@example.org", "name": "Odile" },
        "answer": null,
        "earlier_answer": "accepted",
        "earlier": {
            "message": 6,
            "summary": "Lunch with Odile",
            "location": "The Gull café",
            "local_start": "2026-11-07T11:00:00+01:00",
            "local_end": "2026-11-07T12:00:00+01:00",
            "all_day": false,
            "date": "2026-11-02T08:15:00+01:00",
        },
        "changes": ["time"],
        "outdated": false,
        "calendar_event": 3,
        "can_answer": true,
        "why_not": null,
        "can_remove": false,
        "replies": [],
    }})
}

#[test]
fn a_message_without_an_invitation_has_no_card() {
    assert_eq!(Invite::read(1, &json!({ "invite": null })), None);
    assert_eq!(Invite::read(1, &json!({})), None);
}

#[test]
fn the_card_reads_the_event_its_answer_and_what_changed() {
    let invite = Invite::read(7, &update()).unwrap();
    assert_eq!((invite.message, invite.method, invite.answer, invite.earlier_answer), (7, Method::Request, None, Some(Rsvp::Going)));
    assert_eq!((invite.organizer.as_deref(), invite.calendar_event, invite.can_answer), (Some("Odile"), Some(3), true));
    assert_eq!((invite.month(), invite.day()), ("NOV".to_owned(), "7".to_owned()));
    assert_eq!(invite.line(), "Saturday 7 November · 12:30 – 13:30 · The Gull café");
    assert_eq!(invite.changed().as_deref(), Some("Changed: time 11:00 → 12:30"));
    assert_eq!(invite.earlier_line().as_deref(), Some("Out of date: the invitation from Monday showed 11:00. You answered Going."));
    assert_eq!(invite.goes_to().as_deref(), Some("Your answer goes to odile@example.org at once."));
    assert_eq!(invite.answered(), None);
}

#[test]
fn changes_name_what_moved() {
    let mut answer = update();
    answer["invite"]["changes"] = json!(["time", "place", "title", "repeat", "something"]);
    answer["invite"]["location"] = json!("Harbour Kitchen");
    answer["invite"]["local_start"] = json!("2026-11-08T12:30:00+01:00");
    answer["invite"]["local_end"] = json!("2026-11-08T13:30:00+01:00");
    let invite = Invite::read(7, &answer).unwrap();
    assert_eq!(
        invite.changed().as_deref(),
        Some("Changed: time Saturday 7 November 11:00 → Sunday 8 November 12:30, place The Gull café → Harbour Kitchen, title, repeat, something")
    );
    // Without the version before, the change is only named.
    answer["invite"]["earlier"] = Value::Null;
    answer["invite"]["changes"] = json!(["time", "place"]);
    let invite = Invite::read(7, &answer).unwrap();
    assert_eq!(invite.changed().as_deref(), Some("Changed: time, place"));
    assert_eq!(invite.earlier_line(), None);
    answer["invite"]["changes"] = json!([]);
    assert_eq!(Invite::read(7, &answer).unwrap().changed(), None);
}

#[test]
fn whole_days_and_open_ends_read_as_dates() {
    let mut answer = update();
    answer["invite"]["all_day"] = json!(true);
    answer["invite"]["location"] = json!("");
    answer["invite"]["local_start"] = json!("2026-11-07T00:00:00+01:00");
    answer["invite"]["local_end"] = json!("2026-11-08T00:00:00+01:00");
    assert_eq!(Invite::read(7, &answer).unwrap().line(), "Saturday 7 November");
    answer["invite"]["local_end"] = json!("2026-11-10T00:00:00+01:00");
    assert_eq!(Invite::read(7, &answer).unwrap().line(), "Saturday 7 November – Monday 9 November");
    answer["invite"]["all_day"] = json!(false);
    answer["invite"]["local_end"] = json!("2026-11-08T09:00:00+01:00");
    assert_eq!(Invite::read(7, &answer).unwrap().line(), "Saturday 7 November 00:00 – Sunday 8 November 09:00");
    answer["invite"]["local_end"] = Value::Null;
    assert_eq!(Invite::read(7, &answer).unwrap().line(), "Saturday 7 November · 00:00");
    answer["invite"]["local_start"] = Value::Null;
    answer["invite"]["location"] = json!("The Gull café");
    let invite = Invite::read(7, &answer).unwrap();
    assert_eq!((invite.line(), invite.month(), invite.day()), ("The Gull café".to_owned(), String::new(), String::new()));
    // A version before whose message came on a day not known says so plainly.
    answer["invite"]["earlier"]["date"] = Value::Null;
    answer["invite"]["earlier_answer"] = Value::Null;
    assert_eq!(
        Invite::read(7, &answer).unwrap().earlier_line().as_deref(),
        Some("Out of date: the invitation before showed Saturday 7 November 11:00.")
    );
}

#[test]
fn a_cancel_and_a_reply_read_their_own_parts() {
    let cancel = json!({ "invite": { "method": "cancel", "can_remove": true, "calendar_event": 3, "organizer": { "address": "odile@example.org", "name": null } } });
    let invite = Invite::read(8, &cancel).unwrap();
    assert_eq!((invite.method, invite.can_remove, invite.organizer.as_deref()), (Method::Cancel, true, Some("odile@example.org")));
    let reply = json!({ "invite": { "method": "reply", "replies": [
        { "address": "bela@example.org", "name": "Béla", "status": "declined" },
        { "address": "cili@example.org", "name": null, "status": "accepted" },
        { "address": "dora@example.org", "name": "Dóra", "status": "tentative" },
        { "address": "ede@example.org", "name": "", "status": "needs_action" },
    ] } });
    let invite = Invite::read(9, &reply).unwrap();
    assert_eq!(invite.method, Method::Reply);
    let lines: Vec<String> = invite.replies.iter().map(Invite::reply_line).collect();
    assert_eq!(lines, ["Béla is not going.", "cili@example.org is going.", "Dóra might go.", "ede@example.org has not answered."]);
    assert_eq!(Invite::read(9, &json!({ "invite": { "method": "publish" } })).unwrap().method, Method::Other);
    assert_eq!(
        Invite::read(9, &json!({ "invite": { "method": "request", "answer": "tentative" } })).unwrap().answered().as_deref(),
        Some("You answered Maybe.")
    );
}

/// The organiser's text is hostile: what the card shows of it has no line breaks or invisible marks.
#[test]
fn hostile_text_is_shown_flat() {
    let mut answer = update();
    answer["invite"]["summary"] = json!("Lunch\r\nwith\u{202e}Odile");
    let invite = Invite::read(7, &answer).unwrap();
    assert!(!invite.event.summary.contains(['\r', '\n', '\u{202e}']), "{:?}", invite.event.summary);
}

#[test]
fn answers_name_themselves_both_ways() {
    for answer in Rsvp::ALL {
        assert_eq!(Rsvp::named(answer.id()), Some(answer));
    }
    assert_eq!(Rsvp::named("needs_action"), None);
    assert_eq!(Rsvp::ALL.map(Rsvp::text), ["Going", "Maybe", "No"]);
}

#[test]
fn a_body_that_only_says_what_the_card_says_is_a_restatement() {
    let invite = Invite::read(7, &update()).unwrap();
    assert!(invite.restates("Odile invites you to Lunch with Odile at The Gull café."));
    assert!(invite.restates(""));
    assert!(!invite.restates("Lunch with Odile at The Gull café. Bring the keys to the shed, please."), "their own words stay on screen");
    assert!(!invite.restates(&"Lunch ".repeat(1000)), "too long to be a generated line");
}
