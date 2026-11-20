// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Scheduled page under Sending: its list and empty states, the cancel confirmed in place, the keyboard walk, the
//! page at rest, hostile subjects and recipients, and that `schedule.cancel` runs only on the owner's press of the
//! confirmation.

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Offset, Timestamp};
use serde_json::{Value, json};

use crate::mailbox::Mailbox;
use crate::scheduled::{Kind, Scheduled, ScheduledRow, when};
use crate::settings::{Section, Under};
use crate::{InboxApp, sample};

/// Thursday 5 November 2026, 09:00 UTC.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn harness(app: InboxApp) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_max_steps(40)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The sample window that can send, with what `schedule` does to its mailbox, on the Scheduled page.
fn on_scheduled(schedule: impl FnOnce(&mut Mailbox)) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::sending(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    schedule(app.mailbox_mut());
    app.open_scheduled();
    harness(app)
}

/// The sample's four entries: an alert today 14:50, a snooze back today 16:00, a send on Friday 11:00 and a reminder
/// on Saturday 07:00.
fn four() -> Harness<'static, InboxApp> {
    on_scheduled(|mailbox| sample::scheduled(mailbox).unwrap())
}

const ALERT: &str = "Cancel the alert Today 14:50: Design review";
const SNOOZE: &str = "Cancel the snooze Today 16:00: A question about the font licence";
const SEND: &str = "Cancel the send Fri 11:00: Thank-you cards: the proof";
const REMINDER: &str = "Cancel the reminder Sat 07:00: Weekend plans";

/// What the engine still has on the schedule, as `schedule.list` answers it: each entry's `what`.
fn kept(harness: &mut Harness<'static, InboxApp>) -> Vec<String> {
    let answer = harness.state_mut().mailbox_mut().run("schedule.list", &json!({})).unwrap();
    answer["entries"].as_array().unwrap().iter().map(|entry| entry["what"].as_str().unwrap().to_owned()).collect()
}

fn focused(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().label().unwrap_or_default())
}

fn press(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
}

fn key(harness: &mut Harness<'static, InboxApp>, key: Key, modifiers: Modifiers) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
}

#[test]
fn sending_s_row_opens_the_scheduled_page_and_its_link_comes_back() {
    let mut harness = four();
    harness.state_mut().settings_mut().show(Section::Sending);
    harness.run();
    press(&mut harness, "open the list →");
    assert_eq!(harness.state().settings().under(), Some(Under::Scheduled));
    assert_eq!(harness.state().settings().section(), Section::Sending, "the page stands under Sending");
    harness.get_by_label(ALERT);
    press(&mut harness, "← Sending");
    assert_eq!(harness.state().settings().under(), None);
    assert_eq!(harness.state().settings().section(), Section::Sending);
}

#[test]
fn the_list_shows_every_entry_the_next_first_with_its_kind_subject_and_whom_it_names() {
    let harness = four();
    harness.get_by_role_and_label(Role::Heading, "Scheduled");
    harness.get_by_label_contains("Waiting · 4");
    let tops: Vec<f32> = [ALERT, SNOOZE, SEND, REMINDER].iter().map(|label| harness.get_by_label(label).rect().center().y).collect();
    for pair in tops.windows(2) {
        assert!((pair[1] - pair[0] - 41.0).abs() <= 1.0, "a row is 40 high with its hairline, the next first: {tops:?}");
    }
    let rows = harness.state().settings().scheduled().rows();
    let notes: Vec<(Kind, String, String)> = rows.iter().map(|row| (row.kind, row.title(), row.note())).collect();
    assert_eq!(
        notes,
        [
            (Kind::Alert, "Design review".to_owned(), "10 min before".to_owned()),
            (Kind::Snooze, "A question about the font licence".to_owned(), "Petra Harbour".to_owned()),
            (Kind::Send, "Thank-you cards: the proof".to_owned(), "to Greta Lindqvist".to_owned()),
            (Kind::Reminder, "Weekend plans".to_owned(), "if no reply · to Teodor Lisk".to_owned()),
        ]
    );
    assert_eq!(rows.iter().map(|row| row.account).collect::<Vec<_>>(), [Some(2), Some(2), Some(1), Some(1)], "each row knows its account's bar");
}

