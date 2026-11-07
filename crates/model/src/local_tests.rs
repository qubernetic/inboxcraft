// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::Timestamp;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{Alert, Calendar, CalendarEvent, Draft, DraftCopy, Due, Group, MessageState, OutboxState, Outgoing, Scheduled, Screening, Sender};
use crate::{AccountId, AlertId, CalendarEventId, CalendarId, DraftId, EmailAddress, FolderId, MessageId, ScheduleId, Uid, UidValidity};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(seconds, 0).unwrap()
}

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
    let text = serde_json::to_string(value).unwrap();
    assert_eq!(&serde_json::from_str::<T>(&text).unwrap(), value, "{text}");
}

fn draft() -> Draft {
    Draft {
        to: vec!["bela@example.org".to_owned(), "not an address".to_owned()],
        cc: vec!["cili@example.net".to_owned()],
        bcc: vec!["dora@example.org".to_owned()],
        subject: "Budget 2026".to_owned(),
        text: "The draft budget is attached.".to_owned(),
        answers: Some(MessageId::new(4)),
        uploaded: Some(DraftCopy { folder: FolderId::new(6), uid_validity: UidValidity::new(9).unwrap(), uid: Uid::new(17).unwrap() }),
        synced: Some(at(900)),
        unnamed: true,
        ..Draft::new(DraftId::new(3), AccountId::new(1), at(1_000))
    }
}

#[test]
fn a_new_draft_is_empty() {
    let draft = Draft::new(DraftId::new(3), AccountId::new(1), at(1_000));
    assert_eq!((draft.id, draft.account, draft.saved), (DraftId::new(3), AccountId::new(1), at(1_000)));
    assert!(draft.to.is_empty() && draft.cc.is_empty() && draft.bcc.is_empty());
    assert!(draft.subject.is_empty() && draft.text.is_empty());
    assert_eq!((draft.answers, draft.uploaded, draft.synced, draft.unnamed), (None, None, None, false));
}

