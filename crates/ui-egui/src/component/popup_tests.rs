// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Id, Key, KeyboardShortcut, Modifiers, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{item, show};
use crate::component::tests::{builder, drew_icon, theme};
use crate::glass::Under;
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::motion;

/// A popup at (40, 30) with two rows; the state keeps the rows chosen and the key of Snooze.
fn harness() -> Harness<'static, (Vec<&'static str>, Keymap)> {
    let theme = theme();
    let mut keymap = Keymap::default();
    keymap.bind("mail.snooze", KeyboardShortcut::new(Modifiers::NONE, Key::S));
    let mut harness = builder().build_ui_state(
        move |ui, (chosen, keymap): &mut (Vec<&'static str>, Keymap)| {
            motion::set_reduced(ui.ctx(), true);
            let picked = show(ui.ctx(), &theme, Id::unique("menu"), pos2(40.0, 30.0), Under::Sharp, 220.0, |ui| {
                let mut picked = None;
                for (icon, name, action) in [(IconId::Snooze, "Snooze", Some("mail.snooze")), (IconId::Flag, "Flag", None)] {
                    if item(ui, &theme, keymap, icon, name, action).clicked() {
                        picked = Some(name);
                    }
                }
                picked
            });
            chosen.extend(picked);
        },
        (Vec::new(), keymap),
    );
    harness.run();
    harness
}

#[test]
fn a_popup_shows_its_rows_from_its_anchor_with_their_icons() {
    let harness = harness();
    let snooze = harness.get_by_label("Snooze");
    assert_eq!(snooze.accesskit_node().role(), Role::MenuItem);
    let rect = snooze.rect();
    assert!(rect.left() >= 40.0 && rect.top() >= 30.0 && rect.left() < 60.0, "{rect:?}");
    assert!(rect.width() > 180.0, "a row spans the popup: {rect:?}");
    assert!(drew_icon(harness.output(), IconId::Snooze, Style::Regular));
    assert!(drew_icon(harness.output(), IconId::Flag, Style::Regular));
}

#[test]
fn choosing_a_row_names_it() {
    let mut harness = harness();
    harness.get_by_label("Flag").click();
    harness.run();
    assert_eq!(harness.state().0, vec!["Flag"]);
}

#[test]
fn a_row_shows_the_key_of_its_action_as_it_is_now() {
    let mut harness = harness();
    let texts = |harness: &Harness<'_, (Vec<&'static str>, Keymap)>| {
        let mut found = Vec::new();
        for clipped in &harness.output().shapes {
            collect(&clipped.shape, &mut found);
        }
        found
    };
    assert!(texts(&harness).contains(&"S".to_owned()));
    harness.state_mut().1.bind("mail.snooze", KeyboardShortcut::new(Modifiers::NONE, Key::Z));
    harness.step();
    let now = texts(&harness);
    assert!(now.contains(&"Z".to_owned()) && !now.contains(&"S".to_owned()), "{now:?}");
}

fn collect(shape: &egui::Shape, found: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(text) => found.push(text.galley.job.text.clone()),
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, found)),
        _ => {}
    }
}

#[test]
fn the_rows_stand_one_under_the_other_even_inside_a_row() {
    let theme = theme();
    let harness = builder().build_ui(move |ui| {
        ui.horizontal(|ui| {
            super::panel(ui, &theme, Under::Fading, 200.0, |ui| {
                item(ui, &theme, &Keymap::default(), IconId::Snooze, "Snooze", None);
                item(ui, &theme, &Keymap::default(), IconId::Flag, "Flag", None);
            });
        });
    });
    let (snooze, flag) = (harness.get_by_label("Snooze").rect(), harness.get_by_label("Flag").rect());
    assert!(flag.top() >= snooze.bottom() - 0.5 && (flag.left() - snooze.left()).abs() < 0.5, "{snooze:?} {flag:?}");
    assert!((snooze.width() - 192.0).abs() < 1.0, "{snooze:?}");
}
