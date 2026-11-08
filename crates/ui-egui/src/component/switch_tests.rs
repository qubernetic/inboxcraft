// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::switch;
use crate::component::tests::{builder, theme};
use crate::motion;

#[test]
fn a_switch_is_named_says_whether_it_is_on_and_flips_when_pressed() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, on: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            if switch(ui, &theme, "Notify me", *on).clicked() {
                *on = !*on;
            }
        },
        false,
    );
    harness.run();
    let node = harness.get_by_role_and_label(Role::Switch, "Notify me");
    assert_eq!(node.accesskit_node().toggled(), Some(Toggled::False));
    node.click();
    harness.run();
    assert!(*harness.state());
    assert_eq!(harness.get_by_role_and_label(Role::Switch, "Notify me").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn a_switch_is_wider_than_tall_and_shows_its_focus() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        let response = switch(ui, &theme, "Alert", true);
        response.request_focus();
    });
    harness.run();
    let rect = harness.get_by_role_and_label(Role::Switch, "Alert").rect();
    assert!(rect.width() > rect.height() * 1.5 && rect.height() > 10.0, "{rect:?}");
}
