// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Role as FolderRole, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::compose::Line;
use crate::mailbox::{Mailbox, Place, SendState};
use crate::sample;
use crate::workers::Scripted;

/// 2026-11-05T09:00:00Z, a Thursday.
const NOW: i64 = 1_793_869_200;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(NOW + seconds, 0).unwrap()
}

/// The window over the sample mail in a real session whose first account can send, on `clock`.
fn sending(clock: &FakeClock, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    let mailbox = Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
    harness.run();
    harness
}

/// [`sending`], both accounts of the sample able to send.
fn sending_from_both(clock: &FakeClock) -> Harness<'static, InboxApp> {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    for account in store.accounts().unwrap() {
        batch.push(Change::PutAccount(account.with_submission(Some(server.clone()))));
    }
    store.apply(&batch).unwrap();
    let mailbox = Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
    harness.run();
    harness
}

fn clock() -> FakeClock {
    FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap())
}

/// Writes a message to Odile with `subject` in the composer, from the pane's New message.
fn write(harness: &mut Harness<'static, InboxApp>, subject: &str) {
    crate::draw::tests::new_message(harness);
    harness.run();
    let app = harness.state_mut();
    let (Some(composer), mailbox) = app.composer_mut() else { panic!("no composer") };
    "Odile Brannock <odile@example.org>".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.take(mailbox, Line::To).unwrap();
    subject.clone_into(composer.subject_mut());
    composer.edited();
    composer.save(mailbox).unwrap();
    harness.run();
}

/// [`write`], then Send.
fn send(harness: &mut Harness<'static, InboxApp>, subject: &str) -> u64 {
    write(harness, subject);
    let draft = harness.state().composer().unwrap().draft();
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    draft
}

#[test]
fn a_send_counts_down_in_its_toast_and_undo_opens_the_draft_again() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        let draft = send(&mut harness, "Work party on Saturday");
        assert!(harness.state().composer().is_none(), "sent");
        // A screen reader hears the subject and that it can be undone, not the seconds: the label stays as they pass.
        harness.get_by_label("sending · Work party on Saturday, can be undone");
        assert_eq!(harness.state().counting(), Some((draft, 5)));
        clock.advance(Duration::from_secs(2));
        harness.run();
        harness.get_by_label("sending · Work party on Saturday, can be undone");
        assert_eq!(harness.state().counting(), Some((draft, 3)), "counted down");
        harness.get_by_role_and_label(Role::Button, "undo").click();
        harness.run();
        assert!(harness.state().mailbox().outbox().is_empty(), "taken back");
        let composer = harness.state().composer().expect("the draft opens again");
        assert_eq!((composer.draft(), composer.subject()), (draft, "Work party on Saturday"));
        assert!(harness.query_by_label_contains("sending · ").is_none(), "the countdown is gone");
    }
}

#[test]
fn the_send_s_undo_stops_it_under_the_command_line_s_results() {
    // Its time runs out under the results too: one press, a screen reader's activate or the key takes the send back,
    // and the draft opens again as it does without them.
    for how in ["press", "activate", "key"] {
        let clock = clock();
        let mut harness = sending(&clock, egui::Theme::Light);
        let draft = send(&mut harness, "Work party on Saturday");
        harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
        harness.run();
        harness.state_mut().command_mut().set_text(">zz");
        harness.run();
        assert!(harness.state().command().results(), "{how}: the results show");
        assert_eq!(harness.state().mailbox().outbox().len(), 1, "{how}: counting down");
        match how {
            "press" => harness.get_by_role_and_label(Role::Button, "undo").click(),
            "activate" => harness.get_by_role_and_label(Role::Button, "undo").click_accesskit(),
            _ => harness.key_press_modifiers(Modifiers::COMMAND, Key::Z),
        }
        harness.run();
        assert!(harness.state().mailbox().outbox().is_empty(), "{how}: taken back by one");
        assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft), "{how}: the draft opens again");
    }
}

/// Opens the command line with Ctrl+K and types `>zz` in it one character a frame, so the field has edits to undo.
fn type_in_the_command_line(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    for typed in [">", "z", "z"] {
        harness.event(egui::Event::Text(typed.to_owned()));
        harness.run();
    }
    assert_eq!(harness.state().command().text(), "zz", "typed");
    assert!(harness.state().command().results(), "the results show");
}

