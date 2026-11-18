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

#[test]
fn the_from_menu_opens_by_the_pointer_enter_and_space_and_keeps_tab_inside() {
    for key in [None, Some(Key::Enter), Some(Key::Space)] {
        let mut harness = composing(egui::Theme::Light);
        crate::component::popup::tests::walk_a_menu(&mut harness, (Role::ComboBox, "From Mira Holloway <mira@example.org>"), key);
    }
}

#[test]
fn a_recipient_that_is_not_an_address_is_named_under_its_line() {
    let mut harness = composing(egui::Theme::Light);
    type_into(&mut harness, "To", "ansel.example.org,");
    let line = harness.get_by_role_and_label(Role::TextInput, "To").rect();
    let under: Vec<egui::Rect> = harness.query_all_by_label("ansel.example.org is not an address").map(|node| node.rect()).collect();
    assert_eq!(under.len(), 1, "under the line only, not again beside Send: {under:?}");
    assert!(under.iter().any(|rect| rect.top() > line.bottom() && rect.top() < line.bottom() + 24.0), "{under:?} under {line:?}");
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
        .with_max_steps(12)
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
