// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reply, Reply all and Forward in the reader open the composer filled in; the composer's template menu puts a
//! template in, and its blanks are asked for one at a time before Send is free. Driven through the AccessKit tree.

use egui::accesskit::Role;
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::InboxApp;
use crate::compose::Line;
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

/// The subject of the sample message answered.
const ROTA: &str = "Allotment rota for November";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn harness(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
    harness.run();
    harness
}

/// The harness with Odile's rota open in the reader.
fn reading(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = harness(theme);
    harness.query_all_by_label_contains(ROTA).next().unwrap().click();
    harness.run();
    assert!(harness.state().reader().open_id().is_some());
    harness
}

fn press(harness: &mut Harness<'static, InboxApp>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

fn template(harness: &mut Harness<'static, InboxApp>, name: &str, text: &str) {
    let params = json!({ "account": 1, "name": name, "text": text });
    harness.state_mut().mailbox_mut().run("template.save", &params).unwrap();
}

#[test]
fn reply_opens_the_composer_filled_in_with_the_text_to_write_in() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = reading(theme);
        press(&mut harness, "Reply");
        let composer = harness.state().composer().unwrap();
        assert!(!composer.floating(), "in the reader's place");
        assert_eq!(composer.title(), format!("Re: {ROTA}"));
        assert!(composer.chips(Line::To)[0].text.contains("odile@example.org"));
        assert!(composer.text().contains("> Hi all"));
        harness.get_by_label(&format!("Re: {ROTA}"));
        harness.get_by_role_and_label(Role::ListItem, "Odile Brannock, odile@example.org");
        harness.run();
        assert!(harness.ctx.memory(|memory| memory.has_focus(super::compose::text_id())), "the text has the focus");
    }
}

#[test]
fn reply_all_and_forward_open_theirs_and_forward_asks_whom() {
    let mut harness = reading(egui::Theme::Light);
    press(&mut harness, "Reply all");
    assert_eq!(harness.state().composer().unwrap().title(), format!("Re: {ROTA}"));
    press(&mut harness, "Close; the draft is kept");
    harness.query_all_by_label_contains(ROTA).next().unwrap().click();
    harness.run();
    press(&mut harness, "Forward");
    let composer = harness.state().composer().unwrap();
    assert_eq!(composer.title(), format!("Fwd: {ROTA}"));
    assert!(composer.chips(Line::To).is_empty());
    harness.run();
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::compose::field_id(Line::To))), "To has the focus");
}

#[test]
fn a_reply_keeps_the_draft_that_was_being_written() {
    let mut harness = harness(egui::Theme::Light);
    press(&mut harness, "New message");
    let first = harness.state().composer().unwrap().draft();
    harness.query_all_by_label_contains(ROTA).next().unwrap().click();
    harness.run();
    assert!(harness.state().composer().unwrap().floating());
    // The floating panel stands over the reader's foot: folded, it leaves Reply in sight.
    press(&mut harness, "Fold the draft New message");
    press(&mut harness, "Reply");
    let reply = harness.state().composer().unwrap();
    assert_ne!(reply.draft(), first);
    assert!(!reply.floating());
    let drafts = harness.state_mut().mailbox_mut().run("compose.list", &json!({})).unwrap();
    assert_eq!(drafts["drafts"].as_array().map(Vec::len), Some(2), "the first draft is kept");
}

#[test]
fn without_templates_the_menu_says_there_are_none() {
    let mut harness = harness(egui::Theme::Light);
    press(&mut harness, "New message");
    let button = harness.get_by_role_and_label(Role::Button, "Templates");
    assert!(button.accesskit_node().is_disabled());
}

#[test]
fn a_template_with_a_blank_blocks_send_until_it_is_filled_in() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness(theme);
        template(&mut harness, "Work party", "We meet at the plot at {?time} and finish by noon.");
        press(&mut harness, "New message");
        harness.get_by_role_and_label(Role::TextInput, "To").type_text("odile@example.org,");
        harness.run();
        press(&mut harness, "Templates");
        harness.get_by_label("Work party").click();
        harness.run();
        assert!(harness.state().composer().unwrap().text().starts_with("We meet at the plot at [[time]] and finish by noon."));
        harness.get_by_label("Fill in \"time\" to send");
        assert!(harness.get_by_role_and_label(Role::Button, "Send").accesskit_node().is_disabled());
        let field = "Fill in \"time\"";
        harness.get_by_role_and_label(Role::TextInput, field).focus();
        harness.run();
        harness.get_by_role_and_label(Role::TextInput, field).type_text("ten");
        harness.run();
        harness.key_press(Key::Enter);
        harness.run();
        assert!(harness.state().composer().unwrap().text().starts_with("We meet at the plot at ten and finish by noon."));
        assert!(harness.query_all_by_role_and_label(Role::TextInput, field).next().is_none(), "nothing left to fill in");
        assert!(!harness.get_by_role_and_label(Role::Button, "Send").accesskit_node().is_disabled());
    }
}