#[test]
fn ctrl_z_in_the_command_line_stops_the_send_and_leaves_the_field_as_typed() {
    // One key, one thing: the send's time runs out, so its Undo takes the key, and the field's last edit stays.
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    send(&mut harness, "Work party on Saturday");
    type_in_the_command_line(&mut harness);
    assert_eq!(harness.state().mailbox().outbox().len(), 1, "counting down");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(harness.state().mailbox().outbox().is_empty(), "the send is stopped");
    assert_eq!(harness.state().command().text(), "zz", "the field keeps its text");
}

#[test]
fn ctrl_z_in_the_command_line_without_a_countdown_is_the_field_s_own_undo() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    type_in_the_command_line(&mut harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(harness.state().command().text(), "z", "the field takes its last edit back");
    assert!(harness.state().command().results(), "the results stay");
}

#[test]
fn ctrl_z_takes_the_send_back_while_it_counts_down() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    let draft = send(&mut harness, "Work party on Saturday");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(harness.state().mailbox().outbox().is_empty());
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft));
}

#[test]
fn the_toast_goes_once_the_hold_is_over_and_the_send_is_on_its_way() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    send(&mut harness, "Work party on Saturday");
    clock.advance(Duration::from_secs(6));
    harness.run();
    assert_eq!(harness.state().counting(), None);
    assert!(harness.query_by_label_contains("sending · ").is_none());
    let state = harness.state().mailbox().outbox().first().map(|row| row.state.clone());
    assert_eq!(state, Some(SendState::Sending), "handed to the worker");
}

#[test]
fn the_outbox_shows_in_the_pane_while_anything_waits_and_lists_it() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        assert!(harness.query_by_label_contains("Outbox").is_none(), "nothing waits");
        let draft = send(&mut harness, "Work party on Saturday");
        harness.get_by_label("Outbox, 1 send").click();
        harness.run();
        assert_eq!(harness.state().mailbox().place(), Place::Outbox);
        harness.get_by_label("1 waiting");
        harness.get_by_label("To Odile Brannock <odile@example.org>, Work party on Saturday, Sending in 5 s");
        harness.get_by_role_and_label(Role::Button, "Undo the send of “Work party on Saturday”").click();
        harness.run();
        assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft));
        assert_eq!(harness.state().mailbox().place(), Place::View(FolderRole::Inbox), "the outbox is empty: it gives way");
        assert!(harness.query_by_label("Outbox, 1 send").is_none());
    }
}

#[test]
fn beside_the_outbox_the_reader_says_nothing_opens_and_the_line_offers_no_open_key() {
    // `outbox/mixed`: a row of the outbox is a send, not a message; the keys are next, previous, commands (#325, I-2).
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Dark);
    send(&mut harness, "Work party on Saturday");
    assert!(harness.state().composer().is_none(), "the send closed the composer");
    harness.state_mut().mailbox_mut().open(Place::Outbox);
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Outbox);
    assert!(harness.state().reader.open_id().is_none());
    assert!(harness.query_by_label_contains("Nothing opens from here.").is_some());
    assert!(harness.query_by_label_contains("Pick a message.").is_none());
    let words: Vec<String> = ["next", "previous", "commands"].map(str::to_owned).into();
    assert_eq!(super::round_tests::key_words(&harness), words);
}

#[test]
fn a_time_picked_makes_send_a_schedule_and_the_outbox_says_when() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Re: Draft logo for the bakery");
    harness.get_by_role_and_label(Role::Button, "Send later").click();
    harness.run();
    assert_eq!(harness.query_all_by_label("Send later").count(), 2, "the caret and the picker's title");
    harness.get_by_label_contains("Tomorrow morning").click();
    harness.run();
    assert_eq!(harness.state().composer().and_then(crate::compose::Composer::later), Some(at(86_400 - 3_600)));
    harness.get_by_role_and_label(Role::Button, "Schedule · Fri 08:00").click();
    harness.run();
    assert!(harness.state().composer().is_none());
    assert_eq!(harness.state().notice(), Some("Scheduled for Friday 6 November, 08:00"));
    assert_eq!(harness.state().counting(), None, "no countdown for a send later");
    harness.get_by_label("Outbox, 1 send").click();
    harness.run();
    harness.get_by_label("To Odile Brannock <odile@example.org>, Re: Draft logo for the bakery, Scheduled · Fri 08:00");
    harness.get_by_role_and_label(Role::Button, "Take “Re: Draft logo for the bakery” back and edit it").click();
    harness.run();
    assert!(harness.state().mailbox().outbox().is_empty());
    assert!(harness.state().composer().is_some(), "the draft is open again");
}

