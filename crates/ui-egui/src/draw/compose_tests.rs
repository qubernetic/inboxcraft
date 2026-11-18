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
    harness_over(sending_mailbox(), egui::Theme::Light)
}

/// The sample mail in a real session whose first account can send, without a window.
fn sending_mailbox() -> Mailbox {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock())
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
    harness.get_by_role_and_label(Role::Button, "Back to the reader's place").click();
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
    harness.get_by_label("Studio, holloway.studio@example.org").click();
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
    assert!(harness.state().counting().is_some(), "the countdown shows in place of a notice");
    harness.get_by_label("Sending “Hello” in 5 s");
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
    harness.get_by_role_and_label(Role::Button, "Back to the reader's place");
    // The keys elsewhere: on the calendar, nothing of the composer has them.
    let focused = harness.ctx.memory(|memory| memory.focused());
    if let Some(focused) = focused {
        harness.ctx.memory_mut(|memory| memory.surrender_focus(focused));
    }
    harness.run();
    assert!(!harness.state().can_run(keymap::SEND), "Send waits for the composer in the reader's place");
    // With the keys in the floating panel, Send goes from it.
    type_into(&mut harness, "Subject", "Plot");
    assert!(harness.state().can_run(keymap::SEND), "the floating panel with the keys");
    harness.get_by_role_and_label(Role::Button, "Fold the draft Plot").click();
    harness.run();
    assert!(!harness.state().can_run(keymap::SEND), "folded, Send waits for the composer to show");
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

#[test]
fn the_from_menu_opens_by_the_pointer_enter_and_space_and_keeps_tab_inside() {
    for key in [None, Some(Key::Enter), Some(Key::Space)] {
        let mut harness = composing(egui::Theme::Light);
        crate::component::popup::tests::walk_a_menu(&mut harness, (Role::ComboBox, "From Mira Holloway <mira@example.org>"), key);
    }
}

#[test]
fn a_recipient_that_is_not_an_address_is_named_beside_send() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "To", "ansel.example.org,");
    let send = harness.get_by_role_and_label(Role::Button, "Send").rect();
    let said: Vec<egui::Rect> = harness.query_all_by_label("ansel.example.org is not an address").map(|node| node.rect()).collect();
    assert_eq!(said.len(), 1, "beside Send only, not again under the line: {said:?}");
    assert!(said.iter().all(|rect| rect.left() > send.right() && (rect.center().y - send.center().y).abs() < 1.0), "{said:?} beside {send:?}");
}

#[test]
fn a_refused_paste_is_said_in_view_and_forgotten_when_the_composer_closes() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "Message text", "We meet at the plot at ten.");
    harness.event(egui::Event::Paste("x".repeat(crate::component::field::MAX_BODY + 1)));
    harness.run();
    let too_long = |harness: &Harness<'static, InboxApp>| harness.query_all(egui_kittest::kittest::by().label_contains("Too long")).count();
    assert_eq!(too_long(&harness), 1, "the composer says why the paste went nowhere");
    assert_eq!(harness.state().composer().unwrap().text(), "We meet at the plot at ten.", "nothing of it was taken");
    harness.get_by_role_and_label(Role::Button, "Close; the draft is kept").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    assert_eq!(too_long(&harness), 0, "a new composer says nothing of the last one's paste");
}

#[test]
fn a_draft_over_the_cap_opens_whole_and_is_edited_down_and_kept() {
    let mut harness = composing(egui::Theme::Light);
    let over = format!("{}.", "x".repeat(crate::component::field::MAX_BODY * 2));
    if let (Some(composer), _) = harness.state_mut().composer_mut() {
        *composer.text_mut() = over.clone();
    }
    harness.run();
    assert_eq!(harness.state().composer().unwrap().text(), over, "opened whole");
    harness.get_by_role_and_label(Role::MultilineTextInput, "Message text").focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::CTRL, Key::End);
    harness.run();
    harness.event(egui::Event::Text("y".to_owned()));
    harness.run();
    assert_eq!(harness.state().composer().unwrap().text(), over, "it grows no further");
    harness.key_press(Key::Backspace);
    harness.run();
    let shorter = &over[..over.len() - 1];
    assert_eq!(harness.state().composer().unwrap().text(), shorter, "it is edited down");
    let id = harness.state().composer().unwrap().draft();
    assert_eq!(kept(&mut harness, id)["text"].as_str().map(str::len), Some(shorter.len()), "and kept as it now is");
}

/// Frames enough for egui's undo history to keep the edit before them.
fn pause(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..90 {
        harness.step();
    }
    harness.run();
}

fn subject(harness: &Harness<'static, InboxApp>) -> String {
    harness.state().composer().map(|composer| composer.subject().to_owned()).unwrap_or_default()
}