#[test]
fn with_nothing_waiting_the_page_says_so() {
    let harness = on_scheduled(|_| {});
    harness.get_by_label("Nothing waits for its time.");
    assert!(harness.query_by_label_contains("Waiting ·").is_none());
}

#[test]
fn a_cancel_asks_in_place_says_it_cannot_be_undone_and_runs_only_on_the_owner_s_press() {
    let mut harness = four();
    press(&mut harness, SEND);
    harness.get_by_role_and_label(Role::Heading, "Cancel the send to Greta Lindqvist?");
    harness.get_by_label("This cannot be undone.");
    harness.get_by_label("The message goes back to Drafts, where you can change it or send it.");
    assert_eq!(focused(&harness).as_deref(), Some("Keep it"), "the confirmation takes the keyboard on Keep it");
    assert_eq!(kept(&mut harness).len(), 4, "asking cancels nothing");
    // A hover and the keyboard on the danger button run nothing.
    harness.get_by_label("Cancel the send").hover();
    harness.run();
    key(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Cancel the send"));
    assert_eq!(kept(&mut harness).len(), 4, "a focus or a hover cancels nothing");
    // Esc is the confirmation's: it closes, the entry stays, the keyboard goes back to its cancel.
    key(&mut harness, Key::Escape, Modifiers::NONE);
    assert!(harness.query_by_label("This cannot be undone.").is_none());
    assert_eq!(focused(&harness).as_deref(), Some(SEND));
    assert!(harness.state().settings().is_open(), "Esc closes the confirmation only");
    assert_eq!(kept(&mut harness).len(), 4);
    // The owner's press of the confirmation cancels it: the send leaves the schedule, its draft is in Drafts again.
    press(&mut harness, SEND);
    press(&mut harness, "Cancel the send");
    assert_eq!(kept(&mut harness), ["alert", "unsnooze", "follow_up"]);
    let drafts = harness.state_mut().mailbox_mut().run("compose.list", &json!({})).unwrap();
    assert!(drafts["drafts"].as_array().unwrap().iter().any(|draft| draft["subject"] == "Thank-you cards: the proof"), "{drafts}");
    assert!(harness.query_by_label(SEND).is_none());
    harness.get_by_label_contains("Waiting · 3");
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some(REMINDER), "the keyboard goes to the row that took its place");
}

#[test]
fn keep_it_keeps_the_entry_and_each_kind_says_what_its_cancel_does() {
    let mut harness = four();
    for (row, heading, follows) in [
        (ALERT, "Cancel the alert for “Design review”?", "The event stays in the calendar; this alert does not ring."),
        (SNOOZE, "Cancel the snooze of “A question about the font licence”?", "The message comes back to its inbox now, as if its time had come."),
        (REMINDER, "Cancel the reminder on “Weekend plans”?", "The message itself stays as it is; no reminder comes if no reply arrives."),
    ] {
        press(&mut harness, row);
        harness.get_by_role_and_label(Role::Heading, heading);
        harness.get_by_label("This cannot be undone.");
        harness.get_by_label(follows);
        press(&mut harness, "Keep it");
        assert!(harness.query_by_label("This cannot be undone.").is_none());
        assert_eq!(focused(&harness).as_deref(), Some(row), "Keep it hands the keyboard back to the row's cancel");
    }
    assert_eq!(kept(&mut harness).len(), 4);
}

