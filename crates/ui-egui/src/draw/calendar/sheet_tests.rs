// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings shown from the calendar's surfaces, and the window's own controls over them: from the event editor with
//! typing nothing typed lands in the editor and Esc reaches neither the place nor the editor; from the floating composer
//! Send's key does nothing and nothing typed is lost; the settings' key again shows both as they were left. The
//! window's controls stand over the editor's ground and do what they do without losing the typing.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, ViewportCommand, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::Session;
use inboxcraft_engine::appearance::WindowFrame;
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

/// The calendar's week in the app's own frame (the strip with the window's controls).
fn window() -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(sending());
    app.set_own_frame(true);
    app.set_frame(WindowFrame::Strip);
    app.show_calendar();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.set_view(mailbox, View::Week);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Dark)
        .with_max_steps(40)
        .with_step_dt(1.0 / 60.0)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
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

/// Shows the settings by their key, as a person does with the keyboard in a field.
fn open_settings(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings().is_open());
}

/// Types `text` as the keyboard would, wherever the keys are.
fn type_anywhere(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.event(Event::Text(text.to_owned()));
    harness.run();
}

/// Whether the control with the keyboard lies on the settings.
fn focus_in_the_settings(harness: &Harness<'static, InboxApp>) -> bool {
    let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    let Some(focused) = harness.ctx.memory(|memory| memory.focused()) else { return false };
    harness.ctx.read_response(focused).is_some_and(|response| sheet.contains_rect(response.rect))
}

#[test]
fn the_settings_shown_from_the_editor_with_typing_take_nothing_of_it_and_its_key_comes_back_to_it() {
    let mut harness = window();
    typed(&mut harness, "Kiln");
    open_settings(&mut harness);
    type_anywhere(&mut harness, "zz");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "typing on the settings never lands in the editor");
    let mut on_page = false;
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        on_page |= focus_in_the_settings(&harness);
    }
    assert!(on_page, "Tab reaches the settings' controls");
    assert!(harness.state().settings().is_open(), "Tab walks the place, it does not leave it");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"));
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().settings().is_open(), "Esc closes no place");
    assert!(!harness.state().calendar().editor().is_some_and(|editor| editor.warned()), "the editor never saw that Esc");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(!harness.state().settings().is_open(), "the settings' key again shows the calendar");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "and the editor keeps its typing");
}

#[test]
fn the_settings_shown_from_the_floating_composer_send_nothing_and_lose_nothing() {
    let mut harness = window();
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "To").focus();
    harness.get_by_role_and_label(Role::TextInput, "To").type_text("odile@example.org,");
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.get_by_role_and_label(Role::TextInput, "Subject").type_text("Plot");
    harness.run();
    assert!(harness.state().can_run(keymap::SEND), "the keyboard is in the floating composer");
    open_settings(&mut harness);
    assert!(!harness.state().can_run(keymap::SEND), "on the settings Send's key rests");
    assert!(harness.query_by_label("Fold the draft Plot").is_none(), "the settings draw no composer");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    type_anywhere(&mut harness, "zz");
    assert!(!queued(&mut harness), "nothing sent");
    assert_eq!(subject(&harness).as_deref(), Some("Plot"), "nothing typed lands in the composer, nothing is lost");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(!harness.state().settings().is_open());
    assert_eq!(subject(&harness).as_deref(), Some("Plot"));
    assert!(harness.query_by_label("Fold the draft Plot").is_some(), "the composer is still open");
}

/// Moves the pointer to `at`, presses and releases there, and returns the viewport commands on the way.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: Pos2) -> Vec<ViewportCommand> {
    let mut seen = Vec::new();
    harness.hover_at(at);
    harness.step();
    for pressed in [true, false] {
        harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.step();
        seen.extend(harness.output().viewport_output.values().flat_map(|viewport| viewport.commands.iter().cloned()));
    }
    seen
}

#[test]
fn the_window_s_controls_stand_over_the_editor_s_ground_and_lose_no_typing() {
    let mut harness = window();
    typed(&mut harness, "Kiln");
    for (name, command) in [("Minimise", ViewportCommand::Minimized(true)), ("Close the window", ViewportCommand::Close)] {
        let control = harness.get_by_role_and_label(Role::Button, name).rect().center();
        let seen = press_at(&mut harness, control);
        harness.run();
        assert!(seen.contains(&command), "{name} does what it does over the editor: {seen:?}");
        assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "{name} keeps the typing");
    }
}
