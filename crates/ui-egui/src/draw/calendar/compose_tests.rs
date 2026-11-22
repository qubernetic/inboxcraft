// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The floating composer and the event editor open together on the calendar: Send's key still needs the keyboard in
//! the composer, a press on the composer while the editor holds typing only warns and loses neither, and one Esc
//! closes exactly one surface.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use crate::calendar::View;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

/// What the editor says while what was typed is not saved.
const WARNING: &str = "What you typed is not saved. Save keeps it; Cancel, or Esc again, throws it away.";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

/// The sample in a session whose first account can send, so a Send that ran would take the composer away.
fn sending() -> Mailbox {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock())
}

/// The calendar's week with the composer floating at the bottom right, its subject "Plot" and the keyboard in it.
fn both() -> Harness<'static, InboxApp> {
    both_at(vec2(1440.0, 900.0))
}

/// [`both`] in a window of `size`.
/// `app` with the small event editor, whose place beside the floating composer these tests check.
fn small_editor(mut app: InboxApp) -> InboxApp {
    app.set_event_editor_host(inboxcraft_engine::appearance::EventEditorHost::Small);
    app
}

fn both_at(size: egui::Vec2) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(size)
        .with_theme(egui::Theme::Dark)
        .with_max_steps(40)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), small_editor(InboxApp::at_mail(sending())));
    harness.run();
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
    let (calendar, mailbox) = harness.state_mut().calendar_mut();
    calendar.set_view(mailbox, View::Week);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "To").focus();
    harness.get_by_role_and_label(Role::TextInput, "To").type_text("odile@example.org,");
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.get_by_role_and_label(Role::TextInput, "Subject").type_text("Plot");
    harness.run();
    assert!(floats_open(&harness));
    assert!(harness.state().can_run(keymap::SEND), "the keyboard is in the floating composer");
    harness
}

/// Opens New event and types `text` into its title.
fn typed(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text(text);
    harness.run();
}

/// Whether the composer floats open over the calendar.
fn floats_open(harness: &Harness<'static, InboxApp>) -> bool {
    harness.state().composer().is_some_and(|composer| !composer.folded()) && harness.query_by_label("Fold the draft Plot").is_some()
}

fn summary(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.state().calendar().editor().map(|editor| editor.form().summary.clone())
}

fn subject(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.state().composer().map(|composer| composer.subject().to_owned())
}

fn queued(harness: &mut Harness<'static, InboxApp>) -> bool {
    let outbox = harness.state_mut().mailbox_mut().run("outbox.list", &json!({})).unwrap();
    outbox.to_string().contains("\"draft\"")
}

/// A press and release of the pointer at `at`, each in a frame of its own.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2) {
    harness.event(Event::PointerMoved(at));
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
}

#[test]
fn send_s_key_with_the_editor_open_needs_the_keyboard_in_the_composer() {
    let mut harness = both();
    typed(&mut harness, "Kiln");
    assert!(!harness.state().can_run(keymap::SEND), "the keyboard is in the editor's title");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    assert_eq!(subject(&harness).as_deref(), Some("Plot"), "Send's key in the editor sends nothing");
    assert!(!queued(&mut harness));
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "and the editor keeps its typing");
}

#[test]
fn a_press_on_the_composer_while_the_editor_has_typing_warns_and_loses_nothing() {
    let mut harness = both();
    typed(&mut harness, "Kiln");
    // The composer's own Send and its subject, where the editor does not cover them.
    let send = harness.get_by_role_and_label(Role::Button, "Send").rect();
    let field = harness.get_by_role_and_label(Role::TextInput, "Subject").rect();
    let (title, save) = (harness.get_by_role_and_label(Role::TextInput, "Title").rect(), harness.get_by_role_and_label(Role::Button, "Save").rect());
    let points = [send.center(), field.center()];
    let composer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("compose-floating"));
    let mut expected = String::from("Kiln");
    // The backdrop comes over the composer when the editor opens; the second round puts the composer on top, as when
    // it first shows while the editor is open.
    for round in 0..2 {
        if round == 1 {
            harness.ctx.move_to_top(composer);
            harness.run();
        }
        for at in points {
            assert!(at.x > title.right() + 16.0 || at.y > save.bottom() + 16.0, "{at:?} lies beside the editor");
            // Typing more takes the last warning away, so each press must raise its own.
            harness.get_by_role_and_label(Role::TextInput, "Title").focus();
            harness.get_by_role_and_label(Role::TextInput, "Title").type_text("!");
            harness.run();
            expected.push('!');
            assert!(harness.query_by_label(WARNING).is_none());
            press_at(&mut harness, at);
            harness.run();
            assert_eq!(summary(&harness).as_deref(), Some(expected.as_str()), "a press on the composer keeps the editor's typing");
            assert!(harness.query_by_label(WARNING).is_some(), "and warns");
            assert_eq!(subject(&harness).as_deref(), Some("Plot"), "the draft stays (round {round}, {at:?})");
            assert!(floats_open(&harness));
            assert!(!queued(&mut harness), "a press on Send beside the editor sends nothing");
            assert!(!harness.state().can_run(keymap::SEND), "the composer does not take the keyboard from the editor");
        }
    }
}