#[test]
fn send_now_instead_takes_the_time_away() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Hello");
    let app = harness.state_mut();
    let (Some(composer), _) = app.composer_mut() else { panic!("no composer") };
    composer.set_later(Some(at(3_600)));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Send later").click();
    harness.run();
    harness.get_by_label("Send now instead").click();
    harness.run();
    assert_eq!(harness.state().composer().and_then(crate::compose::Composer::later), None);
    harness.get_by_role_and_label(Role::Button, "Send");
}

#[test]
fn a_time_picked_that_has_passed_blocks_send_and_says_so() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Hello");
    let app = harness.state_mut();
    let (Some(composer), _) = app.composer_mut() else { panic!("no composer") };
    composer.set_later(Some(at(60)));
    clock.advance(Duration::from_secs(120));
    harness.run();
    harness.get_by_label("The time picked has passed: pick another");
    assert!(harness.state().composer().is_some());
}

#[test]
fn a_refused_send_shows_the_reply_and_can_be_edited_or_sent_again() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        let draft = send(&mut harness, "Work party on Saturday");
        clock.advance(Duration::from_secs(6));
        harness.state_mut().mailbox_mut().pump(&mut Scripted::default().refusing("550 5.1.1 No such user here"));
        harness.state_mut().mailbox_mut().open(Place::Outbox);
        harness.run();
        harness.get_by_label("1 not sent");
        harness.get_by_label("To Odile Brannock <odile@example.org>, Work party on Saturday, Not sent: 550 5.1.1 No such user here");
        harness.get_by_role_and_label(Role::Button, "Send “Work party on Saturday” again").click();
        harness.run();
        assert_eq!(harness.state().counting(), Some((draft, 5)), "held again, counted down");
        harness.get_by_label("sending · Work party on Saturday, can be undone");
        clock.advance(Duration::from_secs(6));
        harness.state_mut().mailbox_mut().pump(&mut Scripted::default().refusing("451 4.3.0 Try again later"));
        harness.run();
        harness.get_by_role_and_label(Role::Button, "Edit “Work party on Saturday”").click();
        harness.run();
        assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft));
        assert_eq!(harness.state().mailbox().outbox().len(), 1, "still refused until it is sent again");
    }
}

#[test]
fn a_send_that_went_late_says_so_once() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Work party on Saturday");
    let app = harness.state_mut();
    let (Some(composer), _) = app.composer_mut() else { panic!("no composer") };
    composer.set_later(Some(at(3_600)));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Schedule · 10:00").click();
    harness.run();
    clock.advance(Duration::from_secs(7_200));
    harness.run();
    assert_eq!(harness.state().notice(), Some("Sent late, InboxCraft was not running at its time: Work party on Saturday"));
}

#[test]
fn the_sending_page_sets_how_long_a_send_is_held() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        harness.state_mut().settings_mut().open_at(crate::settings::Section::Sending);
        harness.run();
        assert!(harness.query_all_by_label_contains("Undo send. Every message waits here").next().is_some());
        let toggled =
            |harness: &Harness<'static, InboxApp>, name: &str| harness.get_by_role_and_label(Role::RadioButton, name).accesskit_node().toggled();
        assert_eq!(toggled(&harness, "5 s"), Some(Toggled::True), "five seconds until the owner chooses");
        harness.get_by_role_and_label(Role::RadioButton, "30 s").click();
        harness.run();
        assert_eq!(harness.state().mailbox().hold_seconds(), 30);
        assert_eq!(toggled(&harness, "30 s"), Some(Toggled::True));
        harness.get_by_label_contains("Now 30 s.");
    }
}

#[test]
fn a_bounced_message_is_marked_in_its_row_and_its_reader_says_why() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
        sample::bounce(&mut store).unwrap();
        let mailbox = Mailbox::new(Session::sample(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
        let mut harness = Harness::builder()
            .with_size(vec2(1300.0, 820.0))
            .with_theme(theme)
            .with_max_steps(12)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
        harness.state_mut().mailbox_mut().open(Place::View(FolderRole::Sent));
        harness.run();
        harness.get_by_label_contains("not delivered").click();
        harness.run();
        harness.get_by_label_contains("Not delivered: This message did not arrive: petra@example.org: 550 5.1.1");
    }
}