#[test]
fn the_subject_refuses_whole_what_would_pass_998_bytes_and_undo_still_works() {
    let mut harness = composing(egui::Theme::Light);
    let start = "é".repeat(490);
    type_into(&mut harness, "Subject", &start);
    pause(&mut harness);
    harness.get_by_role_and_label(Role::TextInput, "Subject").type_text("!");
    harness.run();
    pause(&mut harness);
    let was = format!("{start}!");
    assert_eq!(subject(&harness), was);
    // 981 bytes: twenty more of two bytes, or twenty-one of one, would pass 998.
    for (case, event) in [("a paste", egui::Event::Paste("ő".repeat(10))), ("typing", egui::Event::Text("abcdefghijklmnopqrstu".to_owned()))] {
        harness.event(event);
        harness.run();
        assert_eq!(subject(&harness), was, "{case}: refused whole, nothing cut");
        assert!(harness.query_by_label_contains("Too long: this text holds at most 998 bytes").is_some(), "{case}: and said");
    }
    // What still fits is taken.
    harness.event(egui::Event::Paste("ő".repeat(8)));
    harness.run();
    assert_eq!(subject(&harness).len(), 997, "up to the cap");
    pause(&mut harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(subject(&harness), was, "Ctrl Z undoes the last edit taken");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(subject(&harness), start, "and the one before it");
}

#[test]
fn a_recipient_line_says_what_it_refused_and_keeps_it_typed() {
    let mut harness = composing(egui::Theme::Light);
    let long = format!("{}@example.org", "ő".repeat(600));
    type_into(&mut harness, "To", &format!("{long},"));
    harness.run();
    assert!(chips(&harness, Line::To).is_empty(), "no chip of it");
    assert_eq!(
        harness.state().composer().map(|composer| composer.typing(Line::To).to_owned()),
        Some(format!("{long},")),
        "kept as typed, its comma too"
    );
    // Under the line, and as the reason Send waits.
    let said = "An address is too long: it holds at most 998 bytes, and it was not added.";
    assert_eq!(harness.query_all_by_label(said).count(), 2);
}

/// Types `text` into `line` of the open composer and hands it to the composer as the field does.
fn typed_into(harness: &mut Harness<'static, InboxApp>, line: Line, text: &str) {
    let (composer, mailbox) = harness.state_mut().composer_mut();
    let composer = composer.unwrap();
    text.clone_into(composer.typing_mut(line).unwrap());
    composer.typed(mailbox, line).unwrap();
}

fn typing_of(harness: &Harness<'static, InboxApp>, line: Line) -> String {
    harness.state().composer().map(|composer| composer.typing(line).to_owned()).unwrap_or_default()
}

#[test]
fn refused_recipients_survive_close_reopen_answer_and_a_quit() {
    let mut harness = composing(egui::Theme::Light);
    let many: Vec<String> = (0..500).map(|n| format!("p{n}@example.org")).collect();
    typed_into(&mut harness, Line::To, &format!("{},", many.join(",")));
    typed_into(&mut harness, Line::To, "q1@example.org, q2@example.org; q3@example.org,");
    let long = format!("{}@example.org", "ő".repeat(600));
    typed_into(&mut harness, Line::Cc, &format!("{long},"));
    harness.run();
    let draft = harness.state().composer().unwrap().draft();
    let refused_to = typing_of(&harness, Line::To);
    let refused_cc = typing_of(&harness, Line::Cc);
    assert!(refused_to.contains("q3@example.org") && refused_cc.contains(&long));
    // A quit keeps what the store keeps: the draft holds the typed text at once.
    let shown = kept(&mut harness, draft);
    assert_eq!(shown["typed"][0].as_str(), Some(refused_to.as_str()), "the draft keeps the typed text");
    assert_eq!(shown["typed"][1].as_str(), Some(refused_cc.as_str()));
    // Close, then open it again: both lines hold their typed text and say why.
    harness.state_mut().close_composer();
    harness.run();
    assert!(harness.state().composer().is_none());
    harness.state_mut().reopen(draft);
    harness.run();
    assert_eq!(typing_of(&harness, Line::To), refused_to);
    assert_eq!(typing_of(&harness, Line::Cc), refused_cc);
    let composer = harness.state().composer().unwrap();
    assert!(composer.refusal(Line::To).is_some_and(|said| said.starts_with("3 addresses")), "{:?}", composer.refusal(Line::To));
    assert!(composer.refusal(Line::Cc).is_some(), "the long address is said again");
    assert!(!composer.sendable());
    // An answer started meanwhile closes this one: it keeps them too.
    assert!(harness.state_mut().answer(1, crate::compose::Answer::Reply));
    harness.run();
    let shown = kept(&mut harness, draft);
    assert_eq!(shown["typed"][0].as_str(), Some(refused_to.as_str()));
    // Nothing of the typed text is a recipient.
    assert!(shown["to"].as_array().unwrap().iter().all(|chip| !chip["text"].as_str().unwrap_or_default().starts_with('q')));
}

/// A kept draft to `odile@example.org`, its recipient lines' typed text written as an agent writes it
/// (`compose.update` through the control channel), and not open.
fn planted(harness: &mut Harness<'static, InboxApp>, typed: [&str; 3]) -> u64 {
    let mailbox = harness.state_mut().mailbox_mut();
    let id = mailbox.run_for_agent("compose.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    mailbox.run_for_agent("compose.update", &json!({ "id": id, "to": ["odile@example.org"], "subject": "Plot", "typed": typed })).unwrap();
    id
}

/// What the open composer shows as recipients: each drawn line's chips and the parts of its typed text.
fn shown_recipients(harness: &Harness<'static, InboxApp>) -> [Vec<String>; 3] {
    let composer = harness.state().composer().unwrap();
    Line::ALL.map(|line| {
        if harness.query_by_role_and_label(Role::TextInput, line.name()).is_none() {
            return Vec::new();
        }
        let typed = composer.typing(line).split([',', ';']).map(str::trim).filter(|part| !part.is_empty()).map(str::to_owned);
        composer.chips(line).iter().map(|chip| chip.text.clone()).chain(typed).collect()
    })
}

/// The recipients the draft `id` holds, as `mail.send` takes them.
fn kept_recipients(harness: &mut Harness<'static, InboxApp>, id: u64) -> [Vec<String>; 3] {
    let kept = kept(harness, id);
    Line::ALL.map(|line| kept[line.key()].as_array().unwrap().iter().map(|chip| chip["text"].as_str().unwrap().to_owned()).collect())
}

fn queued(harness: &mut Harness<'static, InboxApp>, id: u64) -> bool {
    let outbox = harness.state_mut().mailbox_mut().run("outbox.list", &json!({})).unwrap();
    outbox.to_string().contains(&format!("\"draft\":{id}"))
}

#[test]
fn typed_text_an_agent_left_in_bcc_is_drawn_and_sent_only_as_shown() {
    let mut harness = sending();
    let id = planted(&mut harness, ["", "", "x@example.org"]);
    harness.state_mut().reopen(id);
    harness.run();
    let bcc = harness.query_by_role_and_label(Role::TextInput, "Bcc");
    assert!(bcc.is_some(), "the Bcc line that holds typed text is drawn");
    assert_eq!(typing_of(&harness, Line::Bcc), "x@example.org");
    let shown = shown_recipients(&harness);
    assert_eq!(shown, [vec!["odile@example.org".to_owned()], Vec::new(), vec!["x@example.org".to_owned()]]);
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert!(harness.state().composer().is_none(), "sent");
    assert!(queued(&mut harness, id));
    assert_eq!(kept_recipients(&mut harness, id), shown, "what went is what was shown");
}

#[test]
fn the_owners_leftover_cc_text_comes_back_drawn_after_a_kill() {
    let mut harness = sending();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    type_into(&mut harness, "To", "odile@example.org,");
    harness.get_by_role_and_label(Role::Button, "Cc  Bcc").click();
    harness.run();
    type_into(&mut harness, "Cc", "ansel@example.org");
    harness.run();
    let id = harness.state().composer().unwrap().draft();
    assert_eq!(kept(&mut harness, id)["typed"][1].as_str(), Some("ansel@example.org"), "the store holds it");
    // A kill: the composer and the window's focus are gone without closing, and only the store's copy is left.
    harness.state_mut().composer = None;
    if let Some(focused) = harness.ctx.memory(egui::Memory::focused) {
        harness.ctx.memory_mut(|memory| memory.surrender_focus(focused));
    }
    harness.run();
    harness.state_mut().reopen(id);
    harness.run();
    assert!(harness.query_by_role_and_label(Role::TextInput, "Cc").is_some(), "the Cc line is drawn");
    assert_eq!(typing_of(&harness, Line::Cc), "ansel@example.org");
    let shown = shown_recipients(&harness);
    assert_eq!(shown, [vec!["odile@example.org".to_owned()], vec!["ansel@example.org".to_owned()], Vec::new()]);
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert!(queued(&mut harness, id));
    assert_eq!(kept_recipients(&mut harness, id), shown);
}

#[test]
fn a_refused_address_in_cc_typed_text_is_drawn_with_its_sentence() {
    let mut harness = sending();
    let long = format!("{}@example.org", "ő".repeat(600));
    let id = planted(&mut harness, ["", &format!("{long},"), ""]);
    harness.state_mut().reopen(id);
    harness.run();
    assert!(harness.query_by_role_and_label(Role::TextInput, "Cc").is_some(), "the Cc line is drawn");
    let said = "An address is too long: it holds at most 998 bytes, and it was not added.";
    assert_eq!(harness.query_all_by_label(said).count(), 2, "under the line and as the reason Send waits");
    assert!(!harness.state().composer().unwrap().sendable());
}

#[test]
fn send_refuses_typed_text_in_a_line_not_drawn_and_shows_the_line() {
    let mut harness = sending();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    type_into(&mut harness, "To", "odile@example.org,");
    let id = harness.state().composer().unwrap().draft();
    // Forced: text in Bcc while the line is not drawn, as no path of the app leaves it.
    let (composer, _) = harness.state_mut().composer_mut();
    "x@example.org".clone_into(composer.unwrap().typing_mut(Line::Bcc).unwrap());
    assert!(harness.query_by_role_and_label(Role::TextInput, "Bcc").is_none());
    harness.state_mut().send_composer();
    harness.run();
    assert!(harness.state().composer().is_some(), "not sent");
    assert!(!queued(&mut harness, id));
    assert!(kept_recipients(&mut harness, id)[2].is_empty(), "nothing became a Bcc recipient");
    assert_eq!(typing_of(&harness, Line::Bcc), "x@example.org", "nor was it dropped");
    assert!(harness.query_by_role_and_label(Role::TextInput, "Bcc").is_some(), "the line is shown now");
    let error = harness.state().mailbox().error().unwrap_or_default().to_owned();
    assert!(error.contains("Bcc"), "and Send says why: {error}");
    assert!(harness.query_all_by_label_contains(&error).next().is_some(), "in view");
}

/// The harness with the composer open over `mailbox`, a frame every 60th of a second as a window runs.
fn composing_over(mailbox: Mailbox) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        // The composer's wipe runs 300 ms: 18 frames.
        .with_max_steps(40)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(mailbox));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    harness
}