#[test]
fn hostile_subjects_and_recipients_are_one_clean_row() {
    let mut harness = on_scheduled(|mailbox| {
        let id = mailbox.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
        // The longest subject the engine keeps (998 bytes), with controls in it; the 10 000-character case is the row's
        // own test below and the engine's.
        let subject = format!("Line\r\none\u{0}\u{202E}gnol{}", "x".repeat(970));
        // A recipient line is capped by the engine; the longest it keeps, with controls in it.
        let to = format!("\u{2066}Mallory\r\n{} <m@example.org>", "y".repeat(200));
        mailbox.run("compose.update", &json!({ "id": id, "to": [to], "subject": subject })).unwrap();
        mailbox.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T12:00:00Z" })).unwrap();
        let id = mailbox.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
        mailbox.run("compose.update", &json!({ "id": id, "to": ["a@example.org"], "subject": "Plain" })).unwrap();
        mailbox.run("mail.send_later", &json!({ "id": id, "at": "2026-11-05T13:00:00Z" })).unwrap();
    });
    let row = harness.get_by_label_contains("Cancel the send Today 12:00").rect();
    let plain = harness.get_by_label("Cancel the send Today 13:00: Plain").rect();
    assert!((plain.center().y - row.center().y - 41.0).abs() <= 1.0, "the hostile row is one row high");
    assert!(row.right() <= 1440.0);
    let hostile = harness.state().settings().scheduled().rows().first().unwrap().clone();
    for text in [&hostile.subject, &hostile.who] {
        assert!(text.chars().count() <= 240, "capped where the row is built");
        assert!(!text.chars().any(|c| c.is_control() || c == '\u{202E}' || c == '\u{2066}'), "{text:?}");
    }
    // Its cancel still cancels it by its id.
    harness.get_by_label_contains("Cancel the send Today 12:00").click();
    harness.run();
    press(&mut harness, "Cancel the send");
    assert_eq!(kept(&mut harness), ["send"]);
}

#[test]
fn the_keyboard_walks_the_page_in_its_drawn_order() {
    let mut harness = four();
    harness.get_by_label("← Sending").focus();
    harness.run();
    let mut walked = Vec::new();
    for _ in 0..4 {
        key(&mut harness, Key::Tab, Modifiers::NONE);
        walked.push(focused(&harness).unwrap_or_default());
    }
    assert_eq!(walked, [ALERT, SNOOZE, SEND, REMINDER]);
    for _ in 0..4 {
        key(&mut harness, Key::Tab, Modifiers::SHIFT);
    }
    assert_eq!(focused(&harness).as_deref(), Some("← Sending"));
}

#[test]
fn shift_tab_round_a_long_list_reaches_the_last_entry_and_keeps_it() {
    let mut harness = on_scheduled(|mailbox| {
        for n in 0..60 {
            let id = mailbox.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
            mailbox.run("compose.update", &json!({ "id": id, "to": ["a@example.org"], "subject": format!("S{n:02}") })).unwrap();
            mailbox.run("mail.send_later", &json!({ "id": id, "at": format!("2026-11-05T{:02}:{:02}:00Z", 10 + n / 60, n % 60) })).unwrap();
        }
    });
    harness.get_by_label("Command line").focus();
    harness.run();
    key(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&harness).as_deref(), Some("Cancel the send Today 10:59: S59"), "the last row, out of sight, is reached and kept");
    key(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&harness).as_deref(), Some("Cancel the send Today 10:58: S58"));
}

#[test]
fn the_page_rests() {
    let mut harness = four();
    assert!(crate::component::tests::rests(&mut harness, 4), "the list rests");
    press(&mut harness, SNOOZE);
    assert!(crate::component::tests::rests(&mut harness, 4), "the confirmation rests");
    press(&mut harness, "Keep it");
    harness.state_mut().settings_mut().leave();
    harness.run();
    assert!(crate::component::tests::rests(&mut harness, 4), "the page left rests");
}

#[test]
fn a_kept_list_of_junk_reads_as_no_rows_not_a_panic() {
    let mut scheduled = Scheduled::default();
    let junk_entries = json!({ "entries": [1, "a", { "id": 1 }, { "id": 1, "due": "never", "what": "send" }, { "id": 1, "due": "2026-11-05T10:00:00Z", "what": "fly" }] });
    for junk in [Value::Null, json!({ "entries": 3 }), junk_entries] {
        scheduled.read(&junk);
        assert!(scheduled.rows().is_empty(), "{junk}");
    }
}

