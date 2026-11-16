// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui::{Key, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::Select;
use crate::component::tests::{builder, theme};
use crate::iconset::IconId;
use crate::motion;

const OPTIONS: [(u8, &str); 3] = [(0, "Never"), (1, "Daily"), (2, "Weekly")];

/// The field's width in the test window, and what the state holds: open, and the choice.
fn harness(width: f32, options: &'static [(u8, &'static str)]) -> Harness<'static, (bool, u8)> {
    let theme = theme();
    let mut harness = builder().with_size(vec2(500.0, 400.0)).build_ui_state(
        move |ui, (open, current): &mut (bool, u8)| {
            motion::set_reduced(ui.ctx(), true);
            let shown = options.iter().find(|(value, _)| value == current).map_or("Custom", |(_, name)| name);
            let select = Select { name: "Repeat", shown, options, current: Some(*current), icon: IconId::Changed, width };
            let (_, chosen) = select.show(ui, &theme, open);
            if let Some(chosen) = chosen {
                *current = chosen;
            }
        },
        (false, 0),
    );
    harness.run();
    harness
}

#[test]
fn the_closed_field_is_a_combo_box_named_for_what_it_is_and_saying_its_choice() {
    let harness = harness(240.0, &OPTIONS);
    let field = harness.get_by_role_and_label(Role::ComboBox, "Repeat");
    assert_eq!(field.accesskit_node().value().as_deref(), Some("Never"));
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "Daily").is_none(), "closed, the choices are not there");
    assert!(field.rect().height() > 20.0);
}

#[test]
fn a_press_opens_the_choices_with_the_current_one_marked_and_taking_one_closes_them() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    assert!(harness.state().0);
    assert_eq!(harness.get_by_role_and_label(Role::MenuItemRadio, "Never").accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(harness.get_by_role_and_label(Role::MenuItemRadio, "Weekly").accesskit_node().toggled(), Some(Toggled::False));
    assert!(harness.get_by_role_and_label(Role::MenuItemRadio, "Never").is_focused(), "the keyboard starts on the current choice");
    harness.get_by_role_and_label(Role::MenuItemRadio, "Weekly").click();
    harness.run();
    assert_eq!(*harness.state(), (false, 2));
    assert_eq!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").accesskit_node().value().as_deref(), Some("Weekly"));
}

#[test]
fn escape_closes_the_choices_and_gives_the_focus_back() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(*harness.state(), (false, 0), "nothing was picked");
    assert!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").is_focused());
}

#[test]
fn a_press_beside_the_popup_closes_it_and_a_press_on_the_field_does_too() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    harness.hover_at(pos2(480.0, 380.0));
    harness.run();
    harness.drag_at(pos2(480.0, 380.0));
    harness.run();
    assert!(!harness.state().0, "a press beside it");

    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    assert!(harness.state().0);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    assert!(!harness.state().0, "the field closes what it opened");
}

#[test]
fn many_or_long_choices_stay_inside_the_window() {
    const MANY: [(u8, &str); 40] = {
        let mut rows = [(0_u8, "A rather long name for a calendar, cut where it does not fit in the popup"); 40];
        let mut index = 0;
        while index < 40 {
            rows[index].0 = index as u8;
            index += 1;
        }
        rows
    };
    let mut harness = harness(120.0, &MANY);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    let first = harness.get_all_by_role(Role::MenuItemRadio).next().map(|row| row.rect());
    assert!(first.is_some_and(|row| row.width() <= 130.0), "{first:?}");
}

#[test]
fn an_odd_width_is_no_trouble() {
    for width in [0.0, -5.0, f32::NAN, f32::INFINITY, 1.0] {
        let mut harness = harness(width, &OPTIONS);
        harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
        harness.run();
        assert!(harness.state().0);
    }
}