/// An answer to an invitation the server refused can be sent again, but not edited: its draft is the answer, which
/// changes only from the invitation.
#[test]
fn a_refused_invitation_answer_offers_no_edit() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
        let draft = inboxcraft_model::Draft {
            to: vec!["odile@example.org".to_owned()],
            subject: "Accepted: Lunch with Odile".to_owned(),
            calendar: Some("BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n".to_owned()),
            ..inboxcraft_model::Draft::new(inboxcraft_model::DraftId::new(90), AccountId::new(1), at(0))
        };
        let refused = inboxcraft_model::OutboxState::Refused { reply: "550 5.1.1 No such user here".to_owned() };
        let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
        let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
        let mut batch = Batch::new();
        batch.push(Change::PutAccount(account.with_submission(Some(server))));
        // Kept as it went, as every send is: nothing changed since.
        let outgoing = inboxcraft_model::Outgoing::of(&draft, refused);
        batch.push(Change::PutDraft(draft));
        batch.push(Change::PutOutgoing(outgoing));
        store.apply(&batch).unwrap();
        let mailbox = Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
        let mut harness = Harness::builder()
            .with_size(vec2(1300.0, 820.0))
            .with_theme(theme)
            .with_max_steps(12)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
        harness.state_mut().mailbox_mut().open(Place::Outbox);
        harness.run();
        assert!(harness.state().mailbox().outbox().first().is_some_and(|row| row.invite_answer));
        assert!(harness.query_by_label("Edit “Accepted: Lunch with Odile”").is_none());
        harness.get_by_role_and_label(Role::Button, "Send “Accepted: Lunch with Odile” again").click();
        harness.run();
        // It goes at once: no hold, no countdown, no Undo.
        assert_eq!(harness.state().counting(), None);
        assert!(harness.query_by_label_contains("sending · ").is_none());
        let state = harness.state().mailbox().outbox().first().map(|row| row.state.clone());
        assert_eq!(state, Some(SendState::Sending));
    }
}

#[test]
fn each_send_of_the_outbox_tells_a_screen_reader_its_place_in_the_list() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    send(&mut harness, "First of two");
    send(&mut harness, "Second of two");
    harness.state_mut().mailbox_mut().open(Place::Outbox);
    harness.run();
    let list = harness.get_by_role_and_label(Role::List, "Sends");
    assert_eq!(list.accesskit_node().size_of_set(), Some(2), "the list's size");
    let mut places: Vec<(Option<usize>, Option<usize>)> = harness
        .query_all_by_role(Role::ListItem)
        .filter(|node| node.accesskit_node().label().is_some_and(|label| label.contains(" of two")))
        .map(|node| (node.accesskit_node().position_in_set(), node.accesskit_node().size_of_set()))
        .collect();
    places.sort();
    assert_eq!(places, [(Some(0), Some(2)), (Some(1), Some(2))]);
}

#[test]
fn a_refused_send_is_the_outbox_rows_72_high_and_a_held_one_56() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Dark);
    send(&mut harness, "Work party on Saturday");
    harness.state_mut().mailbox_mut().open(Place::Outbox);
    harness.run();
    let held = harness.get_by_label("To Odile Brannock <odile@example.org>, Work party on Saturday, Sending in 5 s").rect();
    assert_eq!(held.height(), 56.0, "{held:?}");
    clock.advance(Duration::from_secs(6));
    harness.state_mut().mailbox_mut().pump(&mut Scripted::default().refusing("550 5.1.1 No such user here"));
    harness.run();
    let refused = harness.get_by_label("To Odile Brannock <odile@example.org>, Work party on Saturday, Not sent: 550 5.1.1 No such user here").rect();
    assert_eq!(refused.height(), 72.0, "{refused:?}");
}

#[test]
fn a_countdown_toast_never_stands_over_the_docked_composer_s_send() {
    // Beside the list (1300) the toast is centred on the list; where the composer takes the body (800) it stands over
    // the composer's foot.
    for width in [1300.0, 800.0] {
        let clock = clock();
        let mut harness = sending(&clock, egui::Theme::Light);
        harness.set_size(vec2(width, 820.0));
        harness.run();
        send(&mut harness, "First of two");
        write(&mut harness, "Second of two");
        let toast = harness.get_by_label_contains("sending · First of two").rect();
        let send = harness.get_by_role_and_label(Role::Button, "Send").rect();
        assert!(!toast.intersects(send), "{width}: the toast {toast:?} over Send {send:?}");
    }
}