#[test]
fn a_row_read_from_a_hostile_answer_is_plain_and_capped() {
    let text = format!("a\r\nb\u{0}\u{202E}{}", "\u{301}".repeat(10_000));
    let row = ScheduledRow::read(&json!({
        "id": 7, "due": "2026-11-05T10:00:00Z", "what": "unsnooze", "account": "junk", "subject": text, "who": format!("W\u{2066}{}", "y".repeat(10_000)),
    }))
    .unwrap();
    assert!(row.subject.chars().count() <= 240 && !row.subject.chars().any(|c| c.is_control() || c == '\u{202E}'));
    assert!(row.who.chars().count() <= 240 && !row.who.chars().any(|c| c.is_control() || c == '\u{2066}'));
    assert_eq!(row.account, None, "an account that is no number is none");
}

#[test]
fn a_time_is_today_a_weekday_within_the_week_and_a_date_further_on() {
    let now = Timestamp::from_unix(NOW, 0).unwrap();
    let at = |hours: i64| Timestamp::from_unix(NOW + hours * 3_600, 0).unwrap();
    assert_eq!(when(at(5), now, Offset::UTC), "Today 14:00");
    assert_eq!(when(at(26), now, Offset::UTC), "Fri 11:00");
    assert_eq!(when(at(24 * 6), now, Offset::UTC), "Wed 09:00");
    assert_eq!(when(at(24 * 7), now, Offset::UTC), "12 Nov 09:00");
    assert_eq!(when(at(-2), now, Offset::UTC), "Today 07:00");
    assert_eq!(when(at(-24), now, Offset::UTC), "Yesterday", "a day gone by, as the list dates it");
}

#[test]
fn a_frame_formats_the_time_of_the_rows_it_draws_only() {
    let mut harness = on_scheduled(|mailbox| {
        for n in 0..200 {
            let id = mailbox.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
            mailbox.run("compose.update", &json!({ "id": id, "to": ["a@example.org"], "subject": format!("S{n:03}") })).unwrap();
            mailbox.run("mail.send_later", &json!({ "id": id, "at": format!("2026-11-05T{:02}:{:02}:00Z", 10 + n / 60, n % 60) })).unwrap();
        }
    });
    crate::scheduled::WHENS.with(|whens| whens.set(0));
    harness.step();
    let formatted = crate::scheduled::WHENS.with(std::cell::Cell::get);
    assert!(formatted > 0 && formatted < 60, "a frame formats the rows near the view and the two ends, not all 200: {formatted}");
}

#[test]
fn a_held_key_cancels_at_most_once_and_never_by_its_repeats() {
    use crate::component::tests::{held_on, hold_key};
    for key in [Key::Enter, Key::Space] {
        let mut harness = four();
        // Held on a row's cancel: the press asks, the keyboard lands on Keep it, and the repeats cancel nothing.
        harness.get_by_label(SEND).focus();
        harness.run();
        hold_key(&mut harness, key, 6);
        assert_eq!(kept(&mut harness).len(), 4, "a held {key:?} on a row's cancel cancels nothing");
        // A key already down when the danger button takes the keyboard: its repeats are no press.
        if harness.query_by_label("Cancel the send").is_none() {
            press(&mut harness, SEND);
        }
        held_on(&mut harness, key, 6, |harness| harness.get_by_label("Cancel the send").focus());
        assert_eq!(kept(&mut harness).len(), 4, "repeats of a held {key:?} on the danger button cancel nothing");
        // A fresh press, held: one cancel, whatever the keyboard lands on next.
        harness.get_by_label("Cancel the send").focus();
        harness.run();
        hold_key(&mut harness, key, 6);
        assert_eq!(kept(&mut harness).len(), 3, "a held {key:?} on the danger button cancels once");
    }
}

