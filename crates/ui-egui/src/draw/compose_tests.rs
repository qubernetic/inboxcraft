// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composer driven through its AccessKit tree: New message opens it in the reader's place, the recipient lines
//! follow the chip rules, closing keeps every word, a message pressed in the list sends it to its floating panel, and
//! Send sends.

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{InboxApp, Screen, TOAST_SECONDS};
use crate::compose::Line;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn harness_over(mailbox: Mailbox, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(mailbox));
    harness.run();
    harness
}

fn harness(theme: egui::Theme) -> Harness<'static, InboxApp> {
    harness_over(Mailbox::new(sample::session(clock()).unwrap(), clock()), theme)
}

/// The sample mail in a real session whose first account can send.
fn sending() -> Harness<'static, InboxApp> {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    harness_over(Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock()), egui::Theme::Light)
}

/// The harness with the composer open from the pane's New message.
fn composing(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = harness(theme);
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    harness
}

fn type_into(harness: &mut Harness<'static, InboxApp>, field: &str, text: &str) {
    let role = if field == "Message text" { Role::MultilineTextInput } else { Role::TextInput };
    harness.get_by_role_and_label(role, field).focus();
    harness.run();
    harness.get_by_role_and_label(role, field).type_text(text);
    harness.run();
}

fn chips(harness: &Harness<'static, InboxApp>, line: Line) -> Vec<String> {
    harness.state().composer().map(|composer| composer.chips(line).iter().map(|chip| chip.text.clone()).collect()).unwrap_or_default()
}

fn kept(harness: &mut Harness<'static, InboxApp>, id: u64) -> Value {
    harness.state_mut().mailbox_mut().run("compose.show", &json!({ "id": id })).unwrap()
}

#[test]
fn new_message_opens_the_composer_in_the_readers_place() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = composing(theme);
        let composer = harness.state().composer().unwrap();
        assert!(!composer.floating());
        harness.get_by_role_and_label(Role::Label, "New message");
        harness.get_by_role_and_label(Role::ComboBox, "From Mira Holloway <mira@example.org>");
        for field in ["To", "Subject"] {
            harness.get_by_role_and_label(Role::TextInput, field);
        }
        harness.get_by_role_and_label(Role::MultilineTextInput, "Message text");
        assert!(harness.query_all_by_role_and_label(Role::TextInput, "Cc").next().is_none(), "Cc waits for Cc Bcc");
        assert!(harness.get_by_role_and_label(Role::Button, "Send").accesskit_node().is_disabled(), "no recipient yet");
        harness.get_by_label("Add a recipient to send");
        harness.run();
        assert!(harness.ctx.memory(|memory| memory.has_focus(super::compose::field_id(Line::To))), "To has the focus");
        harness.get_by_role_and_label(Role::Button, "Cc  Bcc").click();
        harness.run();
        harness.get_by_role_and_label(Role::TextInput, "Cc");
        harness.get_by_role_and_label(Role::TextInput, "Bcc");
    }
}

#[test]
fn chip_rules_enter_a_comma_a_highlighted_suggestion_backspace_and_remove() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "To", "teodor@example.org");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(chips(&harness, Line::To), ["teodor@example.org"], "Enter takes what was typed");
    harness.get_by_role_and_label(Role::ListItem, "teodor@example.org");
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::compose::field_id(Line::To))), "the field keeps the focus");

    type_into(&mut harness, "To", "ansel.example.org,");
    assert_eq!(chips(&harness, Line::To), ["teodor@example.org", "ansel.example.org"], "a comma ends a recipient");
    harness.get_by_role_and_label(Role::ListItem, "ansel.example.org, not an address");
    harness.get_by_label("ansel.example.org is not an address");
    assert!(harness.get_by_role_and_label(Role::Button, "Send").accesskit_node().is_disabled(), "an impossible address blocks Send");

    type_into(&mut harness, "To", "odi");
    harness.get_by_label("Odile Brannock <odile@example.org>");
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().composer().unwrap().highlighted(), Some(0));
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(chips(&harness, Line::To)[2], "Odile Brannock <odile@example.org>", "Enter takes the highlighted suggestion, not the text");
    harness.get_by_role_and_label(Role::ListItem, "Odile Brannock, odile@example.org");
    assert!(harness.query_all_by_label("Odile Brannock <odile@example.org>").next().is_none(), "the suggestions went");

    harness.key_press(Key::Backspace);
    harness.run();
    assert_eq!(chips(&harness, Line::To), ["teodor@example.org", "ansel.example.org"], "Backspace in the empty line removes the last chip");
    harness.get_by_role_and_label(Role::Button, "Remove ansel.example.org").click();
    harness.run();
    assert_eq!(chips(&harness, Line::To), ["teodor@example.org"]);
    assert!(!harness.get_by_role_and_label(Role::Button, "Send").accesskit_node().is_disabled(), "nothing blocks Send now");
    let id = harness.state().composer().unwrap().draft();
    assert_eq!(kept(&mut harness, id)["to"], json!([{ "text": "teodor@example.org", "ok": true }]), "every chip is kept at once");
}

