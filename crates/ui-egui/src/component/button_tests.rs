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
