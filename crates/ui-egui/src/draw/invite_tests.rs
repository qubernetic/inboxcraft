// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invitation card in the reader, driven through its AccessKit tree on the sample's invented lunch: the update
//! says what changed and what was answered before, Going sends at once and then shows the answer with Change answer,
//! the version a later one replaced is a faint strip, and a cancellation takes the event out of the calendar.

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::Session;
use inboxcraft_mime::{Mailbox as Sender, MessageBuilder};
use inboxcraft_model::{
    AccountId, Address, CalendarEvent, CalendarEventId, EmailAddress, Envelope, FolderId, Host, Message, MessageId, Security, Server, Signs, Summary,
    Uid,
};
use inboxcraft_store::{Batch, Body, Change, Memory, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z, a Thursday: the first invitation came on Monday.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn harness_over(mailbox: Mailbox, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 1000.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(mailbox));
    harness.run();
    harness
}

/// The sample mail with Odile's invitations.
fn invited() -> Memory {
    let now = Timestamp::from_unix(NOW, 0).unwrap();
    let mut store = sample::mailbox(now).unwrap();
    let mut batch = Batch::new();
    for change in sample::invitations(now).unwrap() {
        batch.push(change);
    }
    store.apply(&batch).unwrap();
    store
}

/// The sample with Odile's invitations, which refuses to send.
fn sample_harness() -> Harness<'static, InboxApp> {
    harness_over(Mailbox::new(sample::session_over(invited(), clock()).unwrap(), clock()), egui::Theme::Light)
}

/// The sample mail with Odile's invitations in a real session whose first account can send, with `changes` made to
/// it.
fn sending(changes: Vec<Change>) -> Harness<'static, InboxApp> {
    let mut store = invited();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new().with(Change::PutAccount(account.with_submission(Some(server))));
    for change in changes {
        batch.push(change);
    }
    store.apply(&batch).unwrap();
    harness_over(Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock()), egui::Theme::Light)
}

/// The id of the message whose subject is `subject`.
fn id_of(harness: &mut Harness<'static, InboxApp>, subject: &str) -> u64 {
    let found = harness.state_mut().mailbox_mut().run("mail.search", &json!({ "subject": subject })).unwrap();
    let messages = found["messages"].as_array().unwrap();
    let found = messages.iter().find(|message| message["subject"] == json!(subject)).unwrap();
    found["id"].as_u64().unwrap()
}

fn open(harness: &mut Harness<'static, InboxApp>, subject: &str) -> u64 {
    let id = id_of(harness, subject);
    harness.state_mut().mailbox_mut().select(Some(id));
    harness.run();
    id
}

fn button(harness: &Harness<'static, InboxApp>, name: &str) -> bool {
    harness.query_by_role_and_label(Role::Button, name).is_some()
}

fn shows(harness: &Harness<'static, InboxApp>, text: &str) -> bool {
    harness.query_by_label(text).is_some()
}

/// A calendar message from Odile in Mira's inbox, as message `id` with UID `id`: `method` and `event` (the lines of
/// its event).
fn message(id: u64, subject: &str, method: &str, event: &str) -> Vec<Change> {
    let calendar = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\nMETHOD:{method}\r\nBEGIN:VEVENT\r\nUID:{id}@example.org\r\n\
         DTSTAMP:20261104T080000Z\r\nDTSTART:20261110T180000Z\r\nDTEND:20261110T200000Z\r\nSUMMARY:Board game night\r\n{event}\
         END:VEVENT\r\nEND:VCALENDAR\r\n"
    );
    let raw = MessageBuilder::new()
        .from(Sender::named("Odile Brannock", "odile@example.org"))
        .to(Sender::new("mira@example.org"))
        .subject(subject)
        .text("See the calendar part.")
        .calendar(method, &calendar)
        .build()
        .unwrap();
    let from = Address::new(Some("Odile Brannock"), EmailAddress::new("odile@example.org").unwrap());
    let received = Timestamp::from_unix(NOW - 600, 0);
    let envelope = Envelope::new(subject).with_from(vec![from]).with_date(received).with_signs(Signs::default().with_calendar(true));
    let summary = Summary::new(Uid::new(u32::try_from(id).unwrap()).unwrap(), envelope).with_received(received);
    let message = Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(1), summary);
    vec![Change::PutMessage(message), Change::PutBody(MessageId::new(id), Body::new(raw, "See the calendar part.").unwrap())]
}

const ORGANISED_BY_ODILE: &str = "ORGANIZER;CN=Odile Brannock:mailto:odile@example.org\r\nATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:mira@example.org\r\n";

#[test]
fn the_update_says_what_changed_and_what_was_answered_before() {
    let mut harness = sample_harness();
    open(&mut harness, "Updated invitation: Lunch with Odile");
    assert!(shows(&harness, "Changed: time 11:00 → 12:30"));
    assert!(shows(&harness, "Out of date: the invitation from Monday showed 11:00. You answered Going."));
    assert!(shows(&harness, "Your answer goes to odile@example.org at once."));
    for name in ["Going", "Maybe", "No"] {
        assert!(button(&harness, name), "{name}");
    }
    assert!(harness.query_all_by_label("Lunch with Odile").count() >= 1);
    // The calendar part is the card, not a file to save.
    assert!(!button(&harness, "Save attachment"));
}