/// Types `text` into the message text a character a frame, the field keeping the keys.
fn typed_by_frames(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.get_by_role_and_label(Role::MultilineTextInput, "Message text").focus();
    harness.run();
    for c in text.chars() {
        harness.get_by_role_and_label(Role::MultilineTextInput, "Message text").type_text(&c.to_string());
        harness.step();
    }
}

fn text_of(harness: &Harness<'static, InboxApp>) -> String {
    harness.state().composer().map(|composer| composer.text().to_owned()).unwrap_or_default()
}

#[test]
fn typing_keeps_the_draft_at_most_once_a_second() {
    let mut harness = composing_over(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let id = harness.state().composer().unwrap().draft();
    let before = harness.state().mailbox().runs();
    // Thirty characters in half a second: one keep at the first, the rest wait for the second to pass.
    typed_by_frames(&mut harness, "We meet at the plot at ten. B.");
    let runs = harness.state().mailbox().runs() - before;
    assert!(runs <= 3, "{runs} commands for 30 characters typed in half a second");
    assert!(harness.state().composer().unwrap().changed(), "the rest waits");
    assert_ne!(kept(&mut harness, id)["text"].as_str().map(str::len), Some(text_of(&harness).len()));
    // A second later the draft is kept whole, the field still having the keys.
    for _ in 0..70 {
        harness.step();
    }
    assert_eq!(kept(&mut harness, id)["text"], json!(text_of(&harness)));
    assert!(!harness.state().composer().unwrap().changed());
}

#[test]
fn a_close_in_the_frame_after_a_keystroke_keeps_it() {
    let mut harness = composing_over(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let id = harness.state().composer().unwrap().draft();
    typed_by_frames(&mut harness, "Plot 14");
    // Typed within the second: the keep waits, so only Close can have kept it.
    assert!(!kept(&mut harness, id)["text"].as_str().unwrap_or_default().starts_with("Plot 14"), "not kept yet");
    harness.get_by_role_and_label(Role::Button, "Close; the draft is kept").click();
    harness.step();
    assert!(harness.state().composer().is_none());
    assert!(kept(&mut harness, id)["text"].as_str().unwrap().starts_with("Plot 14"));
}

#[test]
fn closing_the_window_in_the_frame_after_a_keystroke_keeps_it() {
    let mut harness = composing_over(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let id = harness.state().composer().unwrap().draft();
    typed_by_frames(&mut harness, "Plot 14");
    assert!(harness.state().composer().unwrap().changed(), "not kept yet");
    harness.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
    harness.step();
    assert!(kept(&mut harness, id)["text"].as_str().unwrap().starts_with("Plot 14"));
}

#[test]
fn the_end_of_the_app_in_the_frame_after_a_keystroke_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let now = Timestamp::from_unix(NOW, 0).unwrap();
    let mut store = inboxcraft_store::Sqlite::open(dir.path(), inboxcraft_store::StoreConfig::default()).unwrap();
    let mut batch = Batch::new();
    for account in sample::mailbox(now).unwrap().accounts().unwrap() {
        batch.push(Change::PutAccount(account));
    }
    store.apply(&batch).unwrap();
    let mut harness = composing_over(Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock()));
    let (draft, account) = harness.state().composer().map(|composer| (composer.draft(), composer.account())).unwrap();
    typed_by_frames(&mut harness, "Plot 14");
    assert!(harness.state().composer().unwrap().changed(), "not kept yet");
    drop(harness);
    let store = inboxcraft_store::Sqlite::open(dir.path(), inboxcraft_store::StoreConfig::default()).unwrap();
    let drafts = store.drafts(AccountId::new(account)).unwrap();
    let kept = drafts.iter().find(|held| held.id.get() == draft).unwrap();
    assert!(kept.text.starts_with("Plot 14"), "{:?}", kept.text);
}

#[test]
fn send_waits_for_a_composer_that_was_never_painted() {
    let mut harness = sending();
    let id = planted(&mut harness, ["", "", ""]);
    // The draft reopens and Send comes in the same input, before a frame painted it: nothing goes.
    harness.state_mut().reopen(id);
    harness.state_mut().send_composer();
    assert!(harness.state().composer().is_some(), "still open");
    assert!(!queued(&mut harness, id), "not sent unseen");
    // Once it was painted, Send sends it.
    harness.run();
    harness.state_mut().send_composer();
    assert!(harness.state().composer().is_none());
    assert!(queued(&mut harness, id));
}

#[test]
fn send_waits_for_a_composer_moved_to_another_place_until_painted_there() {
    let mut harness = sending();
    let id = planted(&mut harness, ["", "", ""]);
    harness.state_mut().reopen(id);
    harness.run();
    harness.state_mut().composer_mut().0.unwrap().set_floating(true);
    harness.state_mut().compose();
    harness.state_mut().send_composer();
    assert!(!queued(&mut harness, id), "back in the reader's place, unpainted there");
    harness.run();
    harness.state_mut().send_composer();
    assert!(queued(&mut harness, id));
}

fn rect_of(harness: &Harness<'static, InboxApp>, role: Role, label: &str) -> egui::Rect {
    harness.get_by_role_and_label(role, label).rect()
}

#[test]
fn send_stands_at_the_left_with_its_key() {
    let mut harness = composing(egui::Theme::Dark);
    let waiting = rect_of(&harness, Role::Button, "Send");
    type_into(&mut harness, "To", "odile@example.org,");
    harness.run();
    let send = rect_of(&harness, Role::Button, "Send");
    let text = rect_of(&harness, Role::MultilineTextInput, "Message text");
    let templates = rect_of(&harness, Role::Button, "Templates");
    let discard = rect_of(&harness, Role::Button, "Discard the draft");
    // At the left of the foot, in line with the text's box (17 out from the text: its edge and 16 in).
    assert!((send.left() - (text.left() - 17.0)).abs() < 1.0, "{send:?} {text:?}");
    assert!(send.top() > text.bottom(), "under the text");
    assert!(send.right() < templates.left() && templates.right() < discard.left(), "Templates and Discard at the right");
    assert!((discard.center().y - send.center().y).abs() < 0.5, "on one line");
    // With its key: the chip `Ctrl Enter` widens it past its name; waiting, the disabled Send of the frames has none.
    assert!(send.width() > waiting.width() + 40.0, "{send:?} has its key, {waiting:?} not");
    assert_eq!(harness.state().keymap().chip(&harness.ctx, keymap::SEND).as_deref(), Some("Ctrl Enter"));
}

#[test]
fn the_reason_stands_beside_send_when_it_waits() {
    let mut harness = composing(egui::Theme::Dark);
    let send = rect_of(&harness, Role::Button, "Send");
    let why = rect_of(&harness, Role::Label, "Add a recipient to send");
    let later = rect_of(&harness, Role::Button, "Send later");
    assert!(why.left() > later.right() && why.left() - later.right() < 16.0, "after Send later: {why:?} {later:?}");
    assert!((why.center().y - send.center().y).abs() < 0.5, "beside Send: {why:?} {send:?}");
    assert!(why.right() < rect_of(&harness, Role::Button, "Templates").left());
    type_into(&mut harness, "To", "odile@example.org,");
    harness.run();
    assert!(harness.query_by_label("Add a recipient to send").is_none(), "gone once it can go");
}

#[test]
fn the_tab_order_is_the_reading_order() {
    let mut harness = harness(egui::Theme::Dark);
    harness.state_mut().mailbox_mut().run("template.save", &json!({ "account": 1, "name": "Thanks", "text": "Thank you." })).unwrap();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    type_into(&mut harness, "To", "odile@example.org,");
    harness.get_by_role_and_label(Role::Button, "Cc  Bcc").click();
    harness.run();
    harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::field_id(Line::To)));
    harness.run();
    let mut walked = Vec::new();
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        let focused = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused());
        if let Some(label) = focused.and_then(|node| node.accesskit_node().label())
            && walked.last() != Some(&label)
        {
            walked.push(label);
        }
        if walked.last().is_some_and(|label| label == "Discard the draft") {
            break;
        }
    }
    let order = ["Cc", "Bcc", "Subject", "Message text", "Send", "Send later", "Templates", "Discard the draft"];
    let places: Vec<usize> = order.iter().map(|label| walked.iter().position(|walked| walked == label).unwrap_or(usize::MAX)).collect();
    assert!(places.windows(2).all(|pair| pair[0] < pair[1] && pair[1] != usize::MAX), "{walked:?}");
    // Tab in the text typed nothing and went on.
    assert!(!harness.state().composer().unwrap().text().contains('\t'));
}