#[test]
fn with_the_composer_and_the_editor_open_one_esc_closes_exactly_one() {
    // Nothing typed: one Esc closes the editor and leaves the composer as it was.
    let mut harness = both();
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    assert!(summary(&harness).is_some());
    harness.key_press(Key::Escape);
    harness.run();
    assert!(summary(&harness).is_none(), "the editor closes");
    assert!(floats_open(&harness), "the composer stays open");
    assert_eq!(subject(&harness).as_deref(), Some("Plot"));
    // Typing: the first Esc warns, the second closes the editor, and the composer is still as it was.
    typed(&mut harness, "Kiln");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"));
    assert!(harness.query_by_label(WARNING).is_some());
    assert!(floats_open(&harness));
    harness.key_press(Key::Escape);
    harness.run();
    assert!(summary(&harness).is_none());
    assert!(floats_open(&harness), "the composer stays open");
    assert_eq!(subject(&harness).as_deref(), Some("Plot"));
    assert!(!queued(&mut harness));
}

#[test]
fn the_event_added_toast_keeps_clear_of_the_floating_composer_s_send() {
    for width in [1300.0, 1440.0, 1100.0] {
        let mut harness = both_at(vec2(width, 820.0));
        typed(&mut harness, "Kiln");
        harness.get_by_role_and_label(Role::Button, "Save").click();
        harness.run();
        assert!(harness.state().calendar().editor().is_none(), "{width}: saved");
        let said = harness.get_by_label_contains("Event added").rect();
        let undo = harness.get_by_role_and_label(Role::Button, "undo").rect();
        let toast = said.union(undo).expand2(vec2(16.0, 4.0));
        let send = harness.get_by_role_and_label(Role::Button, "Send").rect();
        assert!(!toast.intersects(send), "{width}: the toast {toast:?} over Send {send:?}");
        let events = harness.state().calendar().events().filter(|event| event.summary == "Kiln").count();
        assert_eq!(events, 1, "{width}: the event added");
        press_at(&mut harness, send.center());
        harness.run();
        assert!(queued(&mut harness), "{width}: the press on Send sent the message");
        let events = harness.state().calendar().events().filter(|event| event.summary == "Kiln").count();
        assert_eq!(events, 1, "{width}: and did not take the event back");
    }
}

#[test]
fn the_event_s_undo_stands_in_the_status_line_never_in_the_floating_composer_s_foot() {
    // At 800 and 900 the composer floats over the body; the message stands in the status line under the body, from its
    // first frame (it stood over the composer's foot as a toast).
    for width in [800.0, 900.0] {
        let mut harness = both_at(vec2(width, 820.0));
        typed(&mut harness, "Kiln");
        let send = harness.get_by_role_and_label(Role::Button, "Send").rect();
        harness.get_by_role_and_label(Role::Button, "Save").click();
        let mut seen = 0;
        for _ in 0..40 {
            harness.step();
            let Some(said) = harness.query_by_label_contains("Event added").map(|node| node.rect()) else { continue };
            let undo = harness.get_by_role_and_label(Role::Button, "undo").rect();
            let float = said.union(undo).expand2(vec2(16.0, 4.0));
            let line = crate::draw::shell::laid(&harness.ctx).unwrap().status;
            assert!(!float.intersects(send), "{width}: frame {seen}: the message {float:?} over Send {send:?}");
            assert!(said.top() >= line.top() && said.bottom() <= line.bottom(), "{width}: frame {seen}: {said:?} in the line {line:?}");
            seen += 1;
        }
        assert!(seen > 10, "{width}: the message was seen ({seen} frames)");
    }
}