/// The status line's message whose words hold `words`: its words and its Undo.
fn toast_float(harness: &Harness<'static, InboxApp>, words: &str) -> egui::Rect {
    let said = harness.get_by_label_contains(words).rect();
    let undo = harness.get_by_role_and_label(Role::Button, "undo").rect();
    said.union(undo)
}

/// A press and release of the pointer at `at`.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2) {
    use egui::{Event, PointerButton};
    harness.event(Event::PointerMoved(at));
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
    harness.run();
}

#[test]
fn a_countdown_toast_with_a_long_subject_keeps_clear_of_send_at_every_width_and_size() {
    // Beside the list (1300, 1100), in one pane (900, 800) at 130 %: the toast's float never meets Send, and a press on
    // Send's middle sends the second message rather than taking the first back.
    let long = "The minutes of the allotment society's autumn meeting and the rota for the winter digging";
    for (width, percent) in [(1300.0, 100), (1100.0, 100), (1300.0, 130), (900.0, 130), (800.0, 130), (800.0, 100)] {
        let clock = clock();
        let mut harness = sending(&clock, egui::Theme::Light);
        harness.state_mut().set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
        harness.set_size(vec2(width, 820.0));
        harness.run();
        send(&mut harness, long);
        write(&mut harness, "Second of two");
        let toast = toast_float(&harness, "sending · The minutes");
        let send = harness.get_by_role_and_label(Role::Button, "Send").rect();
        assert!(!toast.intersects(send), "{width} at {percent} %: the toast {toast:?} over Send {send:?}");
        assert!(toast.left() >= 0.0 && toast.right() <= width, "{width} at {percent} %: the toast {toast:?} inside the window");
        press_at(&mut harness, send.center());
        assert!(harness.state().composer().is_none(), "{width} at {percent} %: the press on Send sent the second message");
        assert_eq!(harness.state().mailbox().outbox().len(), 2, "{width} at {percent} %: the first one was not taken back");
    }
}

/// Sends "Work party on Saturday" to Odile, has the server refuse it, and opens the outbox; the draft.
fn refused(harness: &mut Harness<'static, InboxApp>, clock: &FakeClock) -> u64 {
    let draft = send(harness, "Work party on Saturday");
    clock.advance(Duration::from_secs(6));
    harness.state_mut().mailbox_mut().pump(&mut Scripted::default().refusing("550 5.1.1 No such user here"));
    harness.state_mut().mailbox_mut().open(Place::Outbox);
    harness.run();
    draft
}

/// An agent (the control channel) changes the refused draft's recipients.
fn agent_changes(harness: &mut Harness<'static, InboxApp>, draft: u64, change: &serde_json::Value) {
    let mut params = change.clone();
    params["id"] = serde_json::json!(draft);
    harness.state_mut().mailbox_mut().run_for_agent("compose.update", &params).unwrap();
    harness.run();
}

/// Whether the outbox row offers Send again for "Work party on Saturday".
fn offers_send_again(harness: &Harness<'static, InboxApp>) -> bool {
    harness.query_by_label("Send “Work party on Saturday” again").is_some()
}

/// The refused send changed since: its row offers no send, only Edit, and Edit opens it in the composer, whose own
/// Send sends it. Nothing went before that, and an agent cannot send it again at all.
fn only_edit_sends(harness: &mut Harness<'static, InboxApp>, draft: u64) {
    assert!(!offers_send_again(harness), "a changed refused send offers no send from its row");
    assert!(harness.query_by_label_contains("as it is now").is_none());
    let retried = harness.state_mut().mailbox_mut().run_for_agent("outbox.retry", &serde_json::json!({ "id": draft }));
    assert!(matches!(retried, Err(inboxcraft_engine::EngineError::NotAllowed { .. })), "{retried:?}");
    // The owner's own retry of it, from a row read before it changed, is refused by the engine.
    assert!(harness.state_mut().mailbox_mut().retry(draft).is_err(), "outbox.retry refuses a changed draft");
    harness.run();
    assert_eq!(harness.state().counting(), None, "nothing went");
    assert!(matches!(harness.state().mailbox().outbox()[0].state, SendState::Refused { .. }));
    harness.get_by_role_and_label(Role::Button, "Edit “Work party on Saturday”").click();
    harness.run();
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft));
    assert_eq!(harness.state().counting(), None, "opening it sends nothing");
}