#[test]
fn the_composer_s_tab_order_starts_at_from_and_ends_with_its_head() {
    for floating in [false, true] {
        let mut harness = composing(egui::Theme::Dark);
        if floating {
            harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
            harness.run();
        }
        harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::subject_id()));
        harness.run();
        let ours = [
            "From ",
            "To",
            "Cc",
            "Bcc",
            "Remove ",
            "Subject",
            "Message text",
            "Send",
            "Send later",
            "Templates",
            "Discard the draft",
            "Fold the draft",
            "Back to the reader",
            "Close; the draft is kept",
        ];
        let is_ours = |label: &str| ours.iter().any(|own| label.starts_with(own));
        // Twice round the window: every stop Tab reaches, in order.
        let mut walked: Vec<String> = Vec::new();
        for _ in 0..160 {
            harness.key_press(Key::Tab);
            harness.run();
            let focused = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused());
            let label = focused.and_then(|node| node.accesskit_node().label()).unwrap_or_default();
            if walked.last() != Some(&label) {
                walked.push(label);
            }
        }
        // Every time Tab comes into the composer from the rest of the window, it comes in at From.
        let entries: Vec<&String> = walked.windows(2).filter(|pair| !is_ours(&pair[0]) && is_ours(&pair[1])).map(|pair| &pair[1]).collect();
        assert!(!entries.is_empty(), "{floating}: Tab reaches the composer: {walked:?}");
        assert!(entries.iter().all(|label| label.starts_with("From ")), "{floating}: it comes in at {entries:?}");
        // And it leaves from its head, after Discard.
        let exits: Vec<&String> = walked.windows(2).filter(|pair| is_ours(&pair[0]) && !is_ours(&pair[1])).map(|pair| &pair[0]).collect();
        assert!(exits.iter().all(|label| label.as_str() == "Close; the draft is kept"), "{floating}: it leaves at {exits:?}");
        let discard = walked.iter().position(|label| label == "Discard the draft").unwrap_or(usize::MAX);
        let close = walked.iter().skip(discard).position(|label| label == "Close; the draft is kept");
        assert!(close.is_some(), "{floating}: Close after Discard: {walked:?}");
    }
}

