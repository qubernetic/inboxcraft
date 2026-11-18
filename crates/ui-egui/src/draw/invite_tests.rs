// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invitation card in the reader, driven through its AccessKit tree on the sample's invented lunch: the update
//! says what changed and what was answered before, Going sends at once and then shows the answer with Change answer,
//! the version a later one replaced is a faint strip, and a cancellation takes the event out of the calendar.

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
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

/// Whether a node's label holds `text`: a line of the card heard within the card's own words.
fn holds(harness: &Harness<'static, InboxApp>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

/// A calendar message from Odile in Mira's inbox, as message `id` with UID `id`: `method` and `event` (the lines of
/// its event).
fn message(id: u64, subject: &str, method: &str, event: &str) -> Vec<Change> {
    titled(id, subject, method, "Board game night", event)
}

/// [`message`] with the event's title `summary`.
fn titled(id: u64, subject: &str, method: &str, summary: &str, event: &str) -> Vec<Change> {
    let calendar = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\nMETHOD:{method}\r\nBEGIN:VEVENT\r\nUID:{id}@example.org\r\n\
         DTSTAMP:20261104T080000Z\r\nDTSTART:20261110T180000Z\r\nDTEND:20261110T200000Z\r\nSUMMARY:{summary}\r\n{event}\
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
    assert!(holds(&harness, "Out of date: the invitation from Monday showed 11:00. You answered Going."));
    assert!(shows(&harness, "Your answer goes to odile@example.org at once."));
    for name in ["Going", "Maybe", "No"] {
        assert!(button(&harness, name), "{name}");
    }
    assert!(holds(&harness, "Invitation: Lunch with Odile, 12:30 – 13:30"), "the card names the event and when");
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

/// Each of the three sends its own answer: Going accepts, Maybe is tentative, No declines.
#[test]
fn going_maybe_and_no_each_send_their_own_answer() {
    for (name, answer, said) in [("Going", "accepted", "Going"), ("Maybe", "tentative", "Maybe"), ("No", "declined", "No")] {
        let mut harness = sending(Vec::new());
        let id = open(&mut harness, "Updated invitation: Lunch with Odile");
        harness.get_by_role_and_label(Role::Button, name).click();
        harness.run();
        assert_eq!(harness.state().reader().notice(), Some("Your answer is on its way."), "{name}");
        let answered = harness.state_mut().mailbox_mut().run("invite.show", &json!({ "id": id })).unwrap();
        assert_eq!(answered["invite"]["answer"], json!(answer), "{name}");
        assert!(shows(&harness, &format!("You answered {said}.")), "{name}");
    }
}

#[test]
fn the_version_a_later_one_replaced_is_a_faint_strip() {
    let mut harness = sample_harness();
    open(&mut harness, "Invitation: Lunch with Odile");
    assert!(holds(&harness, "Out of date: a later invitation replaces this one. You answered Going."));
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
    assert!(shows(&harness, "Cancelled: Odile Brannock cancelled this event."));
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

#[test]
fn an_invitation_in_the_reader_is_walked_by_the_keyboard_and_rests() {
    let mut harness = sample_harness();
    open(&mut harness, "Updated invitation: Lunch with Odile");
    let missed = crate::component::tests::keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "Tab never reaches {missed:?}");
    assert!(crate::component::tests::rests(&mut harness, 8), "it repaints at rest");
}

/// The rects the last frame painted filled with or stroked in `colour`: (filled, stroked).
fn painted_in(harness: &Harness<'static, InboxApp>, colour: egui::Color32) -> (Vec<egui::Rect>, Vec<egui::Rect>) {
    fn walk(shape: &egui::Shape, colour: egui::Color32, found: &mut (Vec<egui::Rect>, Vec<egui::Rect>)) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, colour, found)),
            egui::Shape::Rect(rect) => {
                if rect.fill == colour && rect.rect.height() >= 20.0 {
                    found.0.push(rect.rect);
                }
                if rect.stroke.color == colour && rect.stroke.width > 0.0 {
                    found.1.push(rect.rect);
                }
            }
            _ => {}
        }
    }
    let mut found = (Vec::new(), Vec::new());
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, colour, &mut found));
    found
}

