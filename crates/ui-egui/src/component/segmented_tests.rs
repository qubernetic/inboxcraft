// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui::{Align, Layout, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{segmented, segmented_for, width};
use crate::component::tests::{builder, theme};
use crate::motion;
use crate::theme::Font;

fn options(font: Font) -> [(u8, &'static str, Font); 3] {
    [(0, "Day", font), (1, "Week", font), (2, "Month", font)]
}

#[test]
fn each_option_is_a_named_button_and_the_pressed_one_comes_back() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, current: &mut u8| {
            motion::set_reduced(ui.ctx(), true);
            if let Some(picked) = segmented(ui, &theme, &options(theme.fonts.button), *current) {
                *current = picked;
            }
        },
        1,
    );
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Week").accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Day").accesskit_node().toggled(), Some(Toggled::False));
    harness.get_by_role_and_label(Role::Button, "Month").click();
    harness.run();
    assert_eq!(*harness.state(), 2);
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Month").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn several_rows_with_the_same_options_say_what_each_is_for() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        segmented_for(ui, &theme, &options(theme.fonts.button), 0, Some("Personal"));
    });
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Week for Personal");
}

#[test]
fn the_options_stay_in_order_in_a_row_laid_out_from_the_right() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            segmented(ui, &theme, &options(theme.fonts.button), 0);
        });
    });
    harness.run();
    let day = harness.get_by_role_and_label(Role::Button, "Day").rect();
    let week = harness.get_by_role_and_label(Role::Button, "Week").rect();
    let month = harness.get_by_role_and_label(Role::Button, "Month").rect();
    assert!(day.right() <= week.left() && week.right() <= month.left(), "{day:?} {week:?} {month:?}");
}

#[test]
fn the_track_is_as_wide_as_width_says_and_an_option_keeps_its_width_when_chosen() {
    let theme = theme();
    let measure = |current: u8| {
        let mut harness = builder().build_ui(move |ui| {
            let full = width(ui, &theme, &options(theme.fonts.button));
            let before = ui.next_widget_position().x;
            segmented(ui, &theme, &options(theme.fonts.button), current);
            assert!((ui.min_rect().width() - full).abs() <= 1.0, "{} wide, {full} expected from {before}", ui.min_rect().width());
        });
        harness.run();
        harness.get_by_role_and_label(Role::Button, "Week").rect().width()
    };
    assert!((measure(0) - measure(1)).abs() < 1.0);
}

#[test]
fn in_too_little_room_the_options_share_it_and_stay_inside() {
    let theme = theme();
    for room in [160.0_f32, 24.0, 1.0] {
        let mut harness = builder().build_ui(move |ui| {
            ui.scope(|ui| {
                ui.set_max_width(room);
                segmented(ui, &theme, &options(theme.fonts.button), 1);
            });
        });
        harness.run();
        let day = harness.get_by_role_and_label(Role::Button, "Day").rect();
        let month = harness.get_by_role_and_label(Role::Button, "Month").rect();
        assert!(month.right() - day.left() <= room.max(20.0) + 4.0, "{day:?} {month:?} in {room}");
    }
}

#[test]
fn no_options_draw_nothing() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        assert_eq!(segmented::<u8>(ui, &theme, &[], 0), None);
    });
    harness.run();
}

#[test]
fn a_long_name_takes_its_room() {
    let theme = theme();
    let mut harness = builder().with_size(vec2(600.0, 80.0)).build_ui(move |ui| {
        let font = theme.fonts.button;
        segmented(ui, &theme, &[(0, "A name that is a good deal longer than any view", font), (1, "Short", font)], 1);
    });
    harness.run();
    let long = harness.get_by_role_and_label(Role::Button, "A name that is a good deal longer than any view").rect();
    let short = harness.get_by_role_and_label(Role::Button, "Short").rect();
    assert!(long.width() > short.width());
}