#[test]
fn a_draft_record_without_the_newer_fields_reads_back() {
    let draft: Draft = serde_json::from_str(r#"{"id":3,"account":1,"saved":[1000,0]}"#).unwrap();
    assert_eq!(draft, Draft::new(DraftId::new(3), AccountId::new(1), at(1_000)));
}

#[test]
fn a_draft_debug_shows_no_words() {
    let shown = format!("{:?}", draft());
    assert!(!shown.contains("bela") && !shown.contains("Budget") && !shown.contains("attached"), "{shown}");
    assert!(shown.contains("synced"), "{shown}");
}

#[test]
fn every_record_reads_back() {
    round_trip(&draft());
    round_trip(&Draft::new(DraftId::new(3), AccountId::new(1), at(-5)));
    for state in [OutboxState::Waiting, OutboxState::Sending, OutboxState::Refused { reply: "550 5.1.1 no such user".to_owned() }] {
        round_trip(&Outgoing::new(DraftId::new(3), state.clone()));
        round_trip(&Outgoing { message: b"Subject: Hi\r\n\r\nHello\r\n".to_vec(), ..Outgoing::new(DraftId::new(3), state) });
    }
    for what in [Due::Send(DraftId::new(3)), Due::Unsnooze(MessageId::new(4)), Due::FollowUp(MessageId::new(5)), Due::Alert(AlertId::new(6))] {
        round_trip(&Scheduled { id: ScheduleId::new(1), due: at(2_000), what });
    }
    let mut sender = Sender::new(EmailAddress::new("Anna@Example.org").unwrap());
    round_trip(&sender);
    sender.known = true;
    for (screening, group) in [(Screening::Accepted, Group::People), (Screening::Blocked, Group::Updates)] {
        sender.screening = Some(screening);
        sender.group = Some(group);
        round_trip(&sender);
    }
    sender.group = Some(Group::Newsletters);
    round_trip(&sender);
    sender.group = Some(Group::Invitations);
    round_trip(&sender);
    round_trip(&MessageState::default());
    round_trip(&MessageState {
        done: Some(FolderId::new(1)),
        parked: true,
        pinned: true,
        snoozed_until: Some(at(3_000)),
        follow_up: Some(at(4_000)),
        returned: Some(at(2_000)),
    });
    round_trip(&Calendar {
        id: CalendarId::new(2),
        account: AccountId::new(1),
        href: "/dav/calendars/anna/home/".to_owned(),
        name: "Home".to_owned(),
        colour: Some("#3366ff".to_owned()),
    });
    round_trip(&CalendarEvent {
        id: CalendarEventId::new(7),
        calendar: CalendarId::new(2),
        href: "/dav/calendars/anna/home/a.ics".to_owned(),
        etag: Some("\"1\"".to_owned()),
        ical: "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".to_owned(),
    });
    round_trip(&Alert { id: AlertId::new(6), event: CalendarEventId::new(7), at: at(5_000), dismissed: true });
}

/// A record written by this build reads back in a later one that added fields, and the other way round: what a
/// record leaves out takes its default, so the tables need no migration for a new field.
#[test]
fn left_out_fields_take_their_defaults() {
    let draft: Draft = serde_json::from_str(r#"{"id": 3, "account": 1, "saved": [1000, 0], "later": true}"#).unwrap();
    assert_eq!(draft, Draft::new(DraftId::new(3), AccountId::new(1), at(1_000)));
    let sender: Sender = serde_json::from_str(r#"{"address": "anna@example.org"}"#).unwrap();
    assert_eq!(sender, Sender::new(EmailAddress::new("anna@example.org").unwrap()));
    let state: MessageState = serde_json::from_str("{}").unwrap();
    assert!(state.is_empty());
    let calendar: Calendar = serde_json::from_str(r#"{"id": 2, "account": 1, "href": "/c/", "name": "Home"}"#).unwrap();
    assert_eq!(calendar.colour, None);
    let event: CalendarEvent = serde_json::from_str(r#"{"id": 7, "calendar": 2, "href": "/c/a.ics", "ical": ""}"#).unwrap();
    assert_eq!(event.etag, None);
    let outgoing: Outgoing = serde_json::from_str(r#"{"draft": 3, "state": "Sending"}"#).unwrap();
    assert_eq!(outgoing, Outgoing::new(DraftId::new(3), OutboxState::Sending));
    let alert: Alert = serde_json::from_str(r#"{"id": 6, "event": 7, "at": [5000, 0]}"#).unwrap();
    assert!(!alert.dismissed);
}

#[test]
fn a_time_out_of_range_does_not_read_back() {
    assert!(serde_json::from_str::<Draft>(r#"{"id": 3, "account": 1, "saved": [1000, 2000000000]}"#).is_err());
    assert!(serde_json::from_str::<Alert>(r#"{"id": 6, "event": 7, "at": [5000, 2000000000]}"#).is_err());
    assert!(serde_json::from_str::<MessageState>(r#"{"snoozed_until": [1, 2000000000]}"#).is_err());
}

#[test]
fn a_message_state_is_empty_only_with_nothing_set() {
    assert!(MessageState::default().is_empty());
    assert!(!MessageState { pinned: true, ..MessageState::default() }.is_empty());
    assert!(!MessageState { done: Some(FolderId::new(1)), ..MessageState::default() }.is_empty());
}

/// Mail is private: a log or a panic message shows the shape of a draft or an event, never its words.
#[test]
fn debug_shows_no_mail_text() {
    let shown = format!("{:?}", draft());
    for private in ["bela", "cili", "dora", "Budget", "attached"] {
        assert!(!shown.contains(private), "{private} in {shown}");
    }
    assert!(shown.contains("d3") || shown.contains("DraftId(3)"), "{shown}");
    assert!(shown.contains("text_bytes: 29"), "{shown}");
    let event = CalendarEvent {
        id: CalendarEventId::new(7),
        calendar: CalendarId::new(2),
        href: "/c/a.ics".to_owned(),
        etag: None,
        ical: "SUMMARY:Dentist".to_owned(),
    };
    let shown = format!("{event:?}");
    assert!(!shown.contains("Dentist") && shown.contains("ical_bytes: 15"), "{shown}");
    let outgoing = Outgoing { message: b"Subject: Budget\r\n\r\nSecret".to_vec(), ..Outgoing::new(DraftId::new(3), OutboxState::Sending) };
    let shown = format!("{outgoing:?}");
    assert!(!shown.contains("Budget") && shown.contains("message_bytes: 25"), "{shown}");
}
