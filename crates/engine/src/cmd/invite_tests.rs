// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Answering invitations from mail: `invite.show` reads the card, `invite.answer` sends the reply at once through
//! the `mail.send` path and puts the event in the owner's calendar, `invite.remove` takes a cancelled event out. The
//! chain test plays an invented event through request, reply, update and cancel.

use inboxcraft_model::{
    AccountId, CalendarEventId, CalendarId, CalendarJob, DraftId, Event, EventKind, FolderId, FolderState, JobKind, MessageId, Rsvp,
};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use super::calendar_tests::WORK;
use super::compose_tests::validity;
use super::send_tests::store as sending_store;
use crate::arrive::invite::tests::{mail, object, summary};
use crate::test_support::{NOW, at};
use crate::{AgentClass, Caller, EngineError, Session};

/// The sending store with account 1's INBOX (1) and Sent (2) synced, a CalDAV address and its Work calendar (1),
/// empty.
fn store() -> Memory {
    let mut store = sending_store();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let batch = Batch::new()
        .with(Change::PutAccount(account.with_caldav(Some("https://dav.example.org/")).unwrap()))
        .with(Change::PutCalendar(inboxcraft_model::Calendar {
            id: CalendarId::new(1),
            account: AccountId::new(1),
            href: WORK.to_owned(),
            name: "Work".to_owned(),
            colour: None,
        }))
        .with(Change::SetFolderState(FolderId::new(1), FolderState::new(validity())))
        .with(Change::SetFolderState(FolderId::new(2), FolderState::new(validity())));
    store.apply(&batch).unwrap();
    store
}

fn session_of(store: Memory) -> Session {
    let mut session = Session::new(Box::new(store), Box::new(FakeClock::new(at(NOW)))).unwrap();
    session.take_jobs();
    session
}

fn session() -> Session {
    session_of(store())
}

fn event(kind: EventKind) -> Event {
    Event::new(AccountId::new(1), None, kind)
}

/// An invitation `calendar` arrives in account 1's INBOX with UID `n` and its body is kept; its message id.
fn arrive(session: &mut Session, n: u32, message_id: &str, calendar: &str) -> u64 {
    let subject = "Invitation: Lunch with Odile";
    let uid = inboxcraft_model::Uid::new(n).unwrap();
    session
        .apply(&event(EventKind::MessagesAdded {
            folder: FolderId::new(1),
            uid_validity: validity(),
            messages: vec![summary(n, subject, message_id)],
        }))
        .unwrap();
    session
        .apply(&event(EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity(), uid, raw: mail(subject, message_id, calendar) }))
        .unwrap();
    let entry = session.store().messages(FolderId::new(1)).unwrap().into_iter().find(|entry| entry.message().summary().uid() == uid).unwrap();
    entry.message().id().get()
}

fn show(session: &mut Session, id: u64) -> Value {
    session.run("invite.show", &json!({ "id": id })).unwrap()["invite"].clone()
}