/// C1: an agent gives a recipient the display name of another address. The row shows the address it goes to, and
/// offers no send: only Edit.
#[test]
fn a_recipient_named_as_another_address_is_shown_by_its_address_and_only_edited() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        let draft = refused(&mut harness, &clock);
        agent_changes(&mut harness, draft, &serde_json::json!({ "to": ["\"odile@example.org\" <eve@example.net>"] }));
        // Where it goes, not only the name it shows.
        harness.get_by_label_contains("To odile@example.org <eve@example.net>, Work party on Saturday");
        harness.get_by_label_contains("Changed since it was refused: To; Edit to send it");
        only_edit_sends(&mut harness, draft);
        // Opened with Edit, its chip shows where it goes, never the name that reads as another address.
        let composer = harness.state().composer().unwrap();
        assert_eq!(composer.chips(Line::To).iter().map(crate::compose::Chip::shown).collect::<Vec<_>>(), ["eve@example.net"]);
        harness.get_by_role_and_label(Role::ListItem, "eve@example.net");
        assert!(harness.query_by_role_and_label(Role::ListItem, "odile@example.org").is_none());
        // No chip (the outbox row's own line, "To …", says both) shows or says the name that reads as an address.
        let chips: Vec<String> = harness.query_all_by_role(Role::ListItem).filter_map(|item| item.accesskit_node().label()).collect();
        assert!(chips.iter().filter(|label| !label.starts_with("To ")).all(|label| !label.contains("odile@")), "{chips:?}");
    }
}

/// A plain name keeps its chip in the composer, with the address in its tooltip and in what a screen reader hears.
#[test]
fn a_plain_name_shows_on_its_chip_with_the_address_beside_it() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Work party on Saturday");
    let chip = harness.get_by_role_and_label(Role::ListItem, "Odile Brannock, odile@example.org");
    chip.hover();
    for _ in 0..60 {
        harness.step();
    }
    harness.get_by_label("odile@example.org");
}

/// C2: an agent adds twenty Bcc and a twenty-first; the row cannot list them all, says how many it leaves out, and
/// offers no send: only Edit.
#[test]
fn a_twenty_first_bcc_is_never_sent_from_a_cut_list() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    let draft = refused(&mut harness, &clock);
    let mut bcc: Vec<String> = (0..20).map(|n| format!("Friend {n} <friend{n}@example.org>")).collect();
    bcc.push("Mallory <mallory@example.net>".to_owned());
    agent_changes(&mut harness, draft, &serde_json::json!({ "bcc": bcc }));
    let addressed = harness.state().mailbox().outbox()[0].addressed.clone();
    assert!(addressed.ends_with("and 1 more"), "{addressed:?}");
    assert!(!addressed.contains("mallory"), "{addressed:?}");
    only_edit_sends(&mut harness, draft);
}

/// I1: an agent rewrites the text of a refused send. The row never shows the text, so it offers no send: only Edit.
#[test]
fn a_refused_send_whose_text_changed_is_only_edited() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Dark);
    let draft = refused(&mut harness, &clock);
    agent_changes(&mut harness, draft, &serde_json::json!({ "text": "Words the owner never read." }));
    harness.get_by_label_contains("Changed since it was refused: text; Edit to send it");
    only_edit_sends(&mut harness, draft);
    // The composer's own Send sends it, as the composer shows it.
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert_eq!(harness.state().counting(), Some((draft, 5)), "sent from the composer, held as every send is");
}

/// A refused send that did not change keeps its Send again: it goes to exactly whom the owner already sent it to.
/// A change the window has not read yet is refused by the engine at that press.
#[test]
fn an_unchanged_refused_send_goes_again_and_a_change_not_yet_read_is_refused() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    let draft = refused(&mut harness, &clock);
    assert!(offers_send_again(&harness));
    // Changed behind the window's back: the row still offers Send again, and the press is refused.
    let update = serde_json::json!({ "id": draft, "bcc": ["Eve <eve@example.org>"] });
    let mailbox = harness.state_mut().mailbox_mut();
    mailbox.set_caller(inboxcraft_engine::Caller::Agent);
    mailbox.run("compose.update", &update).unwrap();
    mailbox.set_caller(inboxcraft_engine::Caller::Owner);
    harness.get_by_role_and_label(Role::Button, "Send “Work party on Saturday” again").click();
    harness.run();
    assert_eq!(harness.state().counting(), None, "nothing went");
    assert!(matches!(harness.state().mailbox().outbox()[0].state, SendState::Refused { .. }));
    harness.get_by_label_contains("Bcc Eve <eve@example.org>");
    assert!(!offers_send_again(&harness), "read again, the row offers only Edit");
}