fn palette(harness: &Harness<'static, InboxApp>) -> crate::theme::Palette {
    *crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance())).palette
}

#[test]
fn an_invitation_that_asks_has_one_primary_button_going() {
    let mut harness = sending(message(905, "Invitation: Board game night", "REQUEST", ORGANISED_BY_ODILE));
    open(&mut harness, "Invitation: Board game night");
    // The text has more to say, so Reply stands at the foot too.
    let (reply, going) = (harness.get_by_role_and_label(Role::Button, "Reply").rect(), harness.get_by_role_and_label(Role::Button, "Going").rect());
    let (filled, _) = painted_in(&harness, palette(&harness).cyan);
    let primary: Vec<egui::Rect> = filled.into_iter().filter(|rect| rect.left() >= going.left() - 200.0).collect();
    assert_eq!(primary, vec![going], "the pane's one cyan button is Going: {reply:?}");
}

#[test]
fn a_focused_button_in_an_asking_card_keeps_its_own_ring() {
    let mut harness = sending(message(905, "Invitation: Board game night", "REQUEST", ORGANISED_BY_ODILE));
    open(&mut harness, "Invitation: Board game night");
    for name in ["Going", "Maybe", "No"] {
        let mut reached = false;
        for _ in 0..80 {
            harness.key_press(egui::Key::Tab);
            harness.run();
            if harness.query_by_role_and_label(Role::Button, name).is_some_and(|node| node.accesskit_node().is_focused()) {
                reached = true;
                break;
            }
        }
        assert!(reached, "Tab reaches {name}");
        let button = harness.get_by_role_and_label(Role::Button, name).rect();
        let card = harness
            .query_all_by_role(Role::Group)
            .find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("Invitation: Board game night, ")))
            .unwrap()
            .rect();
        let (_, edges) = painted_in(&harness, palette(&harness).cyan_edge);
        // The card's own edge, and a ring of the button's own, 2 clear outside it.
        assert!(edges.iter().any(|edge| edge.contains_rect(card.shrink(1.0)) && edge.width() <= card.width() + 1.0), "the card's edge: {edges:?}");
        let ring = edges.iter().find(|edge| edge.contains_rect(button) && edge.width() < button.width() + 8.0);
        assert!(ring.is_some(), "{name} shows its ring: {button:?} in {edges:?}");
    }
}

/// The texts the last frame painted.
fn painted_words(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    fn walk(shape: &egui::Shape, found: &mut Vec<String>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Text(text) => found.push(text.galley.job.text.clone()),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

/// An invitation's text is the organiser's data: long, right-to-left overrides, control characters, escaped line
/// breaks and thousands of guests never crash the card, are drawn flat, and the card stays a card's height.
#[test]
fn hostile_invitation_text_is_drawn_flat_and_never_crashes() {
    let long = "Board ".repeat(2000);
    let summary = format!("Lunch\\nBcc: evil@example.org\\r\u{7}\u{1b}[2J\u{202e}gnp.exe {long}");
    let place = "LOCATION:Room\\n3\u{202e}\u{7}\r\n";
    let request = format!("{ORGANISED_BY_ODILE}{place}");
    let mut guests = String::from("ORGANIZER:mailto:mira@example.org\r\n");
    for n in 0..2000 {
        guests.push_str(&format!("ATTENDEE;CN=Guest\u{202e}{n};PARTSTAT=ACCEPTED:mailto:g{n}@example.org\r\n"));
    }
    let mut changes = titled(906, "Invitation: hostile", "REQUEST", &summary, &request);
    changes.extend(titled(907, "Accepted: hostile", "REPLY", &summary, &guests));
    let mut harness = sending(changes);
    for subject in ["Invitation: hostile", "Accepted: hostile"] {
        open(&mut harness, subject);
        let card = harness
            .query_all_by_role(Role::Group)
            .find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("Invitation: Lunch")));
        let card = card.unwrap_or_else(|| panic!("{subject}: the card stands"));
        let label = card.accesskit_node().label().unwrap_or_default();
        assert!(!label.contains(['\n', '\r', '\u{7}', '\u{1b}', '\u{202e}']), "{subject}: the card's name is flat: {label:?}");
        assert!(card.rect().height() < 800.0, "{subject}: a card's height, whatever the count: {:?}", card.rect());
        for words in painted_words(&harness) {
            assert!(!words.contains(['\n', '\r', '\u{7}', '\u{1b}', '\u{202e}']), "{subject}: drawn flat: {words:?}");
        }
    }
    // The guests past the first 20 are counted, not listed.
    assert!(holds(&harness, "1980 more guests answered."));
}

