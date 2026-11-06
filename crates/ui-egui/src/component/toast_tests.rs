// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Id, Key, KeyboardShortcut, Modifiers, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{Toast, show};
use crate::component::tests::{builder, theme};
use crate::keymap::Keymap;
use crate::motion;

const UNDO: &str = "ui.undo";

/// A toast with Undo; the state counts the undos.
fn harness(reduce: bool) -> Harness<'static, u32> {
    let theme = theme();
    let mut keymap = Keymap::default();
    keymap.bind(UNDO, KeyboardShortcut::new(Modifiers::COMMAND, Key::Z));
    let mut harness = builder().build_ui_state(
        move |ui, undone: &mut u32| {
            motion::set_reduced(ui.ctx(), reduce);
            let toast = Toast { text: "Marked as done", undo: Some(UNDO) };
            if show(ui.ctx(), &theme, &keymap, Id::unique("toast"), &toast, pos2(300.0, 400.0)) {
                *undone += 1;
            }
        },
        0,
    );
    harness.run();
    harness
}

#[test]
fn a_toast_says_what_happened_and_offers_undo_with_its_key() {
    let harness = harness(true);
    harness.get_by_label("Marked as done");
    harness.get_by_label("Undo");
    harness.get_by_label("Ctrl+Z");
    assert!(harness.get_by_label("Marked as done").rect().bottom() <= 400.5);
}

#[test]
fn undo_is_pressed_by_its_button_or_its_key() {
    let mut harness = harness(true);
    harness.get_by_label("Undo").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(*harness.state(), 2);
}

#[test]
fn a_toast_rises_into_place_and_with_reduced_motion_is_there_at_once() {
    let theme = theme();
    let mut moving = builder().build_ui_state(
        move |ui, shown: &mut bool| {
            if *shown {
                let toast = Toast { text: "Archived", undo: None };
                show(ui.ctx(), &theme, &Keymap::default(), Id::unique("toast"), &toast, pos2(300.0, 400.0));
            }
        },
        false,
    );
    *moving.state_mut() = true;
    moving.step();
    moving.step();
    let early = moving.get_by_label("Archived").rect().top();
    moving.run();
    let late = moving.get_by_label("Archived").rect().top();
    assert!(early > late + 1.0, "it rises: {early} then {late}");
    let mut still = harness(true);
    let first = still.get_by_label("Marked as done").rect().top();
    still.run();
    assert_eq!(still.get_by_label("Marked as done").rect().top(), first);
}
