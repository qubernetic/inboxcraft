// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{checkbox, radio};
use crate::component::tests::{builder, theme};
use crate::motion;

#[test]
fn a_check_box_is_named_says_whether_it_is_ticked_and_flips_when_pressed() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, on: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            if checkbox(ui, &theme, "Private", *on).clicked() {
                *on = !*on;
            }
        },
        false,
    );
    harness.run();
    let node = harness.get_by_role_and_label(Role::CheckBox, "Private");
    assert_eq!(node.accesskit_node().toggled(), Some(Toggled::False));
    node.click();
    harness.run();
    assert!(*harness.state());
    assert_eq!(harness.get_by_role_and_label(Role::CheckBox, "Private").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn a_radio_button_is_named_and_says_whether_it_is_chosen() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, choice: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            if radio(ui, &theme, "Don't tell them", !*choice).clicked() {
                *choice = false;
            }
            if radio(ui, &theme, "Send them the update", *choice).clicked() {
                *choice = true;
            }
        },
        false,
    );
    harness.run();
    let quiet = harness.get_by_role_and_label(Role::RadioButton, "Don't tell them");
    assert_eq!(quiet.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::RadioButton, "Send them the update").click();
    harness.run();
    assert!(*harness.state());
}

#[test]
fn the_name_is_cut_in_too_little_room_and_the_box_keeps_its_size() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        ui.set_max_width(60.0);
        checkbox(ui, &theme, "A calendar with a very long name indeed", true);
        checkbox(ui, &theme, "Short", false);
    });
    harness.run();
    let long = harness.get_by_role_and_label(Role::CheckBox, "A calendar with a very long name indeed").rect();
    assert!(long.width() <= 61.0 && long.height() >= 18.0, "{long:?}");
}