/// A text painted: where, its words, and the rect it was clipped to.
type Painted = (egui::Rect, String, egui::Rect);

/// Every button and every text the last frame painted: rects and words.
fn buttons_and_texts(harness: &Harness<'static, InboxApp>) -> (Vec<(String, egui::Rect)>, Vec<Painted>) {
    use egui_kittest::kittest::NodeT as _;
    let buttons = harness.query_all_by_role(Role::Button).map(|node| (node.accesskit_node().label().unwrap_or_default(), node.rect())).collect();
    fn walk(shape: &egui::Shape, clip: egui::Rect, found: &mut Vec<Painted>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, clip, found)),
            egui::Shape::Text(text) if !text.galley.job.text.trim().is_empty() => {
                found.push((egui::Rect::from_min_size(text.pos, text.galley.size()), text.galley.job.text.clone(), clip));
            }
            _ => {}
        }
    }
    let mut texts = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, clipped.clip_rect, &mut texts));
    (buttons, texts)
}

#[test]
fn an_invitation_card_keeps_its_buttons_whole_in_a_narrow_reader() {
    for (width, percent) in [(1024.0, 100), (1024.0, 130), (1100.0, 100), (1100.0, 130)] {
        let mut harness = sending(message(905, "Invitation: Board game night", "REQUEST", ORGANISED_BY_ODILE));
        harness.state_mut().set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
        harness.set_size(vec2(width, 900.0));
        open(&mut harness, "Invitation: Board game night");
        harness.run();
        let at = format!("{width} {percent}%");
        let card = harness
            .query_all_by_role(Role::Group)
            .find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("Invitation: Board game night, ")))
            .unwrap()
            .rect();
        assert!(card.right() <= width + 0.5, "the card stands in the window: {card:?} {at}");
        let (buttons, texts) = buttons_and_texts(&harness);
        let mut answers = Vec::new();
        for name in ["Going", "Maybe", "No"] {
            let rect = buttons.iter().find(|(label, _)| label == name).map(|(_, rect)| *rect).unwrap_or_else(|| panic!("{name}: {at}"));
            assert!(card.contains_rect(rect), "{name} at {rect:?} stands in the card {card:?}: {at}");
            // Its word whole inside it, not clipped away at the side (it may stand under the fold, scrolled out of sight).
            let whole = |text_rect: &egui::Rect, clip: &egui::Rect| clip.left() <= text_rect.left() + 0.5 && text_rect.right() <= clip.right() + 0.5;
            assert!(
                texts.iter().any(|(text_rect, text, clip)| text == name && rect.contains_rect(*text_rect) && whole(text_rect, clip)),
                "{name} reads whole: {texts:?} {at}"
            );
            for other in &answers {
                assert!(!rect.intersects(*other), "{name} runs over another answer: {at}");
            }
            answers.push(rect);
        }
    }
}

