// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Key, KeyboardShortcut, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::bubble;
use crate::component::button::IconButton;
use crate::component::tests::{builder, quick_tooltips, theme};
use crate::iconset::IconId;
use crate::keymap::{FIND, Keymap};

fn harness() -> Harness<'static, Keymap> {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, keymap: &mut Keymap| {
            IconButton::new(IconId::Search, "Search").action(FIND).show(ui, &theme, keymap);
            IconButton::new(IconId::Reply, "Reply").action("mail.reply").enabled(false).show(ui, &theme, keymap);
        },
        Keymap::default(),
    );
    quick_tooltips(&harness.ctx);
    harness.run();
    harness
}

#[test]
fn hovering_a_button_shows_its_name_and_key() {
    let mut harness = harness();
    assert!(harness.query_by_label("Ctrl+F").is_none(), "no tooltip before the hover");
    harness.get_by_label("Search").hover();
    harness.run();
    harness.get_by_label("Ctrl+F");
    assert_eq!(harness.query_all_by_label("Search").count(), 2, "the button and its tooltip's name");
}

#[test]
fn a_changed_key_shows_in_its_tooltip_at_once() {
    let mut harness = harness();
    harness.get_by_label("Search").hover();
    harness.run();
    harness.get_by_label("Ctrl+F");
    let changed = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::K);
    let written = harness.ctx.format_shortcut(&changed);
    harness.state_mut().bind(FIND, changed);
    harness.step();
    harness.get_by_label(&written);
    assert!(harness.query_by_label("Ctrl+F").is_none());
    harness.state_mut().unbind(FIND);
    harness.step();
    assert!(harness.query_by_label(&written).is_none(), "an action without a key shows no chip");
    assert_eq!(harness.query_all_by_label("Search").count(), 2, "the tooltip still shows the name");
}

#[test]
fn a_disabled_button_still_explains_itself() {
    let mut harness = harness();
    harness.state_mut().bind("mail.reply", KeyboardShortcut::new(Modifiers::NONE, Key::R));
    harness.get_by_label("Reply").hover();
    harness.run();
    harness.get_by_label("R");
}

#[test]
fn the_bubble_alone_names_the_action_and_its_key() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        bubble(ui, &theme, &Keymap::default(), "Search", Some(FIND));
        bubble(ui, &theme, &Keymap::default(), "Flag", None);
        bubble(ui, &theme, &Keymap::empty(), "Find", Some(FIND));
    });
    harness.run();
    harness.get_by_label("Search");
    harness.get_by_label("Flag");
    harness.get_by_label("Find");
    assert_eq!(harness.query_all_by_label("Ctrl+F").count(), 1);
}