#[test]
fn escape_in_the_suggestions_closes_them_and_keeps_the_text() {
    let mut harness = composing(egui::Theme::Dark);
    type_into(&mut harness, "To", "teo");
    assert!(!harness.state().composer().unwrap().suggestions().1.is_empty(), "suggested");
    // Under the line, at the start of its chips, 360 wide with its edge (the composer's frame).
    let from = rect_of(&harness, Role::ComboBox, "From Mira Holloway <mira@example.org>");
    let to = rect_of(&harness, Role::TextInput, "To");
    let row = harness
        .query_all_by_role(Role::ListBoxOption)
        .find(|row| row.accesskit_node().label().is_some_and(|label| label.contains("teodor@example.org")))
        .unwrap()
        .rect();
    assert!((row.left() - 1.0 - from.left()).abs() < 0.5 && (row.width() - 358.0).abs() < 0.5, "{row:?} under {from:?}");
    assert!(row.top() > to.bottom(), "{row:?} under {to:?}");
    harness.key_press(Key::Escape);
    harness.run();
    let composer = harness.state().composer().unwrap();
    assert!(composer.suggestions().1.is_empty());
    assert_eq!(composer.typing(Line::To), "teo");
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::compose::field_id(Line::To))), "the line keeps the keys");
}

#[test]
fn the_lines_stand_as_the_frame_draws_them() {
    let mut harness = composing(egui::Theme::Dark);
    type_into(&mut harness, "To", "ansel.example.org, odile@example.org,");
    harness.run();
    let from = rect_of(&harness, Role::ComboBox, "From Mira Holloway <mira@example.org>");
    let to = rect_of(&harness, Role::TextInput, "To");
    let subject = rect_of(&harness, Role::TextInput, "Subject");
    let chip = harness.query_all_by_role(Role::ListItem).next().unwrap().rect();
    // The values 76 in from the names (a column of 64 and 12), the lines 37 apart (36 and the hairline).
    assert!((chip.left() - from.left()).abs() < 0.5, "chips start under the sender: {chip:?} {from:?}");
    let left = rect_of(&harness, Role::MultilineTextInput, "Message text").left() - 17.0;
    assert!((from.left() - left - 76.0).abs() < 0.5, "{from:?} 76 in from {left}");
    assert!((subject.left() - from.left()).abs() < 0.5);
    assert!((chip.top() - (from.top() + 37.0)).abs() < 0.5, "{chip:?} {from:?}");
    assert!((subject.top() - (from.top() + 74.0 - 6.0)).abs() < 0.5, "{subject:?} {from:?}");
    assert!(to.left() > chip.right(), "the typing after the chips");
    // No box round the line: its chips stand on the composer's ground, the invalid one said beside Send.
    assert!(harness.query_by_label("ansel.example.org is not an address").is_some());
}

#[test]
fn the_floating_panel_is_520_by_560_where_its_frames_put_it_and_folds_to_its_head() {
    let mut harness = composing(egui::Theme::Dark);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let screen = harness.ctx.content_rect();
    let close = rect_of(&harness, Role::Button, "Close; the draft is kept");
    // The frames put the panel 16 in from the window's right edge and 16 over its foot (15 clear of the middle area,
    // inside the window's 1 px line); Close stands 20 in from the panel's right edge and 12 under its top.
    assert!((close.right() - (screen.right() - 16.0 - 20.0)).abs() < 0.5, "{close:?} in {screen:?}");
    assert!((close.top() - (screen.bottom() - 16.0 - 560.0 + 12.0)).abs() < 0.5, "{close:?} in {screen:?}");
    assert!(harness.query_by_role_and_label(Role::MultilineTextInput, "Message text").is_some(), "the whole composer");
    harness.get_by_role_and_label(Role::Button, "Fold the draft New message").click();
    harness.run();
    let folded = rect_of(&harness, Role::Button, "Close; the draft is kept");
    assert!((folded.top() - (screen.bottom() - 16.0 - 56.0 + 12.0)).abs() < 0.5, "one line: {folded:?}");
    assert!(harness.query_by_role_and_label(Role::MultilineTextInput, "Message text").is_none());
    assert!(harness.state().composer().unwrap().folded());
    harness.get_by_role_and_label(Role::Button, "Open the draft New message").click();
    harness.run();
    assert!(harness.query_by_role_and_label(Role::MultilineTextInput, "Message text").is_some(), "open again");
}

#[test]
fn every_control_of_the_composer_is_reached_and_it_rests_with_a_blinking_caret() {
    let mut harness = composing(egui::Theme::Dark);
    type_into(&mut harness, "To", "odile@example.org,");
    harness.get_by_role_and_label(Role::Button, "Cc  Bcc").click();
    harness.run();
    let missed: Vec<String> = crate::component::tests::keyboard_walk(&mut harness)
        .into_iter()
        // The rest of the window has its own walk; the composer's controls are reached.
        .filter(|label| {
            ["From", "To", "Cc", "Bcc", "Subject", "Message text", "Send", "Templates", "Discard", "Close", "Remove"]
                .iter()
                .any(|ours| label.starts_with(ours))
        })
        .collect();
    assert!(missed.is_empty(), "never reached: {missed:?}");
    harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::text_id()));
    harness.run();
    // The caret blinks by a frame asked for at its next change, never continuously.
    harness.run_steps(3);
    let delay = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap();
    assert!(delay >= std::time::Duration::from_millis(1) && delay <= std::time::Duration::from_millis(500), "{delay:?}");
    harness.ctx.memory_mut(|memory| memory.surrender_focus(super::compose::text_id()));
    assert!(crate::component::tests::rests(&mut harness, 6), "without the keys it rests");
}