#[test]
fn the_fill_in_button_takes_what_is_typed_and_asks_for_the_next_blank() {
    let mut harness = harness(egui::Theme::Light);
    template(&mut harness, "Plot", "At {?time} by {?place}.");
    press(&mut harness, "New message");
    press(&mut harness, "Templates");
    harness.get_by_label("Plot").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Fill in \"time\"").focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Fill in \"time\"").type_text("ten");
    harness.run();
    press(&mut harness, "Fill in");
    harness.get_by_role_and_label(Role::TextInput, "Fill in \"place\"");
    assert!(harness.state().composer().unwrap().text().starts_with("At ten by [[place]]."));
}

#[test]
fn the_template_menu_opens_by_the_pointer_enter_and_space_and_keeps_tab_inside() {
    for key in [None, Some(Key::Enter), Some(Key::Space)] {
        let mut harness = harness(egui::Theme::Light);
        template(&mut harness, "Thanks", "Thank you, [name].");
        template(&mut harness, "Later", "I will answer on [day].");
        press(&mut harness, "New message");
        crate::component::popup::tests::walk_a_menu(&mut harness, (Role::Button, "Templates"), key);
    }
}

/// The floating panel's rect, as it was drawn.
fn panel(harness: &Harness<'static, InboxApp>) -> egui::Rect {
    super::compose::drawn_rect(&harness.ctx).unwrap()
}

/// The reader's three answers' rects.
fn answers(harness: &Harness<'static, InboxApp>) -> Vec<egui::Rect> {
    ["Reply", "Reply all", "Forward"].iter().map(|name| harness.get_by_role_and_label(Role::Button, name).rect()).collect()
}

/// New message, then a message opened by a press: the composer floats beside the open message; `folded` folds it.
fn floating_over_a_message(folded: bool) -> Harness<'static, InboxApp> {
    let mut harness = harness(egui::Theme::Light);
    press(&mut harness, "New message");
    harness.query_all_by_label_contains(ROTA).next().unwrap().click();
    harness.run();
    assert!(harness.state().composer().unwrap().floating());
    if folded {
        press(&mut harness, "Fold the draft New message");
        assert!(harness.state().composer().unwrap().folded());
    }
    harness
}

#[test]
fn the_floating_panel_folded_or_open_never_covers_the_reader_s_answers_and_a_press_on_reply_answers() {
    for folded in [true, false] {
        let mut harness = floating_over_a_message(folded);
        let panel = panel(&harness);
        for answer in answers(&harness) {
            assert!(!answer.intersects(panel), "folded {folded}: the answer {answer:?} under the panel {panel:?}");
        }
        // 16 over the answers' row, its right edge where it stood.
        let top = answers(&harness).iter().map(|rect| rect.top()).fold(f32::INFINITY, f32::min);
        assert_eq!(panel.bottom(), (top - 16.0).floor(), "folded {folded}: the panel's foot 16 over the answers");
        let body = super::shell::laid(&harness.ctx).unwrap().body;
        assert_eq!(panel.right(), body.right() - 16.0, "folded {folded}: its right edge as before, 16 in from the body's");
        let first = harness.state().composer().unwrap().draft();
        // A pointer press on Reply, at its middle.
        let at = harness.get_by_role_and_label(Role::Button, "Reply").rect().center();
        harness.hover_at(at);
        harness.run();
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        harness.step();
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        harness.run();
        assert_ne!(harness.state().composer().unwrap().draft(), first, "folded {folded}: Reply pressed by the pointer answers");
    }
}

#[test]
fn the_floating_panel_s_shadow_falls_whole_under_its_foot() {
    for folded in [true, false] {
        let harness = floating_over_a_message(folded);
        let panel = panel(&harness);
        // The shadow reaches 12 + 3 × 16 under the box: painted, not cut, at least 40 of it.
        let reach = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Mesh(mesh) if panel.x_range().contains(mesh.calc_bounds().center().x) => {
                    Some(mesh.calc_bounds().bottom().min(clipped.clip_rect.bottom()))
                }
                _ => None,
            })
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(reach >= panel.bottom() + 40.0, "folded {folded}: the shadow painted down to {reach}, the panel's foot {}", panel.bottom());
    }
}

#[test]
fn with_no_message_open_the_floating_panel_stands_where_it_did() {
    let mut harness = harness(egui::Theme::Light);
    press(&mut harness, "New message");
    harness.state_mut().composer_mut().0.unwrap().set_floating(true);
    harness.run();
    assert!(harness.state().reader().open_id().is_none(), "nothing open");
    // 520 × 560, 16 in from the body's right and bottom edges.
    let body = super::shell::laid(&harness.ctx).unwrap().body;
    let corner = egui::pos2(body.right() - 16.0, (body.bottom() - 16.0).floor());
    assert_eq!(panel(&harness), egui::Rect::from_min_max(corner - egui::vec2(520.0, 560.0), corner));
}