#[test]
fn cancelling_a_snooze_brings_the_message_back_to_its_inbox() {
    let mut harness = four();
    let answer = harness.state_mut().mailbox_mut().run("schedule.list", &json!({})).unwrap();
    let snoozed = answer["entries"].as_array().unwrap().iter().find(|entry| entry["what"] == "unsnooze").unwrap()["message"].as_u64().unwrap();
    let in_inbox = |harness: &mut Harness<'static, InboxApp>| {
        let inbox = harness.state_mut().mailbox_mut().run("mail.list", &json!({ "role": "inbox" })).unwrap();
        inbox["messages"].as_array().unwrap().iter().any(|message| message["id"] == snoozed)
    };
    assert!(!in_inbox(&mut harness), "a snoozed message waits outside the inbox");
    press(&mut harness, SNOOZE);
    harness.get_by_label("The message comes back to its inbox now, as if its time had come.");
    press(&mut harness, "Cancel the snooze");
    assert_eq!(kept(&mut harness), ["alert", "send", "follow_up"], "its return leaves the schedule");
    assert!(in_inbox(&mut harness), "the message is back in its inbox");
    let folders = harness.state_mut().mailbox_mut().run("folder.list", &json!({})).unwrap();
    let parked: Vec<u64> = folders["folders"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|folder| folder["name"] == "InboxCraft/Snoozed")
        .map(|folder| folder["id"].as_u64().unwrap())
        .collect();
    assert!(!parked.is_empty(), "{folders}");
    for folder in parked {
        let list = harness.state_mut().mailbox_mut().run("mail.list", &json!({ "folder": folder })).unwrap();
        assert!(!list["messages"].as_array().unwrap().iter().any(|message| message["id"] == snoozed), "and not in Snoozed");
    }
}

#[test]
fn entries_that_could_not_be_read_are_counted_in_the_page_s_note() {
    let mut harness = four();
    assert!(harness.query_by_label_contains("could not be read").is_none(), "nothing to say when every entry reads");
    let answer = harness.state_mut().mailbox_mut().run("schedule.list", &json!({})).unwrap();
    let mut entries = answer["entries"].as_array().unwrap().clone();
    // One the engine left out, one the window cannot read.
    entries.push(json!({ "id": 99, "due": "junk", "what": "send" }));
    harness.state_mut().settings_mut().scheduled_mut().read(&json!({ "entries": entries, "skipped": 1 }));
    harness.run();
    assert_eq!(harness.state().settings().scheduled().skipped(), 2);
    harness.get_by_label("2 waiting entries could not be read and are not listed.");
    harness.get_by_label(ALERT);
}

/// The text painted in the last frame that starts with `wanted`: where its galley stands and its colour.
fn painted(harness: &Harness<'static, InboxApp>, wanted: &str) -> Option<(egui::Rect, egui::Color32)> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<(egui::Rect, egui::Color32)> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text().starts_with(wanted) => {
                let colour = text.galley.job.sections.first().map_or(text.fallback_color, |section| section.format.color);
                Some((egui::Rect::from_min_size(text.pos, text.galley.size()), colour))
            }
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

#[test]
fn the_intro_is_the_frame_s_two_paragraphs_six_apart() {
    let harness = four();
    let first = "Everything that waits for its time, the next first: a send, a snooze that comes back, a reminder when no reply \
                 came, a calendar alert.";
    let second = "Cancelling a send puts the message back in Drafts. Cancelling a snooze brings the message back to the inbox now. \
                  Cancelling a reminder or an alert removes only that entry: the message and the event stay as they are.";
    let (first, _) = painted(&harness, first).expect("the frame's first paragraph");
    let (second, _) = painted(&harness, second).expect("the frame's second paragraph");
    assert!((second.top() - first.bottom() - 6.0).abs() <= 0.5, "6 between the paragraphs: {}", second.top() - first.bottom());
}

#[test]
fn a_row_s_parts_stand_where_the_frame_puts_them() {
    // The frame's row: the time at 16 (96), the kind at 124 (72), the bar at 208, the subject at 223 (268), the note
    // at 503 (149), cancel at 664 to 704.
    let harness = four();
    let (time, _) = painted(&harness, "Today 14:50").expect("the time is drawn");
    let (subject, _) = painted(&harness, "Design review").expect("the subject is drawn");
    let (note, _) = painted(&harness, "10 min before").expect("the note is drawn");
    let cancel = harness.get_by_label(ALERT).rect();
    let column = time.left() - 16.0;
    assert!((subject.left() - column - 223.0).abs() <= 0.5, "the subject at 223: {}", subject.left() - column);
    assert!((note.left() - column - 503.0).abs() <= 0.5, "the note at 503: {}", note.left() - column);
    assert!((cancel.left() - column - 664.0).abs() <= 1.0, "cancel at 664: {}", cancel.left() - column);
}