#[test]
fn an_answer_the_sample_cannot_send_is_refused_and_says_why() {
    let mut harness = sample_harness();
    open(&mut harness, "Updated invitation: Lunch with Odile");
    harness.get_by_role_and_label(Role::Button, "Going").click();
    harness.run();
    assert!(harness.state().reader().notice().is_some_and(|notice| notice.contains("submission")), "{:?}", harness.state().reader().notice());
    assert!(button(&harness, "Going"), "the card still asks");
}

#[test]
fn an_answer_goes_at_once_and_shows_with_a_way_to_change_it() {
    let mut harness = sending(Vec::new());
    let id = open(&mut harness, "Updated invitation: Lunch with Odile");
    harness.get_by_role_and_label(Role::Button, "Going").click();
    harness.run();
    assert_eq!(harness.state().reader().notice(), Some("Your answer is on its way."));
    assert!(shows(&harness, "You answered Going."));
    assert!(button(&harness, "Change answer") && !button(&harness, "Maybe"));
    let answered = harness.state_mut().mailbox_mut().run("invite.show", &json!({ "id": id })).unwrap();
    assert_eq!(answered["invite"]["answer"], json!("accepted"));
    // Change answer offers the three again, and Keep my answer goes back.
    harness.get_by_role_and_label(Role::Button, "Change answer").click();
    harness.run();
    assert!(button(&harness, "Maybe") && button(&harness, "Keep my answer"));
    harness.get_by_role_and_label(Role::Button, "Keep my answer").click();
    harness.run();
    assert!(button(&harness, "Change answer"));
    harness.get_by_role_and_label(Role::Button, "Change answer").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "No").click();
    harness.run();
    assert!(shows(&harness, "You answered No."));
}

#[test]
fn the_version_a_later_one_replaced_is_a_faint_strip() {
    let mut harness = sample_harness();
    open(&mut harness, "Invitation: Lunch with Odile");
    assert!(shows(&harness, "Out of date: a later invitation replaces this one. You answered Going."));
    assert!(!button(&harness, "Going"));
}

#[test]
fn a_cancellation_takes_the_event_out_of_the_calendar_with_undo() {
    let mut changes = message(900, "Cancelled: Board game night", "CANCEL", ORGANISED_BY_ODILE);
    let held = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap().calendars(AccountId::new(1)).unwrap().remove(0);
    let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\nBEGIN:VEVENT\r\nUID:900@example.org\r\n\
                DTSTART:20261110T180000Z\r\nDTEND:20261110T200000Z\r\nSUMMARY:Board game night\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let event = CalendarEvent {
        id: CalendarEventId::new(900),
        calendar: held.id,
        href: format!("{}games.ics", held.href),
        etag: None,
        ical: ical.to_owned(),
    };
    changes.push(Change::PutCalendarEvent(event));
    let mut harness = sending(changes);
    open(&mut harness, "Cancelled: Board game night");
    assert!(shows(&harness, "Odile Brannock cancelled this event."));
    harness.get_by_role_and_label(Role::Button, "Remove from calendar").click();
    harness.run();
    assert!(shows(&harness, "It is not in your calendar."));
    assert!(!button(&harness, "Remove from calendar"));
    harness.get_by_role_and_label(Role::Button, "Undo").click();
    harness.run();
    assert!(button(&harness, "Remove from calendar"), "Undo puts the event back");
}

#[test]
fn a_cancellation_of_an_event_not_in_the_calendar_says_so() {
    let mut harness = sending(message(901, "Cancelled: Board game night", "CANCEL", ORGANISED_BY_ODILE));
    open(&mut harness, "Cancelled: Board game night");
    assert!(shows(&harness, "It is not in your calendar."));
}

#[test]
fn a_reply_and_a_published_event_only_tell() {
    let reply = "ORGANIZER:mailto:mira@example.org\r\nATTENDEE;CN=Béla;PARTSTAT=DECLINED:mailto:bela@example.org\r\n";
    let mut changes = message(902, "Declined: Board game night", "REPLY", reply);
    changes.extend(message(903, "Board game night", "PUBLISH", ""));
    let mut harness = sending(changes);
    open(&mut harness, "Declined: Board game night");
    assert!(shows(&harness, "Béla is not going."));
    open(&mut harness, "Board game night");
    assert!(shows(&harness, "This calendar message asks for no answer."));
}

#[test]
fn an_invitation_the_owner_organises_cannot_be_answered() {
    let own = "ORGANIZER:mailto:mira@example.org\r\nATTENDEE:mailto:bela@example.org\r\n";
    let mut harness = sending(message(904, "Invitation: Board game night", "REQUEST", own));
    open(&mut harness, "Invitation: Board game night");
    assert!(shows(&harness, "This invitation cannot be answered: you organise it."));
    assert!(!button(&harness, "Going"));
}

#[test]
fn a_generated_body_gives_way_to_the_card_and_a_written_one_stays() {
    let mut harness = sample_harness();
    open(&mut harness, "Updated invitation: Lunch with Odile");
    assert!(!shows(&harness, "Odile Brannock invites you to Lunch with Odile at The Gull café."), "the card says it all");
    assert!(!button(&harness, "Reply"), "and stands for the message");
    let mut harness = sending(message(905, "Invitation: Board game night", "REQUEST", ORGANISED_BY_ODILE));
    open(&mut harness, "Invitation: Board game night");
    assert!(shows(&harness, "See the calendar part."), "a body with more to say stays readable");
    assert!(button(&harness, "Reply"));
}