/// A calendar message `id` (UID `id`) from Odile in Mira's inbox, received `age` seconds before now, with the
/// Message-ID `<id@example.org>` and answering `answers` (a thread); `calendar` gives its method and event lines, or it
/// carries no invitation.
fn threaded(id: u64, subject: &str, calendar: Option<(&str, &str)>, answers: Option<u64>, age: i64) -> Vec<Change> {
    from_whom(("Odile Brannock", "odile@example.org"), 1, id, subject, calendar, answers, age)
}

/// [`threaded`] from `from` (a name and an address), in the sample's folder `folder`.
fn from_whom(from: (&str, &str), folder: u64, id: u64, subject: &str, calendar: Option<(&str, &str)>, answers: Option<u64>, age: i64) -> Vec<Change> {
    let own = format!("{id}@example.org");
    let mut builder = MessageBuilder::new()
        .from(Sender::named(from.0, from.1))
        .to(Sender::new("mira@example.org"))
        .subject(subject)
        .message_id(&own)
        .text("A word more about it.");
    if let Some(answers) = answers {
        builder = builder.in_reply_to(&format!("{answers}@example.org"));
    }
    if let Some((method, event)) = calendar {
        let object = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\nMETHOD:{method}\r\nBEGIN:VEVENT\r\nUID:{id}@example.org\r\n\
             DTSTAMP:20261104T080000Z\r\nDTSTART:20261110T180000Z\r\nDTEND:20261110T200000Z\r\nSUMMARY:Party {id}\r\n{event}\
             END:VEVENT\r\nEND:VCALENDAR\r\n"
        );
        builder = builder.calendar(method, &object);
    }
    let raw = builder.build().unwrap();
    let from = Address::new(Some(from.0), EmailAddress::new(from.1).unwrap());
    let received = Timestamp::from_unix(NOW - age, 0);
    let mut envelope = Envelope::new(subject)
        .with_from(vec![from])
        .with_date(received)
        .with_message_id(inboxcraft_model::MessageRef::new(&own))
        .with_signs(Signs::default().with_calendar(calendar.is_some()));
    if let Some(answers) = answers {
        envelope = envelope.with_in_reply_to(inboxcraft_model::MessageRef::new(&format!("{answers}@example.org")).into_iter().collect());
    }
    let summary = Summary::new(Uid::new(u32::try_from(id).unwrap()).unwrap(), envelope).with_received(received);
    let message = Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(folder), summary);
    vec![Change::PutMessage(message), Change::PutBody(MessageId::new(id), Body::new(raw, "A word more about it.").unwrap())]
}

/// The cyan-filled buttons the reader shows (right of the list, which ends at `READER`): its primaries.
fn primaries(harness: &Harness<'static, InboxApp>) -> Vec<egui::Rect> {
    painted_in(harness, palette(harness).cyan).0.into_iter().filter(|rect| rect.left() >= READER).collect()
}

/// Where the reader starts in the 1300 wide window, at the latest, in points.
const READER: f32 = 700.0;

/// The rect of the button `name` on the invitation card of the event `title`.
fn on_card(harness: &Harness<'static, InboxApp>, title: &str, name: &str) -> Option<egui::Rect> {
    let card = harness
        .query_all_by_role(Role::Group)
        .find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with(&format!("Invitation: {title}, "))))?
        .rect();
    harness.query_all_by_role_and_label(Role::Button, name).map(|node| node.rect()).find(|rect| card.contains_rect(*rect))
}

/// Unfolds the folded line of the thread that holds `words`.
fn unfold(harness: &mut Harness<'static, InboxApp>, words: &str) {
    harness
        .query_all_by_label_contains(words)
        .find(|node| node.accesskit_node().label().is_some_and(|label| label.contains("folded")))
        .unwrap()
        .click();
    for _ in 0..4 {
        harness.run();
    }
}