/// Hostile recipients stay one clean line in the outbox row: no line break, no direction override.
#[test]
fn a_hostile_recipient_is_one_clean_line_in_the_outbox_row() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Dark);
    let draft = refused(&mut harness, &clock);
    agent_changes(&mut harness, draft, &serde_json::json!({ "bcc": ["Mal\u{202E}lory\r\nBcc: x <mallory@example.org>"] }));
    let addressed = harness.state().mailbox().outbox()[0].addressed.clone();
    assert!(!addressed.contains(['\r', '\n', '\u{202E}']), "{addressed:?}");
    assert!(addressed.starts_with("To Odile Brannock <odile@example.org> · Bcc Mallory"), "{addressed:?}");
    assert!(addressed.ends_with("<mallory@example.org>"), "{addressed:?}");
}

/// The text each painted run of the last frame shows, glyph by glyph: an ellipsis where it was cut.
fn painted_runs(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, out)),
            egui::Shape::Text(text) => out.push(text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut out));
    out
}

/// The re-look's N1: an agent puts a long address with a misleading start on a refused send. Edited, its chip shows
/// the whole domain, and the composer's Send sends it only then.
#[test]
fn a_long_address_shows_its_whole_domain_on_its_chip_before_it_is_sent() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    let draft = refused(&mut harness, &clock);
    let probe = "odile@example.org.relay.mail.attacker.example.net";
    agent_changes(&mut harness, draft, &serde_json::json!({ "to": [probe] }));
    only_edit_sends(&mut harness, draft);
    harness.run();
    let runs = painted_runs(&harness);
    assert!(runs.iter().any(|run| run == probe), "the whole address on its chip: {runs:?}");
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert_eq!(harness.state().counting(), Some((draft, 5)), "sent once its address was in sight");
}
/// The addresses stay on the chips of a send edited after it changed until the owner closes the composer; opened
/// again, the chips show the names, with the addresses in their tooltips.
#[test]
fn a_changed_refused_send_shows_addresses_until_the_composer_closes() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    let draft = refused(&mut harness, &clock);
    agent_changes(&mut harness, draft, &serde_json::json!({ "bcc": ["Béla Kis <bela@example.net>"] }));
    only_edit_sends(&mut harness, draft);
    harness.run();
    let runs = painted_runs(&harness);
    assert!(runs.iter().any(|run| run == "odile@example.org") && runs.iter().any(|run| run == "bela@example.net"), "{runs:?}");
    assert!(!runs.iter().any(|run| run == "Béla Kis" || run == "Odile Brannock"), "{runs:?}");
    harness.state_mut().close_composer();
    harness.run();
    assert!(harness.state().composer().is_none());
    harness.state_mut().reopen(draft);
    harness.run();
    let runs = painted_runs(&harness);
    assert!(runs.iter().any(|run| run == "Béla Kis"), "the name again: {runs:?}");
}
/// The re-look's N2: an agent adds twenty Bcc and a twenty-first to a refused send. Edited, the chips show their
/// addresses; the composer's Send first brings every recipient into sight and says so, and only a second press sends.
#[test]
fn the_composer_sends_only_once_every_recipient_was_in_sight() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Dark);
    let draft = refused(&mut harness, &clock);
    let mut bcc: Vec<String> = (0..20).map(|n| format!("Friend {n} <friend{n}@example.org>")).collect();
    bcc.push("Friend 20 <mallory@example.net>".to_owned());
    agent_changes(&mut harness, draft, &serde_json::json!({ "bcc": bcc }));
    only_edit_sends(&mut harness, draft);
    harness.run();
    let runs = painted_runs(&harness);
    assert!(runs.iter().any(|run| run == "friend0@example.org"), "the address, not the name: {runs:?}");
    assert!(!runs.iter().any(|run| run.starts_with("Friend ")), "{runs:?}");
    assert!(!runs.iter().any(|run| run.contains("mallory")), "the twenty-first is out of sight: {runs:?}");
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert_eq!(harness.state().counting(), None, "nothing went while a recipient was out of sight");
    let said = harness.state().mailbox().error().unwrap_or_default().to_owned();
    assert_eq!(said, "Check every recipient, then send");
    assert!(harness.query_all_by_label_contains(&said).next().is_some(), "said in view");
    assert!(painted_runs(&harness).iter().any(|run| run == "mallory@example.net"), "brought into sight");
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert_eq!(harness.state().counting(), Some((draft, 5)), "the second press sends");
}
/// An agent changes the account of the draft while the composer is open on it: the composer's Send sends from the
/// account its From line shows, not the agent's.
#[test]
fn the_composer_sends_from_the_account_it_shows() {
    let clock = clock();
    let mut harness = sending_from_both(&clock);
    write(&mut harness, "From the first");
    let draft = harness.state().composer().unwrap().draft();
    assert_eq!(harness.state().composer().unwrap().account(), 1);
    let from = harness.query_all_by_role(Role::ComboBox).filter_map(|node| node.accesskit_node().label()).find(|label| label.starts_with("From"));
    harness.state_mut().mailbox_mut().run_for_agent("compose.update", &serde_json::json!({ "id": draft, "account": 2 })).unwrap();
    harness.run();
    let shown = harness.query_all_by_role(Role::ComboBox).filter_map(|node| node.accesskit_node().label()).find(|label| label.starts_with("From"));
    assert_eq!(shown, from, "the From line still shows the first account");
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    let row = harness.state().mailbox().outbox().iter().find(|row| row.draft == draft).map(|row| row.account);
    assert_eq!(row, Some(1), "sent from the account the composer showed");
}
/// The text of each painted run of the last frame with the part of it in sight (inside its clip), and only runs with
/// some part in sight.
fn painted_places(harness: &Harness<'static, InboxApp>) -> Vec<(String, egui::Rect)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, clip, out)),
            egui::Shape::Text(text) => {
                let place = egui::Rect::from_min_size(text.pos, text.galley.size()).intersect(clip);
                if place.is_positive() {
                    out.push((text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect(), place));
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, clipped.clip_rect, &mut out));
    out
}