#[test]
fn hostile_drafts_hold_their_layout() {
    let mut harness = sending();
    let mailbox = harness.state_mut().mailbox_mut();
    let id = mailbox.run_for_agent("compose.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    let subject = "ő".repeat(499);
    let chips: Vec<String> = (0..500).map(|n| format!("person{n}@example.org")).collect();
    let text = "A line of a long letter, written without end. ".repeat(23_000);
    let text = text.get(..(1 << 20) - 4096).unwrap().to_owned();
    mailbox.run_for_agent("compose.update", &json!({ "id": id, "to": chips, "subject": subject, "text": text })).unwrap();
    harness.state_mut().reopen(id);
    harness.run();
    let screen = harness.ctx.content_rect();
    let send = rect_of(&harness, Role::Button, "Send");
    let discard = rect_of(&harness, Role::Button, "Discard the draft");
    assert!(screen.contains_rect(send) && screen.contains_rect(discard), "the foot stays in the window: {send:?} {discard:?}");
    assert!(send.bottom() > screen.bottom() - 60.0, "and at its place: {send:?}");
    let text = rect_of(&harness, Role::MultilineTextInput, "Message text");
    assert!(text.top() < send.top(), "the text above the foot");
    // The foot as its frames draw it whatever the draft holds: Send at the left in line with the text's box,
    // Discard at the right.
    assert!((send.left() - (text.left() - 17.0)).abs() < 1.0 && discard.left() > send.right(), "{send:?} {discard:?} {text:?}");
    // The To line holds its 500 in four rows that scroll: the subject stays under it, in sight.
    let subject = rect_of(&harness, Role::TextInput, "Subject");
    let from = rect_of(&harness, Role::ComboBox, "From Mira Holloway <mira@example.org>");
    assert!(subject.top() - from.bottom() < 4.0 * 24.0 + 3.0 * 4.0 + 12.0 + 6.0 + 2.0 + 8.0, "{subject:?} {from:?}");
    assert!(screen.contains_rect(subject));
    assert_eq!(harness.state().composer().unwrap().chips(Line::To).len(), 500);
}

#[test]
fn the_design_ease_starts_fast_and_lands_without_overshooting() {
    let ease = crate::motion::ease;
    assert!(ease(0.0).abs() < 1e-4 && (ease(1.0) - 1.0).abs() < 1e-4);
    // cubic-bezier(.2, .7, .2, 1) at x = 0.5 is 0.92958 (worked out apart from the code).
    assert!((ease(0.5) - 0.92958).abs() < 1e-4, "{}", ease(0.5));
    let mut last = 0.0;
    for step in 0..=100 {
        let value = ease(step as f32 / 100.0);
        assert!(value >= last - 1e-6 && value <= 1.0 + 1e-6, "monotone and never past its end at {step}");
        last = value;
    }
    assert_eq!(ease(f32::NAN), 1.0);
}

/// Where the last frame painted a line of the accent's edge across the composer (its wipe's scan line), top down.
fn scan_lines(harness: &Harness<'static, InboxApp>) -> Vec<f32> {
    let edges = [crate::theme::Mode::Dark, crate::theme::Mode::Light]
        .map(|mode| crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette.cyan_edge);
    fn walk(shape: &egui::Shape, edges: &[egui::Color32], found: &mut Vec<f32>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, edges, found)),
            egui::Shape::LineSegment { points, stroke }
                if edges.contains(&stroke.color) && (points[0].y - points[1].y).abs() < 0.01 && (points[1].x - points[0].x).abs() > 500.0 =>
            {
                found.push(points[0].y);
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &edges, &mut found));
    found
}

#[test]
fn the_composer_wipes_in_over_300_ms_then_rests() {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(60)
        .with_step_dt(0.05)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock())));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.step();
    // The caret of the line that has the keys blinks on its own: without it, only the wipe asks for frames.
    harness.ctx.memory_mut(|memory| memory.surrender_focus(super::compose::field_id(Line::To)));
    harness.step();
    harness.step();
    let moving = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap();
    assert_eq!(moving, std::time::Duration::ZERO, "it moves frame by frame");
    let first = scan_lines(&harness);
    assert_eq!(first.len(), 1, "the scan line runs across the composer: {first:?}");
    harness.step();
    let second = scan_lines(&harness);
    assert!(second.len() == 1 && second[0] > first[0], "and down it: {first:?} {second:?}");
    for _ in 0..8 {
        harness.step();
    }
    assert!(scan_lines(&harness).is_empty(), "gone at the end");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests once in place");
}

#[test]
fn typing_during_the_wipe_ends_it_at_once() {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(60)
        .with_step_dt(0.02)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock())));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.step();
    harness.step();
    assert_eq!(scan_lines(&harness).len(), 1, "wiping in");
    harness.get_by_role_and_label(Role::TextInput, "To").type_text("o");
    harness.step();
    assert!(scan_lines(&harness).is_empty(), "the owner never waits");
    assert_eq!(harness.state().composer().unwrap().typing(Line::To), "o");
}

/// The sending harness with a new message written, ready to go: a recipient and a subject; its draft's id.
fn written(harness: &mut Harness<'static, InboxApp>) -> u64 {
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    type_into(harness, "To", "odile@example.org,");
    type_into(harness, "Subject", "Plot");
    let composer = harness.state().composer().unwrap();
    assert!(composer.sendable(), "ready to go");
    composer.draft()
}

/// Ctrl Enter, the composer's Send key, pressed wherever the keys are.
fn ctrl_enter(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
}

#[test]
fn ctrl_enter_in_the_calendar_event_editor_sends_nothing() {
    let mut harness = sending();
    let id = written(&mut harness);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    type_into(&mut harness, "Title", "Pottery class");
    ctrl_enter(&mut harness);
    assert!(!queued(&mut harness, id), "the floating draft did not go from the editor's field");
    assert!(harness.state().composer().is_some());
}

#[test]
fn ctrl_enter_in_the_search_field_sends_nothing() {
    let mut harness = sending();
    let id = written(&mut harness);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::search_id()));
    harness.run();
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::list::search_id())), "the search field has the keys");
    harness.event(egui::Event::Text("rota".to_owned()));
    harness.run();
    ctrl_enter(&mut harness);
    assert!(!queued(&mut harness, id), "the docked draft did not go from the search field");
    assert!(harness.state().composer().is_some());
}

#[test]
fn ctrl_enter_in_the_settings_sheet_sends_nothing() {
    let mut harness = sending();
    let id = written(&mut harness);
    harness.state_mut().open_settings();
    harness.run();
    harness.get_by_label("Sending, settings page").click();
    harness.run();
    harness.get_by_role_and_label(Role::MultilineTextInput, "Signature of Personal").focus();
    harness.run();
    harness.get_by_role_and_label(Role::MultilineTextInput, "Signature of Personal").type_text("Mira");
    harness.run();
    ctrl_enter(&mut harness);
    assert!(!queued(&mut harness, id), "the draft under the sheet did not go from its field");
    assert!(harness.state().composer().is_some());
}

#[test]
fn ctrl_enter_in_the_composer_sends_docked_and_floating() {
    let mut harness = sending();
    let id = written(&mut harness);
    ctrl_enter(&mut harness);
    assert!(queued(&mut harness, id), "docked, from its subject");
    let mut harness = sending();
    let id = written(&mut harness);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    assert!(harness.state().composer().unwrap().floating());
    type_into(&mut harness, "Subject", " work");
    ctrl_enter(&mut harness);
    assert!(queued(&mut harness, id), "floating, from its subject");
}

/// The sending sample, a frame every 60th of a second as a window runs, with the rota answered to all: the reply's id.
fn answered_to_all() -> (Harness<'static, InboxApp>, u64) {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    let mailbox = Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock());
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(60)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(mailbox));
    harness.run();
    let rota = harness.state().mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id).unwrap();
    // Opened as an act of the window does it, after a frame was drawn: the next frame is the first to paint it.
    assert!(harness.state_mut().answer(rota, crate::compose::Answer::ReplyAll));
    let id = harness.state().composer().unwrap().draft();
    (harness, id)
}