#[test]
fn a_pressed_suggestion_is_taken_and_escape_puts_them_away() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "To", "pet");
    harness.get_by_label("Petra Harbour <petra@example.org>").click();
    harness.run();
    assert_eq!(chips(&harness, Line::To), ["Petra Harbour <petra@example.org>"]);
    type_into(&mut harness, "To", "odi");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().composer().unwrap().suggestions().1, []);
    assert_eq!(harness.state().composer().unwrap().typing(Line::To), "odi", "Escape keeps what was typed");
    assert!(harness.state().composer().is_some(), "Escape does not close the composer");
}

#[test]
fn closing_the_composer_loses_no_text() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "To", "teodor@example.org");
    type_into(&mut harness, "Subject", "Work party on Saturday");
    type_into(&mut harness, "Message text", "We meet at the plot at ten.");
    let id = harness.state().composer().unwrap().draft();
    let saved = kept(&mut harness, id);
    assert_eq!(saved["subject"], json!("Work party on Saturday"), "kept as it is typed");
    assert_eq!(saved["text"], json!("We meet at the plot at ten."));
    harness.get_by_role_and_label(Role::Button, "Close; the draft is kept").click();
    harness.run();
    assert!(harness.state().composer().is_none());
    harness.get_by_label("Draft saved");
    let saved = kept(&mut harness, id);
    assert_eq!(saved["to"][0]["text"], json!("teodor@example.org"), "what was typed in To became a chip");
    assert_eq!(saved["subject"], json!("Work party on Saturday"));
    assert_eq!(saved["text"], json!("We meet at the plot at ten."));
}

#[test]
fn a_message_pressed_in_the_list_floats_the_composer_and_it_comes_back_whole() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "Message text", "Half a thought");
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    assert!(harness.state().composer().unwrap().floating());
    assert!(harness.state().reader().open_id().is_some(), "the reader has its place back");
    harness.get_by_role_and_label(Role::Button, "Open the draft New message").click();
    harness.run();
    assert!(!harness.state().composer().unwrap().floating());
    assert_eq!(harness.state().composer().unwrap().text(), "Half a thought");
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    assert!(harness.state().composer().unwrap().floating(), "the message selected under the composer floats it again");
    harness.get_by_role_and_label(Role::Button, "Close; the draft is kept").click();
    harness.run();
    assert!(harness.state().composer().is_none());
}

#[test]
fn the_keys_on_a_message_rest_while_the_composer_hides_it() {
    let mut harness = harness(egui::Theme::Light);
    harness.key_press(Key::ArrowDown);
    harness.run();
    let selected = harness.state().mailbox().selected();
    harness.key_press(Key::C);
    harness.run();
    assert!(harness.state().composer().is_some(), "C writes a new message");
    harness.ctx.memory_mut(|memory| memory.surrender_focus(super::compose::field_id(Line::To)));
    harness.run();
    harness.key_press(Key::D);
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert!(harness.state().mailbox().undoable().is_none(), "D did nothing to the hidden message");
    assert_eq!(harness.state().mailbox().selected(), selected);
    assert_eq!(harness.state().composer().unwrap().typing(Line::To), "");
}