fn answer(session: &mut Session, id: u64, answer: &str) -> Result<Value, EngineError> {
    session.run("invite.answer", &json!({ "id": id, "answer": answer }))
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

/// What one answer handed out: the message sent, its recipients, and the calendar writes.
struct Out {
    draft: DraftId,
    recipients: Vec<String>,
    raw: String,
    writes: Vec<CalendarJob>,
    /// The ids of the writes.
    ids: Vec<inboxcraft_model::JobId>,
}

fn out(session: &mut Session) -> Out {
    let mut sent = None;
    let mut writes = Vec::new();
    let mut ids = Vec::new();
    for job in session.take_jobs() {
        match job.kind {
            JobKind::Send { draft, recipients, raw, .. } => {
                assert!(sent.is_none(), "two sends");
                sent = Some((draft, recipients.iter().map(|address| address.as_str().to_owned()).collect(), String::from_utf8(raw).unwrap()));
            }
            JobKind::Calendar(calendar @ (CalendarJob::Put { .. } | CalendarJob::Delete { .. })) => {
                ids.push(job.id);
                writes.push(calendar);
            }
            _ => {}
        }
    }
    let (draft, recipients, raw) = sent.unwrap();
    Out { draft, recipients, raw, writes, ids }
}

/// The server took every write of `out`.
fn written(session: &mut Session, out: &Out) {
    for (id, write) in out.ids.iter().zip(&out.writes) {
        if let CalendarJob::Put { calendar, event, .. } = write {
            let report = inboxcraft_model::CalendarReport::Written { calendar: *calendar, event: *event, etag: Some("\"w\"".to_owned()) };
            session.apply(&Event::new(AccountId::new(1), Some(*id), EventKind::Calendar(report))).unwrap();
        }
    }
}

/// The calendar text of a `Put`, lines unfolded.
fn put_text(job: &CalendarJob) -> String {
    match job {
        CalendarJob::Put { ical, .. } => ical.replace("\r\n ", ""),
        other => panic!("{other:?}"),
    }
}

/// The text of the iTIP object a sent message carries, lines unfolded.
fn itip(raw: &str) -> String {
    let message = inboxcraft_mime::parse(raw.as_bytes()).unwrap();
    let inboxcraft_mime::Body::Multipart(parts) = message.body() else { panic!("not a multipart: {raw}") };
    let part = parts.parts().iter().find(|part| part.content_type().is("text", "calendar")).unwrap();
    assert_eq!(part.content_type().param("method"), Some("REPLY"));
    part.text().unwrap().replace("\r\n ", "")
}

const REQUEST_AT: &str = "20261107T113000Z";

#[test]
fn reading_is_safe_and_answering_and_removing_are_the_owners() {
    let mut session = session();
    let class = |session: &Session, id: &str| session.registry().get(id).and_then(|spec| spec.class);
    assert_eq!(class(&session, "invite.show"), Some(AgentClass::Safe));
    assert_eq!(class(&session, "invite.answer"), Some(AgentClass::Owner));
    assert_eq!(class(&session, "invite.remove"), Some(AgentClass::Owner));
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    assert!(session.run_as(Caller::Agent, "invite.show", &json!({ "id": id })).is_ok());
    let refused = session.run_as(Caller::Agent, "invite.answer", &json!({ "id": id, "answer": "accepted" }));
    assert!(matches!(refused, Err(EngineError::NotAllowed { .. })), "{refused:?}");
    assert!(session.take_jobs().iter().all(|job| !matches!(job.kind, JobKind::Send { .. })));
}

#[test]
fn a_message_without_an_invitation_has_no_card() {
    let mut session = session();
    assert_eq!(show(&mut session, 1), Value::Null);
    assert_eq!(session.run("mail.read", &json!({ "id": 1 })).unwrap()["message"]["invitation"], json!(false));
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    assert_eq!(session.run("mail.read", &json!({ "id": id })).unwrap()["message"]["invitation"], json!(true));
    assert!(matches!(session.run("invite.show", &json!({ "id": 999 })), Err(EngineError::NotFound { .. })));
    assert!(bad(answer(&mut session, 1, "accepted")));
    assert!(bad(session.run("invite.remove", &json!({ "id": 1 }))));
}

/// The chain: an invitation, the owner's answer, a changed time that makes the first card out of date, a second
/// answer, and the cancel that takes the event out of the calendar.
#[test]
fn request_reply_update_and_cancel_follow_one_event() {
    let mut session = session();
    let first = request(&mut session);
    let in_calendar = reply(&mut session, first);
    let second = update(&mut session, first, in_calendar);
    cancel(&mut session, second, in_calendar);
}

/// The request: a card with the event, Odile as the organiser, no answer yet, in the Invitations group.
fn request(session: &mut Session) -> u64 {
    let first = arrive(session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    let card = show(session, first);
    assert_eq!(card["method"], json!("request"));
    assert_eq!((&card["summary"], &card["location"]), (&json!("Lunch with Odile"), &json!("The Gull café")));
    assert_eq!((&card["start"], &card["end"], &card["all_day"]), (&json!("2026-11-07T11:30:00Z"), &json!("2026-11-07T12:30:00Z"), &json!(false)));
    assert_eq!(card["organizer"], json!({ "address": "odile@example.org", "name": "Odile" }));
    assert_eq!((&card["invited"], &card["guests"]), (&json!(true), &json!(2)));
    assert_eq!(
        (&card["answer"], &card["outdated"], &card["can_answer"], &card["sendable"]),
        (&Value::Null, &json!(false), &json!(true), &json!(true))
    );
    assert_eq!((&card["changes"], &card["earlier"], &card["calendar_event"]), (&json!([]), &Value::Null, &Value::Null));
    let listed = session.run("mail.list", &json!({ "account": 1, "group": "invitations" })).unwrap();
    assert_eq!(listed["messages"].as_array().unwrap().iter().map(|row| row["id"].clone()).collect::<Vec<_>>(), [json!(first)]);
    first
}

/// The reply goes at once to the organiser, as a REPLY with the owner's answer; the event goes into Work. Its id.
fn reply(session: &mut Session, first: u64) -> u64 {
    let answered = answer(session, first, "accepted").unwrap();
    assert_eq!((&answered["answer"], &answered["state"]), (&json!("accepted"), &json!("sending")));
    let sent = out(session);
    assert_eq!(sent.recipients, ["odile@example.org"]);
    assert!(sent.raw.contains("Subject: Accepted: Lunch with Odile\r\n"), "{}", sent.raw);
    assert!(sent.raw.contains("In-Reply-To: <i1@example.org>\r\n"), "{}", sent.raw);
    let reply = itip(&sent.raw);
    for line in ["METHOD:REPLY", "UID:lunch@example.org", "ORGANIZER;CN=Odile:mailto:odile@example.org", "DTSTART:20261107T113000Z"] {
        assert!(reply.contains(&format!("{line}\r\n")), "{line} in {reply}");
    }
    assert!(reply.contains("PARTSTAT=ACCEPTED") && reply.contains(":mailto:anna@example.org\r\n"), "{reply}");
    assert!(!reply.contains("bela@example.org") && !reply.contains("RSVP"), "only the owner answers: {reply}");
    let [write] = sent.writes.as_slice() else { panic!("{:?}", sent.writes) };
    let copy = put_text(write);
    assert!(!copy.contains("METHOD:"), "a calendar resource has no method: {copy}");
    assert!(copy.contains("UID:lunch@example.org") && copy.contains("SCHEDULE-AGENT=CLIENT"), "{copy}");
    assert!(copy.contains("PARTSTAT=ACCEPTED") && copy.contains("mailto:anna@example.org"), "{copy}");
    let CalendarJob::Put { calendar, url, .. } = write else { panic!() };
    assert_eq!(*calendar, CalendarId::new(1));
    assert!(url.starts_with(WORK) && url.ends_with(".ics"), "{url}");
    let card = show(session, first);
    assert_eq!((&card["answer"], &card["can_answer"]), (&json!("accepted"), &json!(true)));
    let in_calendar = card["calendar_event"].as_u64().unwrap();
    assert!(session.store().calendar_event(CalendarEventId::new(in_calendar)).unwrap().is_some());
    // The send ends: the draft goes, the answer stays; the server takes the event.
    session.apply(&event(EventKind::Sent { draft: sent.draft })).unwrap();
    written(session, &sent);
    assert!(session.store().draft(sent.draft).unwrap().is_none());
    assert_eq!(session.store().message_state(MessageId::new(first)).unwrap().invitation.unwrap().answer, Some(Rsvp::Accepted));
    in_calendar
}

/// The update: 12:30 instead of 11:30. The first card is out of date; the new one says what changed, and its answer
/// changes the event in the calendar to the new time. Its message.
fn update(session: &mut Session, first: u64, in_calendar: u64) -> u64 {
    let second = arrive(session, 41, "i2@example.org", &object("REQUEST", "lunch@example.org", 1, "20261107T123000Z", ""));
    let old = show(session, first);
    assert_eq!((&old["outdated"], &old["latest"], &old["can_answer"]), (&json!(true), &json!(second), &json!(false)));
    assert!(bad(answer(session, first, "declined")));
    let card = show(session, second);
    assert_eq!((&card["outdated"], &card["answer"], &card["earlier_answer"]), (&json!(false), &Value::Null, &json!("accepted")));
    assert_eq!(card["changes"], json!(["time"]));
    assert_eq!((&card["earlier"]["message"], &card["earlier"]["start"]), (&json!(first), &json!("2026-11-07T11:30:00Z")));
    assert_eq!(card["calendar_event"], json!(in_calendar));
    answer(session, second, "tentative").unwrap();
    let sent = out(session);
    let reply = itip(&sent.raw);
    assert!(reply.contains("SEQUENCE:1\r\n") && reply.contains("PARTSTAT=TENTATIVE"), "{reply}");
    assert!(sent.raw.contains("Subject: Tentative: Lunch with Odile\r\n"), "{}", sent.raw);
    let [write] = sent.writes.as_slice() else { panic!("{:?}", sent.writes) };
    let CalendarJob::Put { event: changed, .. } = write else { panic!() };
    assert_eq!(changed.get(), in_calendar);
    let ical = put_text(write);
    assert!(ical.contains("DTSTART:20261107T123000Z") && ical.contains("PARTSTAT=TENTATIVE"), "{ical}");
    second
}

/// The cancel: the card says so, the update is out of date, and the owner takes the event out, with Undo.
fn cancel(session: &mut Session, second: u64, in_calendar: u64) {
    let cancel = arrive(session, 42, "i3@example.org", &object("CANCEL", "lunch@example.org", 2, "20261107T123000Z", "STATUS:CANCELLED\r\n"));
    let card = show(session, cancel);
    assert_eq!((&card["method"], &card["can_answer"], &card["can_remove"]), (&json!("cancel"), &json!(false), &json!(true)));
    assert_eq!(show(session, second)["outdated"], json!(true));
    assert!(bad(answer(session, cancel, "accepted")));
    let removed = session.run("invite.remove", &json!({ "id": cancel })).unwrap();
    assert_eq!(removed["event"], json!(in_calendar));
    assert!(session.store().calendar_event(CalendarEventId::new(in_calendar)).unwrap().is_none());
    let card = show(session, cancel);
    assert_eq!((&card["calendar_event"], &card["can_remove"]), (&Value::Null, &json!(false)));
    assert!(bad(session.run("invite.remove", &json!({ "id": cancel }))));
    // The removal is held for the undo window like any delete, and Undo puts the event back.
    session.run("history.undo", &json!({})).unwrap();
    assert!(session.store().calendar_event(CalendarEventId::new(in_calendar)).unwrap().is_some());
}

#[test]
fn declining_takes_the_event_out_of_the_calendar_and_changing_the_answer_puts_it_back() {
    let mut session = session();
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    answer(&mut session, id, "accepted").unwrap();
    let first = out(&mut session);
    written(&mut session, &first);
    let in_calendar = show(&mut session, id)["calendar_event"].as_u64().unwrap();
    answer(&mut session, id, "declined").unwrap();
    let sent = out(&mut session);
    assert!(itip(&sent.raw).contains("PARTSTAT=DECLINED"));
    assert!(matches!(sent.writes.as_slice(), [CalendarJob::Delete { event, .. }] if event.get() == in_calendar), "{:?}", sent.writes);
    let card = show(&mut session, id);
    assert_eq!((&card["answer"], &card["calendar_event"]), (&json!("declined"), &Value::Null));
    // Declined first: nothing goes into the calendar.
    let other = arrive(&mut session, 41, "i2@example.org", &object("REQUEST", "tea@example.org", 0, REQUEST_AT, ""));
    answer(&mut session, other, "declined").unwrap();
    assert!(out(&mut session).writes.is_empty());
    answer(&mut session, other, "accepted").unwrap();
    assert_eq!(out(&mut session).writes.len(), 1);
}

#[test]
fn an_answer_that_cannot_go_is_refused_and_sends_nothing() {
    let mut session = session();
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    for word in ["yes", "", "ACCEPTED", "needs_action"] {
        assert!(bad(answer(&mut session, id, word)), "{word}");
    }
    // The owner organises it: there is nobody to answer.
    let own = object("REQUEST", "own@example.org", 0, REQUEST_AT, "").replace("CN=Odile:mailto:odile@example.org", "CN=Anna:mailto:Anna@Example.org");
    let mine = arrive(&mut session, 41, "i2@example.org", &own);
    let card = show(&mut session, mine);
    assert_eq!((&card["can_answer"], &card["why_not"]), (&json!(false), &json!("you organise it")));
    assert!(bad(answer(&mut session, mine, "accepted")));
    // No organiser to answer to.
    let nobody = object("REQUEST", "nobody@example.org", 0, REQUEST_AT, "").replace("ORGANIZER;CN=Odile:mailto:odile@example.org\r\n", "");
    let orphan = arrive(&mut session, 42, "i3@example.org", &nobody);
    assert_eq!(show(&mut session, orphan)["why_not"], json!("it names no organiser"));
    assert!(bad(answer(&mut session, orphan, "accepted")));
    // An organiser that is no mail address.
    let phone = object("REQUEST", "phone@example.org", 0, REQUEST_AT, "").replace("mailto:odile@example.org", "tel:+3612345678");
    let dialled = arrive(&mut session, 43, "i4@example.org", &phone);
    assert!(bad(answer(&mut session, dialled, "accepted")));
    assert!(session.take_jobs().iter().all(|job| !matches!(job.kind, JobKind::Send { .. })));
}

#[test]
fn an_account_that_cannot_send_says_so_and_refuses_the_answer() {
    let mut store = store();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    store.apply(&Batch::new().with(Change::PutAccount(account.with_submission(None)))).unwrap();
    let mut session = session_of(store);
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    let card = show(&mut session, id);
    assert_eq!((&card["can_answer"], &card["sendable"]), (&json!(true), &json!(false)));
    let refused = answer(&mut session, id, "accepted");
    assert!(matches!(&refused, Err(EngineError::BadParams { reason, .. }) if reason.contains("submission")), "{refused:?}");
    assert_eq!(show(&mut session, id)["answer"], Value::Null);
}

/// A body kept before invitations were read (an older build, the sample) is read when its card is first asked for.
#[test]
fn a_kept_invitation_without_a_mark_is_marked_when_its_card_is_read() {
    let mut session = session();
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    let mut state = session.store().message_state(MessageId::new(id)).unwrap();
    state.invitation = None;
    crate::test_support::change(&mut session, Change::SetMessageState(MessageId::new(id), state));
    let card = show(&mut session, id);
    assert_eq!((&card["uid"], &card["can_answer"]), (&json!("lunch@example.org"), &json!(true)));
    assert_eq!(session.store().message_state(MessageId::new(id)).unwrap().invitation.map(|mark| mark.uid), Some("lunch@example.org".to_owned()));
}

#[test]
fn an_invitation_to_someone_else_can_be_answered_and_says_so() {
    let mut session = session();
    let forwarded = object("REQUEST", "lunch@example.org", 0, REQUEST_AT, "").replace("mailto:anna@example.org", "mailto:cili@example.org");
    let id = arrive(&mut session, 40, "i1@example.org", &forwarded);
    let card = show(&mut session, id);
    assert_eq!((&card["invited"], &card["can_answer"]), (&json!(false), &json!(true)));
    answer(&mut session, id, "accepted").unwrap();
    let reply = itip(&out(&mut session).raw);
    assert!(reply.contains("mailto:anna@example.org") && !reply.contains("cili@example.org"), "{reply}");
}

#[test]
fn an_account_without_a_calendar_only_sends_the_answer() {
    let mut store = store();
    store.apply(&Batch::new().with(Change::RemoveCalendar(CalendarId::new(1)))).unwrap();
    let mut session = session_of(store);
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    let answered = answer(&mut session, id, "accepted").unwrap();
    assert_eq!(answered["calendar_event"], Value::Null);
    assert!(out(&mut session).writes.is_empty());
}

/// One occurrence of a repeating event: the answer goes, the calendar is left as it is.
#[test]
fn an_answer_to_one_occurrence_leaves_the_calendar_alone() {
    let mut session = session();
    let calendar = object("REQUEST", "standup@example.org", 0, REQUEST_AT, "RECURRENCE-ID:20261107T113000Z\r\n");
    let id = arrive(&mut session, 40, "i1@example.org", &calendar);
    answer(&mut session, id, "accepted").unwrap();
    let sent = out(&mut session);
    assert!(itip(&sent.raw).contains("RECURRENCE-ID:20261107T113000Z\r\n"));
    assert!(sent.writes.is_empty());
}

/// A guest's answer to an event the owner organises shows as a card that only tells.
#[test]
fn a_reply_shows_who_answered_what() {
    let mut session = session();
    let reply = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:party@example.org\r\n\
                 DTSTAMP:20261102T080000Z\r\nDTSTART:20261107T180000Z\r\nSUMMARY:Board game night\r\nORGANIZER:mailto:anna@example.org\r\n\
                 ATTENDEE;CN=Béla;PARTSTAT=DECLINED:mailto:bela@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let id = arrive(&mut session, 40, "r1@example.org", reply);
    let card = show(&mut session, id);
    assert_eq!(card["method"], json!("reply"));
    assert_eq!(card["replies"], json!([{ "address": "bela@example.org", "name": "Béla", "status": "declined" }]));
    assert_eq!((&card["can_answer"], &card["can_remove"]), (&json!(false), &json!(false)));
}

/// An event a CalDAV server put in the calendar on its own (RFC 6638) is the copy an answer changes, not a second one.
#[test]
fn an_event_already_in_the_calendar_is_changed_in_place() {
    let mut session = session();
    let held = inboxcraft_model::CalendarEvent {
        id: CalendarEventId::new(9),
        calendar: CalendarId::new(1),
        href: format!("{WORK}from-server.ics"),
        etag: Some("\"e1\"".to_owned()),
        ical: object("REQUEST", "lunch@example.org", 0, REQUEST_AT, "").replace("METHOD:REQUEST\r\n", ""),
    };
    crate::test_support::change(&mut session, Change::PutCalendarEvent(held));
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    assert_eq!(show(&mut session, id)["calendar_event"], json!(9));
    answer(&mut session, id, "accepted").unwrap();
    let sent = out(&mut session);
    let [CalendarJob::Put { event, etag, url, .. }] = sent.writes.as_slice() else { panic!("{:?}", sent.writes) };
    assert_eq!((event.get(), etag.as_deref(), url.as_str()), (9, Some("\"e1\""), format!("{WORK}from-server.ics").as_str()));
}

/// The owner's own answer, synced back from Sent, is no later version of the invitation: the card stays current and
/// the answer can still change. A guest's reply is no version either.
#[test]
fn the_sent_answer_synced_back_leaves_the_invitation_current() {
    let mut session = session();
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    answer(&mut session, id, "accepted").unwrap();
    let sent = out(&mut session);
    session.apply(&event(EventKind::Sent { draft: sent.draft })).unwrap();
    written(&mut session, &sent);
    let uid = inboxcraft_model::Uid::new(7).unwrap();
    let summary = summary(7, "Accepted: Lunch with Odile", "a1@example.org");
    session.apply(&event(EventKind::MessagesAdded { folder: FolderId::new(2), uid_validity: validity(), messages: vec![summary] })).unwrap();
    session.apply(&event(EventKind::BodyFetched { folder: FolderId::new(2), uid_validity: validity(), uid, raw: sent.raw.into_bytes() })).unwrap();
    let copy = session.store().messages(FolderId::new(2)).unwrap().into_iter().find(|entry| entry.message().summary().uid() == uid).unwrap();
    assert_eq!(show(&mut session, copy.message().id().get())["method"], json!("reply"), "the copy is marked");
    // A guest's reply to the same event, stamped later still.
    let guest = object("REPLY", "lunch@example.org", 0, REQUEST_AT, "").replace("DTSTAMP:20261102T080000Z", "DTSTAMP:20261106T080000Z");
    arrive(&mut session, 41, "r1@example.org", &guest);
    let card = show(&mut session, id);
    assert_eq!((&card["outdated"], &card["latest"], &card["can_answer"]), (&json!(false), &Value::Null, &json!(true)), "{card}");
    assert_eq!((&card["earlier"], &card["changes"]), (&Value::Null, &json!([])));
    answer(&mut session, id, "tentative").unwrap();
    assert!(itip(&out(&mut session).raw).contains("PARTSTAT=TENTATIVE"));
}

/// The organiser's reminders are not the owner's: a first copy in the calendar carries none, a held copy keeps its own.
#[test]
fn the_organisers_reminders_stay_out_of_the_calendar() {
    let mut session = session();
    let alarm = "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Odile says go\r\nTRIGGER:-PT2H\r\nEND:VALARM\r\n";
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, alarm));
    answer(&mut session, id, "accepted").unwrap();
    let first = out(&mut session);
    let [write] = first.writes.as_slice() else { panic!("{:?}", first.writes) };
    let copy = put_text(write);
    assert!(!copy.contains("VALARM") && !copy.contains("Odile says go"), "{copy}");
    written(&mut session, &first);
    // The owner sets a reminder of their own on the copy; a later answer keeps it, not the organiser's.
    let in_calendar = CalendarEventId::new(show(&mut session, id)["calendar_event"].as_u64().unwrap());
    let mut held = session.store().calendar_event(in_calendar).unwrap().unwrap();
    let own = "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Anna's own\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nEND:VEVENT";
    held.ical = held.ical.replacen("END:VEVENT", own, 1);
    crate::test_support::change(&mut session, Change::PutCalendarEvent(held));
    answer(&mut session, id, "tentative").unwrap();
    let second = out(&mut session);
    let [write] = second.writes.as_slice() else { panic!("{:?}", second.writes) };
    let copy = put_text(write);
    assert!(copy.contains("Anna's own") && !copy.contains("Odile says go"), "{copy}");
}