#[test]
fn reply_all_and_ctrl_enter_in_one_input_and_the_next_frame_send_nothing_unseen() {
    let (mut harness, _) = answered_to_all();
    // Back to the rota in the reader, its Reply all in sight.
    harness.state_mut().close_composer();
    let rota = harness.state().mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id);
    harness.state_mut().mailbox_mut().select(rota);
    harness.run();
    // Reply all and Ctrl Enter in one input: the reply opens after that frame was drawn.
    harness.get_by_role_and_label(Role::Button, "Reply all").click();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.step();
    let outbox = harness.state_mut().mailbox_mut().run("outbox.list", &json!({})).unwrap();
    let id = harness.state().composer().map(|composer| composer.draft());
    assert!(id.is_some(), "sent in the frame that opened it: {outbox}");
    let id = id.unwrap();
    // Ctrl Enter in the very next frame, the first to paint it.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.step();
    assert!(!queued(&mut harness, id), "sent in the frame that first painted it");
    assert!(harness.state().composer().is_some());
}

#[test]
fn send_in_the_frames_after_an_answer_waits_until_its_lines_were_seen() {
    let (mut harness, id) = answered_to_all();
    let ctx = harness.ctx.clone();
    harness.step();
    harness.step();
    // The wipe still covers the lines: they were painted, not seen.
    harness.state_mut().run_action(&ctx, keymap::SEND);
    assert!(!queued(&mut harness, id), "sent from under the wipe");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.step();
    assert!(!queued(&mut harness, id), "sent by the key that ended the wipe");
    // Seen whole in a frame before this one: Send goes.
    harness.step();
    harness.state_mut().run_action(&ctx, keymap::SEND);
    assert!(queued(&mut harness, id), "sent once seen");
}

#[test]
fn send_waits_for_a_composer_the_calendar_moved_until_painted_in_its_new_place() {
    let mut harness = sending();
    let id = written(&mut harness);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
    harness.run();
    // Back in the reader's place by a switch of screens, not yet painted there.
    harness.state_mut().run_action(&ctx, keymap::MAIL);
    harness.state_mut().send_composer();
    assert!(!queued(&mut harness, id), "sent unpainted in its new place");
    harness.run();
    harness.state_mut().send_composer();
    assert!(queued(&mut harness, id));
}

#[test]
fn fold_is_an_action_the_palette_offers_while_the_panel_floats() {
    let mut harness = composing(egui::Theme::Dark);
    let ctx = harness.ctx.clone();
    assert!(!harness.state().can_run(keymap::FOLD), "nothing to fold in the reader's place");
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    assert!(harness.state().can_run(keymap::FOLD));
    harness.state_mut().run_action(&ctx, keymap::FOLD);
    harness.run();
    assert!(harness.state().composer().unwrap().folded());
    assert!(harness.query_by_role_and_label(Role::MultilineTextInput, "Message text").is_none());
    harness.state_mut().run_action(&ctx, keymap::FOLD);
    harness.run();
    assert!(!harness.state().composer().unwrap().folded(), "open again");
    assert_eq!(keymap::action(keymap::FOLD).map(|action| action.id), Some("ui.compose.fold"));
}

/// Where the last frame painted the text `text`: its galley's rect on the screen.
fn text_at(harness: &Harness<'static, InboxApp>, text: &str) -> Option<egui::Rect> {
    fn walk(shape: &egui::Shape, text: &str, found: &mut Option<egui::Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, text, found)),
            egui::Shape::Text(shown) if shown.galley.text() == text => *found = Some(shown.galley.rect.translate(shown.pos.to_vec2())),
            _ => {}
        }
    }
    let mut found = None;
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, text, &mut found));
    found
}

#[test]
fn a_wrong_reason_stands_six_after_its_warning_as_its_frame_draws_it() {
    let mut harness = composing(egui::Theme::Dark);
    type_into(&mut harness, "To", "ansel.example.org,");
    let reason = "ansel.example.org is not an address";
    let why = rect_of(&harness, Role::Label, reason);
    let text = text_at(&harness, reason).unwrap();
    // 4 in, the warning's 16, 6: the text starts 26 into "why Send waits".
    assert!((text.left() - why.left() - 26.0).abs() < 0.5, "{text:?} in {why:?}");
}

#[test]
fn the_from_name_keeps_the_room_its_frames_give_docked_and_floating() {
    // The frames' sender names stop 246 short of the line's value docked (608 wide) and 248 floating (404 wide).
    assert_eq!(super::compose::from_name_most(608.0, false), 362.0);
    assert_eq!(super::compose::from_name_most(404.0, true), 156.0);
    assert_eq!(super::compose::from_name_most(100.0, true), 1.0, "never less than a point");
    assert_eq!(super::compose::from_name_most(f32::NAN, false), 1.0);
}

#[test]
fn the_from_menu_hangs_on_the_from_line_s_rule() {
    let mut harness = composing(egui::Theme::Dark);
    let from = rect_of(&harness, Role::ComboBox, "From Mira Holloway <mira@example.org>");
    harness.get_by_role_and_label(Role::ComboBox, "From Mira Holloway <mira@example.org>").click();
    harness.run();
    let first = rect_of(&harness, Role::MenuItemRadio, "Personal, mira@example.org");
    // The menu's top (its 1 px edge over the first row) on the hairline under the line: 6 under the value's box.
    assert!((first.top() - 1.0 - (from.bottom() + 6.0)).abs() < 0.5, "{first:?} under {from:?}");
    assert!((first.left() - 1.0 - from.left()).abs() < 0.5, "{first:?} under {from:?}");
}

/// The sample at 50 frames a second, its composer just opened from New message and drawn once: wiping in.
fn wiping() -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(60)
        .with_step_dt(0.02)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock())));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.step();
    harness.step();
    assert_eq!(scan_lines(&harness).len(), 1, "wiping in");
    harness
}

#[test]
fn a_pointer_press_during_the_wipe_ends_it_at_once() {
    let mut harness = wiping();
    let at = rect_of(&harness, Role::MultilineTextInput, "Message text").center();
    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    assert!(scan_lines(&harness).is_empty(), "a press ends it");
}

#[test]
fn the_wheel_during_the_wipe_ends_it_at_once() {
    let mut harness = wiping();
    let at = rect_of(&harness, Role::MultilineTextInput, "Message text").center();
    harness.event(egui::Event::PointerMoved(at));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -1.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    harness.step();
    assert!(scan_lines(&harness).is_empty(), "the wheel ends it");
}