/// The re-look's trap: 25 Bcc on a changed refused send, edited in a short composer (the app's least window, docked,
/// and a short floating panel). Each refused press brings recipients not yet seen into sight, never under Send, and a
/// bounded number of presses ends in a send once every address was shown.
#[test]
fn a_short_composer_walks_through_every_recipient_and_then_sends() {
    let addresses: Vec<String> = (0..24).map(|n| format!("p{n}@example.org")).chain(["mallory@example.net".to_owned()]).collect();
    for (size, floating) in [(vec2(800.0, 480.0), false), (vec2(1300.0, 500.0), true)] {
        let clock = clock();
        let mut harness = sending(&clock, egui::Theme::Light);
        let draft = refused(&mut harness, &clock);
        let bcc: Vec<String> = addresses.iter().enumerate().map(|(n, address)| format!("Friend {n} <{address}>")).collect();
        agent_changes(&mut harness, draft, &serde_json::json!({ "bcc": bcc }));
        only_edit_sends(&mut harness, draft);
        if floating {
            harness.state_mut().composer_mut().0.unwrap().set_floating(true);
        }
        harness.set_size(size);
        harness.run();
        assert_eq!(harness.state().composer().map(crate::compose::Composer::floating), Some(floating));
        let mut seen = std::collections::BTreeSet::new();
        let (mut presses, mut first_unseen) = (0, None);
        while harness.state().counting().is_none() {
            assert!(presses < 26, "{size:?}: still refused after {presses} presses; seen {seen:?}");
            let send = harness.get_by_role_and_label(Role::Button, "Send").rect();
            // The grown lines leave Subject in sight over the foot.
            let subject = harness.get_by_role_and_label(Role::TextInput, "Subject").rect();
            assert!(subject.bottom() <= send.top(), "{size:?}: Subject {subject:?} pushed down to Send {send:?}");
            let painted: Vec<String> = painted_places(&harness)
                .into_iter()
                .filter(|(run, _)| addresses.contains(run))
                .map(|(run, place)| {
                    assert!(!place.intersects(send), "{size:?}: {run} at {place:?} under Send {send:?}");
                    run
                })
                .collect();
            // The press before brought the first recipient not seen yet into sight, not a later one.
            if let Some(first) = first_unseen.take() {
                assert!(painted.contains(&first), "{size:?}: press {presses} did not show {first}; painted {painted:?}");
            }
            seen.extend(painted);
            first_unseen = addresses.iter().find(|address| !seen.contains(*address)).cloned();
            harness.get_by_role_and_label(Role::Button, "Send").click();
            harness.run();
            presses += 1;
        }
        assert_eq!(harness.state().counting(), Some((draft, 5)), "{size:?}");
        assert_eq!(seen.len(), addresses.len(), "{size:?}: every address was shown before the send; seen {seen:?}");
    }
}