#[test]
fn the_from_menu_changes_the_account_and_its_signature() {
    let mut harness = harness(egui::Theme::Light);
    harness.state_mut().mailbox_mut().run("compose.signature", &json!({ "account": 2, "signature": "Mira\nStudio" })).unwrap();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    harness.get_by_role_and_label(Role::ComboBox, "From Mira Holloway <mira@example.org>").click();
    harness.run();
    harness.get_by_label("Studio (holloway.studio@example.org)").click();
    harness.run();
    let composer = harness.state().composer().unwrap();
    assert_eq!(composer.account(), 2);
    assert!(composer.text().ends_with("-- \nMira\nStudio"));
    harness.get_by_role_and_label(Role::ComboBox, "From Mira Holloway <holloway.studio@example.org>");
}

#[test]
fn send_sends_and_the_toast_says_so() {
    let mut harness = sending();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    type_into(&mut harness, "To", "odile@example.org");
    type_into(&mut harness, "Subject", "Hello");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    assert!(harness.state().composer().is_none());
    assert_eq!(harness.state().notice(), Some("Sending your message"));
    harness.get_by_label("Sending your message");
}

#[test]
fn the_window_asks_for_a_frame_when_a_held_send_is_due() {
    let mut harness = sending();
    harness.state_mut().mailbox_mut().run("send.set", &json!({ "hold_seconds": 30 })).unwrap();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    type_into(&mut harness, "To", "odile@example.org");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    assert!(harness.state().composer().is_none(), "sent");
    // Nothing else draws a frame in an idle window: it asks for one when the hold is over.
    let ctx = egui::Context::default();
    let delay = |wake: &mut dyn FnMut(&egui::Context)| {
        let mut output = ctx.run_pass(egui::RawInput::default(), |ctx| wake(ctx));
        output.textures_delta.clear();
        output.viewport_output[&egui::ViewportId::ROOT].repaint_delay
    };
    // A new context asks for frames of its own at first.
    while delay(&mut |_| {}).is_zero() {}
    assert_eq!(delay(&mut |ctx| harness.state_mut().wake_when_due(ctx)), std::time::Duration::from_secs(30));
}

#[test]
fn a_send_the_sample_refuses_keeps_the_composer_and_says_why() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "To", "odile@example.org,");
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert!(harness.state().composer().is_some());
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("submission")));
}

#[test]
fn discard_removes_the_draft_and_new_message_starts_afresh() {
    let mut harness = composing(egui::Theme::Light);
    let id = harness.state().composer().unwrap().draft();
    harness.get_by_role_and_label(Role::Button, "Discard the draft").click();
    harness.run();
    assert!(harness.state().composer().is_none());
    harness.get_by_label("Draft discarded");
    assert!(harness.state_mut().mailbox_mut().run("compose.show", &json!({ "id": id })).is_err());
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    assert_ne!(harness.state().composer().unwrap().draft(), id);
}

#[test]
fn the_composer_floats_over_the_calendar_and_new_message_brings_it_back() {
    let mut harness = composing(egui::Theme::Light);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Calendar);
    harness.get_by_role_and_label(Role::Button, "Open the draft New message");
    assert!(!harness.state().can_run(keymap::SEND), "Send waits for the composer in the reader's place");
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Mail);
    assert!(harness.state().can_run(keymap::SEND));
}

#[test]
fn the_toast_of_a_step_goes_after_its_time() {
    let mut harness = composing(egui::Theme::Light);
    harness.get_by_role_and_label(Role::Button, "Close; the draft is kept").click();
    harness.run();
    harness.get_by_label("Draft saved");
    harness.step();
    let later = harness.ctx.input(|input| input.time) + TOAST_SECONDS + 1.0;
    while harness.ctx.input(|input| input.time) < later {
        harness.step();
    }
    harness.run();
    assert_eq!(harness.state().notice(), None);
}

#[test]
fn without_an_account_there_is_no_new_message() {
    let store = inboxcraft_store::Memory::new(inboxcraft_store::StoreConfig::default());
    let harness = harness_over(Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock()), egui::Theme::Light);
    assert!(!harness.state().can_run(keymap::COMPOSE));
    assert!(harness.query_all_by_role_and_label(Role::Button, "New message").next().is_none());
}