/// The copy in the owner's calendar is the event alone: no reminder, attachment, `X-` property or other component of
/// the organiser's, and every person in it marked for no server to mail.
#[test]
fn the_calendar_copy_keeps_no_alarm_or_component_of_the_organiser() {
    let mut session = session();
    let extra = "ATTACH:https://example.org/agenda.pdf\r\nX-ODILE-NOTE:bring the slides\r\n\
                 BEGIN:VALARM\r\nACTION:EMAIL\r\nATTENDEE:mailto:cili@example.org\r\nSUMMARY:Go\r\nDESCRIPTION:Go\r\nTRIGGER:-PT1H\r\nEND:VALARM\r\n";
    let calendar = object("REQUEST", "lunch@example.org", 0, REQUEST_AT, extra).replace(
        "END:VCALENDAR\r\n",
        "BEGIN:VTODO\r\nUID:todo@example.org\r\nORGANIZER:mailto:anna@example.org\r\nATTENDEE:mailto:cili@example.org\r\nEND:VTODO\r\n\
         BEGIN:VEVENT\r\nUID:broken@example.org\r\nATTENDEE:mailto:cili@example.org\r\nEND:VEVENT\r\nX-WR-CALNAME:Odile's\r\nEND:VCALENDAR\r\n",
    );
    let id = arrive(&mut session, 40, "i1@example.org", &calendar);
    answer(&mut session, id, "accepted").unwrap();
    let sent = out(&mut session);
    let [write] = sent.writes.as_slice() else { panic!("{:?}", sent.writes) };
    let copy = put_text(write);
    for gone in ["VALARM", "VTODO", "cili@example.org", "ATTACH", "X-ODILE-NOTE", "X-WR-CALNAME", "broken@example.org"] {
        assert!(!copy.contains(gone), "{gone} in {copy}");
    }
    assert_eq!(copy.matches("BEGIN:VEVENT").count(), 1, "{copy}");
    for line in copy.lines().filter(|line| line.starts_with("ORGANIZER") || line.starts_with("ATTENDEE")) {
        assert!(line.contains("SCHEDULE-AGENT=CLIENT"), "{line}");
    }
}

