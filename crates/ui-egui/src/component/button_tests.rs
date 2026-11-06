// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::IconButton;
use crate::component::control_side;
use crate::component::tests::{builder, drew_icon, theme};
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::theme::TextScale;

/// How often each of the three buttons was pressed.
type Presses = [u32; 3];

fn harness() -> Harness<'static, Presses> {
    let theme = theme();
    let keymap = Keymap::default();
    let mut harness = builder().build_ui_state(
        move |ui, presses: &mut Presses| {
            let buttons = [
                IconButton::new(IconId::Archive, "Archive"),
                IconButton::new(IconId::Pin, "Unpin").active(true),
                IconButton::new(IconId::Reply, "Reply").enabled(false),
            ];
            for (button, count) in buttons.into_iter().zip(presses.iter_mut()) {
                if button.show(ui, &theme, &keymap).clicked() {
                    *count += 1;
                }
            }
        },
        [0; 3],
    );
    harness.run();
    harness
}

#[test]
fn an_icon_button_is_a_named_button_that_can_be_pressed() {
    let mut harness = harness();
    let archive = harness.get_by_label("Archive");
    assert_eq!(archive.accesskit_node().role(), Role::Button);
    archive.click();
    harness.run();
    assert_eq!(harness.state(), &[1, 0, 0]);
}

#[test]
fn a_disabled_button_takes_no_press_and_says_so() {
    let mut harness = harness();
    let reply = harness.get_by_label("Reply");
    assert!(reply.accesskit_node().is_disabled());
    reply.click();
    harness.run();
    assert_eq!(harness.state(), &[0, 0, 0]);
}

#[test]
fn an_active_button_is_selected_and_shows_the_fill_icon() {
    let harness = harness();
    assert_eq!(harness.get_by_label("Unpin").accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(harness.get_by_label("Archive").accesskit_node().toggled(), Some(Toggled::False));
    assert!(drew_icon(harness.output(), IconId::Pin, Style::Fill));
    assert!(drew_icon(harness.output(), IconId::Archive, Style::Regular));
}

#[test]
fn the_button_grows_with_the_text_scale() {
    let sides: Vec<f32> = [TextScale::ALL[0], TextScale::ALL[4]]
        .into_iter()
        .map(|scale| {
            let theme = theme().with_text_scale(scale);
            let mut harness = builder().build_ui_state(
                move |ui, side: &mut f32| {
                    crate::theme::install_fonts(ui.ctx());
                    *side = control_side(ui, &theme);
                    IconButton::new(IconId::Done, "Done").show(ui, &theme, &Keymap::default());
                },
                0.0,
            );
            harness.run();
            let side = *harness.state();
            let rect = harness.get_by_label("Done").rect();
            assert!((rect.width() - side).abs() < 0.5 && (rect.height() - side).abs() < 0.5, "{rect:?} {side}");
            side
        })
        .collect();
    assert!(sides[1] > sides[0] * 1.2, "{sides:?}");
}

/// Every text shape of the last frame of `output`: its text and its first glyph colour.
fn text_colours(output: &egui::FullOutput) -> Vec<(String, egui::Color32)> {
    fn open(shape: &egui::Shape, out: &mut Vec<(String, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            egui::Shape::Text(text) => {
                let colour = text.galley.rows.iter().flat_map(|row| row.visuals.mesh.vertices.iter()).map(|vertex| vertex.color).next();
                out.push((text.galley.text().to_owned(), colour.unwrap_or(text.fallback_color)));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    output.shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

#[test]
fn a_button_on_a_gradient_draws_its_icon_in_the_ink_it_is_given() {
    let theme = theme();
    let ink = egui::Color32::WHITE;
    for active in [false, true] {
        let mut harness = builder().build_ui(move |ui| {
            IconButton::new(IconId::Settings, "Settings").ink(ink).active(active).show(ui, &theme, &Keymap::default());
        });
        harness.run();
        let style = if active { Style::Fill } else { Style::Regular };
        assert!(drew_icon(harness.output(), IconId::Settings, style));
        assert!(text_colours(harness.output()).iter().all(|(_, colour)| *colour == ink), "{active}");
        assert_eq!(harness.get_by_label("Settings").accesskit_node().toggled(), Some(if active { Toggled::True } else { Toggled::False }));
    }
}

#[test]
fn a_text_button_is_a_named_button_with_a_tooltip_and_the_primary_one_reads_on_the_gradient() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, presses: &mut [u32; 2]| {
            if super::text(ui, &theme, &Keymap::default(), "Done all", None, false).clicked() {
                presses[0] += 1;
            }
            if super::text(ui, &theme, &Keymap::default(), "Review", Some(crate::keymap::FIND), true).clicked() {
                presses[1] += 1;
            }
        },
        [0; 2],
    );
    harness.run();
    let colours = text_colours(harness.output());
    let colour = |name: &str| colours.iter().find(|(text, _)| text == name).map(|(_, colour)| *colour);
    assert_eq!(colour("Done all"), Some(theme.palette.text));
    assert_eq!(colour("Review"), Some(crate::theme::ink_over(&theme.palette.gradient)));
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Done all").click();
    harness.run();
    assert_eq!(harness.state(), &[1, 1]);
    crate::component::tests::quick_tooltips(&harness.ctx);
    harness.get_by_role_and_label(Role::Button, "Review").hover();
    harness.run();
    harness.get_by_label("Ctrl+F");
}

#[test]
fn an_unavailable_button_takes_no_press_and_says_why() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, presses: &mut u32| {
            let keymap = Keymap::default();
            if IconButton::new(IconId::Snooze, "Snooze").unavailable("Not available yet").show(ui, &theme, &keymap).clicked() {
                *presses += 1;
            }
            if super::unavailable(ui, &theme, &keymap, "Reply", true, "Not yet either").clicked() {
                *presses += 1;
            }
        },
        0,
    );
    crate::component::tests::quick_tooltips(&harness.ctx);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Snooze").hover();
    harness.run();
    harness.get_by_label("Not available yet");
    harness.get_by_role_and_label(Role::Button, "Reply").hover();
    harness.run();
    harness.get_by_label("Not yet either");
    for name in ["Snooze", "Reply"] {
        let button = harness.get_by_role_and_label(Role::Button, name);
        assert!(button.accesskit_node().is_disabled(), "{name}");
        button.click();
        harness.run();
    }
    assert_eq!(*harness.state(), 0);
}