#[test]
fn with_reduced_motion_the_composer_is_there_at_once() {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(60)
        .with_step_dt(0.02)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock())));
    harness.run();
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::MOTION);
    harness.run();
    assert!(crate::motion::reduced(&harness.ctx), "motion reduced");
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.step();
    harness.step();
    assert!(scan_lines(&harness).is_empty(), "no wipe");
    // Seen whole at once: Send takes it from the next frame on.
    harness.ctx.memory_mut(|memory| memory.surrender_focus(super::compose::field_id(Line::To)));
    assert!(crate::component::tests::rests(&mut harness, 2), "nothing moves");
}

#[test]
fn control_characters_and_right_to_left_text_in_a_draft_hold_the_layout() {
    // The window's fonts hold no Hebrew or Arabic: it draws a box for each, as the app does (the harness would panic).
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(12)
        .allow_missing_glyphs()
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(sending_mailbox()));
    harness.run();
    let mailbox = harness.state_mut().mailbox_mut();
    let id = mailbox.run_for_agent("compose.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    let subject = "\u{202e}gpj.exe\u{202c} \u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{627}\u{644}\u{633}\u{644}\u{627}\u{645}\u{7}\u{1b}[31m\u{0}";
    let text = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}\u{200f} line\u{0}\u{7}\u{8}\u{1b}[2J\r\n\u{202e}reversed\u{2066}isolated\n\u{feff}\u{200b}".repeat(200);
    let typed = ["\u{202e}odile@example.org\u{0}", "\u{5e9}\u{5dc}\u{5d5}\u{5dd}@example.org", "\u{7}\u{1b}"];
    // The engine may refuse a subject with control characters; what it keeps is drawn.
    let _ = mailbox.run_for_agent("compose.update", &json!({ "id": id, "subject": subject, "text": text, "typed": typed }));
    let _ = mailbox.run_for_agent("compose.update", &json!({ "id": id, "text": text, "typed": typed }));
    harness.state_mut().reopen(id);
    harness.run();
    let screen = harness.ctx.content_rect();
    for (role, label) in [(Role::ComboBox, "From Mira Holloway <mira@example.org>"), (Role::Button, "Send"), (Role::Button, "Discard the draft")] {
        let rect = rect_of(&harness, role, label);
        assert!(screen.contains_rect(rect), "{label} stays in the window: {rect:?}");
    }
    // The head's title is cut inside its row, the Close button beside it.
    let close = rect_of(&harness, Role::Button, "Close; the draft is kept");
    assert!(screen.contains_rect(close));
    assert!(harness.state().composer().unwrap().text().contains("reversed"), "the text opens whole");
    harness.key_press(Key::Tab);
    harness.run();
}

#[test]
fn hundreds_of_recipients_in_cc_and_bcc_hold_the_layout() {
    let mut harness = sending();
    let mailbox = harness.state_mut().mailbox_mut();
    let id = mailbox.run_for_agent("compose.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    let many = |line: &str| (0..500).map(|n| format!("{line}{n}@example.org")).collect::<Vec<String>>();
    mailbox.run_for_agent("compose.update", &json!({ "id": id, "to": many("to"), "cc": many("cc"), "bcc": many("bcc"), "subject": "Plot" })).unwrap();
    let clock = inboxcraft_time::SystemClock::new();
    let started = inboxcraft_time::Clock::monotonic(&clock);
    harness.state_mut().reopen(id);
    harness.run();
    assert!(inboxcraft_time::Clock::monotonic(&clock).saturating_sub(started) < std::time::Duration::from_secs(10));
    let screen = harness.ctx.content_rect();
    // Each line of 500 takes four rows of chips that scroll (4 × 24, 3 × 4 between, 6 above and below, a hairline):
    // the subject stays in sight under the three, and the foot at its place.
    let from = rect_of(&harness, Role::ComboBox, "From Mira Holloway <mira@example.org>");
    let subject = rect_of(&harness, Role::TextInput, "Subject");
    let line = 4.0 * 24.0 + 3.0 * 4.0 + 12.0 + 1.0;
    assert!((subject.top() - (from.bottom() + 6.0 + 1.0 + 3.0 * line)).abs() < 1.0, "{subject:?} {from:?}");
    assert!(screen.contains_rect(subject), "the subject in sight: {subject:?}");
    let send = rect_of(&harness, Role::Button, "Send");
    assert!(screen.contains_rect(send) && send.bottom() > screen.bottom() - 60.0, "{send:?}");
    let composer = harness.state().composer().unwrap();
    assert_eq!([Line::To, Line::Cc, Line::Bcc].map(|line| composer.chips(line).len()), [500, 500, 500]);
}

#[test]
fn many_templates_in_a_short_window_stay_in_it() {
    let mut harness = Harness::builder()
        .with_size(vec2(1100.0, 520.0))
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock())));
    harness.run();
    for n in 0..40 {
        let params = json!({ "account": 1, "name": format!("Template {n:02}"), "text": "Thank you." });
        harness.state_mut().mailbox_mut().run("template.save", &params).unwrap();
    }
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Templates").click();
    harness.run();
    let screen = harness.ctx.content_rect();
    let rows: Vec<egui::Rect> = harness.query_all_by_role(Role::MenuItem).map(|row| row.rect()).collect();
    assert!(!rows.is_empty(), "the menu is open");
    let shown: Vec<&egui::Rect> = rows.iter().filter(|row| screen.intersects(**row)).collect();
    assert!(shown.iter().all(|row| row.top() >= screen.top() + 3.0 - 0.5), "the menu keeps 3 clear of the window's top: {shown:?}");
    assert!(shown.len() < rows.len(), "the rows past the room scroll: {} of {}", shown.len(), rows.len());
}

/// The send-later picker's rect as it was last drawn.
fn later_picker(harness: &Harness<'static, InboxApp>) -> egui::Rect {
    harness.ctx.memory(|memory| memory.area_rect(super::compose::later_popup())).unwrap()
}

#[test]
fn the_send_later_picker_stands_over_the_foot_4_above_its_caret() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut harness = composing(theme);
        harness.get_by_role_and_label(Role::Button, "Send later").click();
        harness.run();
        let caret = rect_of(&harness, Role::Button, "Send later");
        let picker = later_picker(&harness);
        // Its frame: 320 wide, its left on the caret's, its bottom 4 over the foot's buttons; nothing of it over the foot.
        assert!((picker.width() - 320.0).abs() < 0.5, "{picker:?}");
        assert!((picker.left() - caret.left()).abs() < 0.5, "{picker:?} over {caret:?}");
        assert!((picker.bottom() - (caret.top() - 4.0)).abs() < 0.5, "{picker:?} over {caret:?}");
        // It stays there frame after frame.
        harness.run();
        assert_eq!(later_picker(&harness), picker);
    }
}