/// The answer names the organiser and the owner, and nobody the owner's line in the invitation points to.
#[test]
fn the_reply_names_the_organiser_and_the_owner_only() {
    let mut session = session();
    let calendar = object("REQUEST", "lunch@example.org", 0, REQUEST_AT, "").replace(
        "ATTENDEE;CN=Anna;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:anna@example.org",
        "ATTENDEE;CN=Anna;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE;\
         DELEGATED-FROM=\"mailto:dora@example.org\";MEMBER=\"mailto:team@example.org\";\
         SENT-BY=\"mailto:emil@example.org\";X-SEAT=12:mailto:anna@example.org",
    );
    let id = arrive(&mut session, 40, "i1@example.org", &calendar);
    answer(&mut session, id, "accepted").unwrap();
    let sent = out(&mut session);
    let reply = itip(&sent.raw);
    for gone in ["dora@example.org", "team@example.org", "emil@example.org", "X-SEAT", "DELEGATED", "MEMBER", "SENT-BY"] {
        assert!(!reply.contains(gone), "{gone} in {reply}");
    }
    let line = reply.lines().find(|line| line.starts_with("ATTENDEE")).unwrap();
    assert_eq!(line, "ATTENDEE;CN=Anna;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;CUTYPE=INDIVIDUAL:mailto:anna@example.org");
    let [write] = sent.writes.as_slice() else { panic!("{:?}", sent.writes) };
    assert!(!put_text(write).contains("dora@example.org"), "the copy names nobody new either");
}

