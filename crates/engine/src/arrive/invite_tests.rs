// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Invitations in mail: the calendar part of a kept body read into the message's invitation mark, with invented
//! events of `example.org`.

use inboxcraft_model::{AccountId, Envelope, Event, EventKind, FolderId, Invitation, ItipMethod, MessageId, Rsvp, Signs, Summary};
use inboxcraft_store::Change;
use inboxcraft_time::{Offset, Timestamp};

use super::{MAX_TEXT, event, later, mark, method, read, same_event};
use crate::Session;
use crate::arrive::screener::tests::{arrive, arrived};
use crate::cmd::act_tests::{session as act_session, uid, validity};
use crate::test_support::{address, change};

/// An invitation from Odile to Anna: `method`, the event `uid` at `sequence`, starting at `start` (`20261107T113000Z`)
/// for an hour, with `extra` lines in the event.
pub(crate) fn object(method: &str, uid: &str, sequence: u32, start: &str, extra: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\nMETHOD:{method}\r\nBEGIN:VEVENT\r\nUID:{uid}\r\n\
         SEQUENCE:{sequence}\r\nDTSTAMP:20261102T080000Z\r\nDTSTART:{start}\r\nDURATION:PT1H\r\nSUMMARY:Lunch with Odile\r\n\
         LOCATION:The Gull café\r\nORGANIZER;CN=Odile:mailto:odile@example.org\r\n\
         ATTENDEE;CN=Anna;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:anna@example.org\r\n\
         ATTENDEE;CN=Béla;PARTSTAT=ACCEPTED:mailto:bela@example.org\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

/// A message carrying `calendar` as iMIP does: text and the calendar object as `multipart/alternative`.
pub(crate) fn mail(subject: &str, message_id: &str, calendar: &str) -> Vec<u8> {
    let method = calendar.lines().find_map(|line| line.strip_prefix("METHOD:")).unwrap_or("REQUEST").trim();
    format!(
        "From: Odile <odile@example.org>\r\nTo: anna@example.org\r\nSubject: {subject}\r\nMessage-ID: <{message_id}>\r\n\
         MIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\n\
         Odile invites you.\r\n--b\r\nContent-Type: text/calendar; charset=utf-8; method={method}\r\n\
         Content-Transfer-Encoding: 8bit\r\n\r\n{calendar}--b--\r\n"
    )
    .into_bytes()
}

/// The summary of an invitation with UID `n`, as the sync reads its header.
pub(crate) fn summary(n: u32, subject: &str, message_id: &str) -> Summary {
    let envelope = Envelope::new(subject)
        .with_from(vec![address("Odile", "odile@example.org")])
        .with_to(vec![address("Anna", "anna@example.org")])
        .with_message_id(inboxcraft_model::MessageRef::new(message_id))
        .with_signs(Signs::read([("Content-Type", "multipart/alternative; boundary=b")]));
    Summary::new(uid(n), envelope)
}

fn fetched(session: &mut Session, folder: u64, n: u32, raw: Vec<u8>) {
    let kind = EventKind::BodyFetched { folder: FolderId::new(folder), uid_validity: validity(), uid: uid(n), raw };
    session.apply(&Event::new(AccountId::new(1), None, kind)).unwrap();
}

fn invitation(session: &Session, id: MessageId) -> Option<Invitation> {
    session.store().message_state(id).unwrap().invitation.map(|invitation| *invitation)
}

fn at(text: &str) -> Timestamp {
    crate::params::parse_date(text).unwrap()
}

#[test]
fn the_calendar_part_of_a_message_is_read_wherever_it_is() {
    let calendar = object("REQUEST", "lunch@example.org", 0, "20261107T113000Z", "");
    let read_one = read(&mail("Invitation", "i1@example.org", &calendar)).unwrap();
    assert_eq!((method(&read_one), event(&read_one).map(|event| event.uid.as_str())), (ItipMethod::Request, Some("lunch@example.org")));
    // A message that is itself the calendar object.
    let whole = format!(
        "From: odile@example.org\r\nContent-Type: text/calendar; method=CANCEL\r\n\r\n{}",
        object("CANCEL", "a@example.org", 1, "20261107T113000Z", "")
    );
    assert_eq!(read(whole.as_bytes()).as_ref().map(method), Some(ItipMethod::Cancel));
    // Inside an attached message it is that message's, not this one's.
    let forwarded = format!(
        "Content-Type: multipart/mixed; boundary=m\r\n\r\n--m\r\nContent-Type: text/plain\r\n\r\nSee below.\r\n--m\r\nContent-Type: message/rfc822\r\n\r\n{}\r\n--m--\r\n",
        String::from_utf8(mail("Invitation", "i1@example.org", &calendar)).unwrap()
    );
    assert!(read(forwarded.as_bytes()).is_none());
    // No calendar part, a part that cannot be read, a calendar without an event, a message that is no message.
    assert!(read(b"Subject: Hi\r\n\r\nHello\r\n").is_none());
    assert!(read(&mail("Invitation", "i1@example.org", "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\n")).is_none());
    let empty = read(&mail("Invitation", "i1@example.org", "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n")).unwrap();
    assert!(event(&empty).is_none());
    assert_eq!(method(&empty), ItipMethod::Other);
    assert!(mark(&empty, &Offset::UTC).is_none());
}

#[test]
fn every_method_reads_and_any_other_is_other() {
    for (name, expected) in [
        ("REQUEST", ItipMethod::Request),
        ("request", ItipMethod::Request),
        ("CANCEL", ItipMethod::Cancel),
        ("REPLY", ItipMethod::Reply),
        ("PUBLISH", ItipMethod::Other),
        ("COUNTER", ItipMethod::Other),
    ] {
        let calendar = read(&mail("x", "i1@example.org", &object(name, "a@example.org", 0, "20261107T113000Z", ""))).unwrap();
        assert_eq!(method(&calendar), expected, "{name}");
    }
}

#[test]
fn the_mark_holds_the_event_by_uid_and_sequence_with_what_the_owner_sees() {
    let calendar = object("REQUEST", "lunch@example.org", 3, "20261107T113000Z", "RRULE:FREQ=WEEKLY;COUNT=4\r\n");
    let marked = mark(&read(&mail("Invitation", "i1@example.org", &calendar)).unwrap(), &Offset::UTC).unwrap();
    assert_eq!(
        marked,
        Invitation {
            sequence: 3,
            stamp: Some(at("2026-11-02T08:00:00Z")),
            summary: Some("Lunch with Odile".to_owned()),
            start: Some(at("2026-11-07T11:30:00Z")),
            end: Some(at("2026-11-07T12:30:00Z")),
            repeat: Some("FREQ=WEEKLY;COUNT=4".to_owned()),
            location: Some("The Gull café".to_owned()),
            ..Invitation::new(ItipMethod::Request, "lunch@example.org")
        }
    );
    // A whole day ends the day after when it names no end; one occurrence is named by its RECURRENCE-ID.
    let day = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:day@example.org\r\nRECURRENCE-ID;VALUE=DATE:20261109\r\n\
               DTSTART;VALUE=DATE:20261109\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let marked = mark(&read(&mail("Day", "i2@example.org", day)).unwrap(), &Offset::UTC).unwrap();
    assert!(marked.all_day);
    assert_eq!((marked.start, marked.end), (Some(at("2026-11-09T00:00:00Z")), Some(at("2026-11-10T00:00:00Z"))));
    assert_eq!(marked.recurrence.as_deref(), Some("20261109"));
    // An end given, and a moment without length.
    let ends = object("REQUEST", "e@example.org", 0, "20261107T113000Z", "").replace("DURATION:PT1H\r\n", "DTEND:20261107T140000Z\r\n");
    assert_eq!(mark(&read(&mail("x", "i3@example.org", &ends)).unwrap(), &Offset::UTC).unwrap().end, Some(at("2026-11-07T14:00:00Z")));
    let moment = object("REQUEST", "m@example.org", 0, "20261107T113000Z", "").replace("DURATION:PT1H\r\n", "");
    assert_eq!(mark(&read(&mail("x", "i4@example.org", &moment)).unwrap(), &Offset::UTC).unwrap().end, Some(at("2026-11-07T11:30:00Z")));
}

/// Mail is hostile: a title or a place too long is cut, a UID too long makes no mark.
#[test]
fn hostile_text_is_cut_and_a_huge_uid_marks_nothing() {
    let long = "x".repeat(MAX_TEXT + 50);
    let calendar = object("REQUEST", "a@example.org", 0, "20261107T113000Z", "").replace("Lunch with Odile", &long);
    let marked = mark(&read(&mail("x", "i1@example.org", &calendar)).unwrap(), &Offset::UTC).unwrap();
    assert_eq!(marked.summary.map(|text| text.chars().count()), Some(MAX_TEXT));
    let calendar = object("REQUEST", &"u".repeat(2000), 0, "20261107T113000Z", "");
    assert!(mark(&read(&mail("x", "i1@example.org", &calendar)).unwrap(), &Offset::UTC).is_none());
    let calendar = object("REQUEST", "", 0, "20261107T113000Z", "");
    assert!(mark(&read(&mail("x", "i1@example.org", &calendar)).unwrap(), &Offset::UTC).is_none());
}

#[test]
fn versions_of_one_event_are_ordered_by_sequence_then_stamp() {
    let one = Invitation { sequence: 1, stamp: Some(at("2026-11-02T08:00:00Z")), ..Invitation::new(ItipMethod::Request, "a@example.org") };
    let restamped = Invitation { stamp: Some(at("2026-11-03T08:00:00Z")), ..one.clone() };
    let two = Invitation { sequence: 2, stamp: None, ..one.clone() };
    assert!(later(&two, &restamped) && later(&restamped, &one) && !later(&one, &one) && !later(&one, &two));
    assert!(same_event(&one, &two));
    assert!(!same_event(&one, &Invitation::new(ItipMethod::Request, "b@example.org")));
    assert!(!same_event(&one, &Invitation { recurrence: Some("20261109".to_owned()), ..one.clone() }));
}

#[test]
fn a_kept_body_marks_its_message_and_an_answer_to_the_same_version_stays() {
    let mut session = act_session();
    arrive(&mut session, 1, vec![summary(40, "Invitation: Lunch with Odile", "i1@example.org")]);
    let id = arrived(&session, 40).id();
    assert_eq!(invitation(&session, id), None);
    let calendar = object("REQUEST", "lunch@example.org", 0, "20261107T113000Z", "");
    fetched(&mut session, 1, 40, mail("Invitation: Lunch with Odile", "i1@example.org", &calendar));
    let marked = invitation(&session, id).unwrap();
    assert_eq!((marked.method, marked.uid.as_str(), marked.answer), (ItipMethod::Request, "lunch@example.org", None));
    // It is in the Invitations group now.
    assert!(arrived(&session, 40).summary().envelope().signs().calendar());
    // The owner answered; the body is fetched again (it was pruned) and the answer stays.
    let mut state = session.store().message_state(id).unwrap();
    state.invitation = Some(Box::new(Invitation { answer: Some(Rsvp::Accepted), ..marked }));
    change(&mut session, Change::SetMessageState(id, state));
    fetched(&mut session, 1, 40, mail("Invitation: Lunch with Odile", "i1@example.org", &calendar));
    assert_eq!(invitation(&session, id).unwrap().answer, Some(Rsvp::Accepted));
}

#[test]
fn a_body_without_an_invitation_and_blocked_mail_mark_nothing() {
    let mut session = act_session();
    arrive(&mut session, 1, vec![summary(40, "Hello", "h1@example.org")]);
    let id = arrived(&session, 40).id();
    fetched(&mut session, 1, 40, b"From: odile@example.org\r\nSubject: Hello\r\n\r\nHi\r\n".to_vec());
    assert!(session.store().message_state(id).unwrap().is_empty());
    // Mail in the Blocked folder is never read for an invitation.
    let mut store = crate::cmd::act_tests::store();
    let blocked = crate::test_support::folder(8, 1, "InboxCraft/Blocked", vec![]);
    let batch = inboxcraft_store::Batch::new()
        .with(Change::PutFolder(blocked))
        .with(Change::SetFolderState(FolderId::new(8), inboxcraft_model::FolderState::new(validity())));
    inboxcraft_store::Storage::apply(&mut store, &batch).unwrap();
    let mut session =
        Session::new(Box::new(store), Box::new(inboxcraft_time::FakeClock::new(crate::test_support::at(crate::test_support::NOW)))).unwrap();
    arrive(&mut session, 8, vec![summary(41, "Invitation", "i1@example.org")]);
    let id = arrived(&session, 41).id();
    let calendar = object("REQUEST", "lunch@example.org", 0, "20261107T113000Z", "");
    fetched(&mut session, 8, 41, mail("Invitation", "i1@example.org", &calendar));
    assert!(session.store().body(id).unwrap().is_some());
    assert_eq!(invitation(&session, id), None);
}