/// The sample's Screener folder.
const SCREENER: u64 = 16;

/// An invitation from a sender the screener holds: the let-in card's Accept is the pane's one primary, and the card
/// says what the invitation is but offers no answer, not to the pointer, not to Tab (an answer is mail to them).
#[test]
fn a_held_invitation_offers_no_answer_and_has_one_primary() {
    let organiser = "ORGANIZER;CN=Stranger:mailto:stranger@example.org\r\nATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:mira@example.org\r\n";
    let calendar = Some(("REQUEST", organiser));
    let mut harness = sending(from_whom(("Stranger", "stranger@example.org"), SCREENER, 950, "Invitation: Stranger's party", calendar, None, 600));
    let id = id_of(&mut harness, "Invitation: Stranger's party");
    harness.state_mut().mailbox_mut().open(crate::mailbox::Place::Senders);
    harness.run();
    harness.state_mut().mailbox_mut().select(Some(id));
    for _ in 0..4 {
        harness.run();
    }
    assert!(harness.state().mailbox().held(id), "the sender is held");
    assert!(holds(&harness, "Invitation: Party 950"), "the card still shows what the invitation says");
    assert!(holds(&harness, "the sender is not let in yet"), "the card says why it cannot be answered");
    for name in ["Going", "Maybe", "No", "Keep my answer", "Change answer"] {
        assert!(!button(&harness, name), "{name} is offered to a held sender");
    }
    let accept = harness.get_by_role_and_label(Role::Button, "Accept").rect();
    assert_eq!(primaries(&harness), vec![accept], "the let-in card's Accept is the one primary");
    let missed = crate::component::tests::keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "Tab never reaches {missed:?}");
    for name in ["Going", "Maybe", "No"] {
        assert!(!button(&harness, name), "{name} came back while walking");
    }
}

/// In a thread whose older message is unfolded and asks, and whose open message carries no invitation, the older
/// card's Going is the pane's one primary and Reply is secondary.
#[test]
fn an_older_unfolded_card_that_asks_holds_the_one_primary() {
    let mut changes = threaded(970, "Invitation: Party 970", Some(("REQUEST", ORGANISED_BY_ODILE)), None, 1200);
    changes.extend(threaded(971, "Re: Invitation: Party 970", None, Some(970), 600));
    let mut harness = sending(changes);
    open(&mut harness, "Re: Invitation: Party 970");
    for _ in 0..4 {
        harness.run();
    }
    assert_eq!(harness.state().reader().thread().len(), 2, "one thread of two");
    let reply = harness.get_by_role_and_label(Role::Button, "Reply").rect();
    assert_eq!(primaries(&harness), vec![reply], "Reply is the one primary with no card that asks");
    unfold(&mut harness, "Odile Brannock");
    let going = on_card(&harness, "Party 970", "Going").unwrap();
    assert_eq!(primaries(&harness), vec![going], "the older card's Going is the one primary, Reply secondary");
}

/// Two unfolded cards that ask: the newest one's Going is the pane's one primary, the older one's is secondary.
#[test]
fn of_two_cards_that_ask_the_newest_holds_the_one_primary() {
    let mut changes = threaded(980, "Invitation: Party 980", Some(("REQUEST", ORGANISED_BY_ODILE)), None, 1200);
    changes.extend(threaded(981, "Re: Invitation: Party 980", Some(("REQUEST", ORGANISED_BY_ODILE)), Some(980), 600));
    let mut harness = sending(changes);
    open(&mut harness, "Re: Invitation: Party 980");
    for _ in 0..4 {
        harness.run();
    }
    unfold(&mut harness, "Odile Brannock");
    let older = on_card(&harness, "Party 980", "Going").unwrap();
    let newer = on_card(&harness, "Party 981", "Going").unwrap();
    assert_ne!(older, newer);
    assert_eq!(primaries(&harness), vec![newer], "the newest card's Going is the one primary");
}