/// An answer the server refused stays in the outbox as it was: it is not a draft to edit and send elsewhere.
#[test]
fn a_refused_answer_cannot_be_edited() {
    let mut session = session();
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    answer(&mut session, id, "accepted").unwrap();
    let sent = out(&mut session);
    let refused = EventKind::SendFailed { draft: sent.draft, kind: inboxcraft_model::ErrorKind::MessageRefused, reply: None };
    session.apply(&event(refused)).unwrap();
    let draft = sent.draft.get();
    assert!(bad(session.run("compose.update", &json!({ "id": draft, "to": ["cili@example.org"] }))));
    assert!(bad(session.run("compose.update", &json!({ "id": draft, "account": 2 }))));
    assert!(bad(session.run("compose.template", &json!({ "id": draft, "name": "thanks" }))));
    assert_eq!(session.store().draft(sent.draft).unwrap().unwrap().to, ["odile@example.org"]);
    // The outbox says it is an answer, so the window offers no Edit for it.
    let listed = session.run("outbox.list", &json!({})).unwrap();
    assert_eq!((&listed["items"][0]["state"], &listed["items"][0]["invite_answer"]), (&json!("refused"), &json!(true)), "{listed}");
    // Sent again, it goes at once as the first time did, with no hold to take it back in.
    let again = session.run("outbox.retry", &json!({ "id": draft })).unwrap();
    assert_eq!(again["state"], json!("sending"), "{again}");
    assert_eq!(out(&mut session).recipients, ["odile@example.org"]);
    assert!(session.store().schedule().unwrap().is_empty());
    session.apply(&event(EventKind::SendFailed { draft: sent.draft, kind: inboxcraft_model::ErrorKind::MessageRefused, reply: None })).unwrap();
    // Answering again from the invitation still works.
    answer(&mut session, id, "tentative").unwrap();
    assert!(itip(&out(&mut session).raw).contains("PARTSTAT=TENTATIVE"));
}

/// An answer goes at once, whatever hold the owner chose for other sends: it is taken back by changing the answer.
#[test]
fn an_answer_goes_at_once_without_the_hold() {
    let mut session = session();
    session.run("send.set", &json!({ "hold_seconds": 60 })).unwrap();
    let id = arrive(&mut session, 40, "i1@example.org", &object("REQUEST", "lunch@example.org", 0, REQUEST_AT, ""));
    let answered = answer(&mut session, id, "accepted").unwrap();
    assert_eq!(answered["state"], json!("sending"));
    let sent = out(&mut session);
    assert_eq!(sent.recipients, ["odile@example.org"]);
    let outbox = session.store().outbox().unwrap();
    assert!(matches!(outbox.as_slice(), [item] if item.draft == sent.draft && item.state == inboxcraft_model::OutboxState::Sending), "{outbox:?}");
    assert!(session.store().schedule().unwrap().is_empty(), "no send waits on the schedule");
    assert!(bad(session.run("mail.unsend", &json!({ "id": sent.draft.get() }))));
}
