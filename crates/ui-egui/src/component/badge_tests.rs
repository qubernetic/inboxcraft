// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{count, group, group_ink};
use crate::component::tests::{builder, drew_icon, theme};
use crate::iconset::{IconId, Style};
use crate::theme::{Mode, PRESETS, Theme, contrast};

#[test]
fn a_count_badge_ends_at_its_right_edge_and_grows_with_its_number() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, lefts: &mut Vec<f32>| {
            *lefts = vec![count(ui, &theme, 3, 200.0, 20.0), count(ui, &theme, 12_345, 200.0, 60.0)];
        },
        Vec::new(),
    );
    harness.run();
    let lefts = harness.state();
    assert!(lefts[0] < 200.0 && lefts[1] < lefts[0], "{lefts:?}");
}

#[test]
fn a_group_badge_is_a_named_picture_with_the_duotone_icon() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        group(ui, &theme, IconId::People, "People", 32.0);
        group(ui, &theme, IconId::Notify, "Updates", f32::NAN);
    });
    harness.run();
    let people = harness.get_by_label("People");
    assert_eq!(people.accesskit_node().role(), Role::Image);
    assert!((people.rect().width() - 32.0).abs() < 0.5);
    assert!(drew_icon(harness.output(), IconId::People, Style::Duotone));
}

#[test]
fn the_group_badge_s_icon_reads_on_every_stop_of_every_preset() {
    for preset in &PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let theme = Theme::new(mode, preset);
            let ink = group_ink(&theme);
            for stop in theme.palette.gradient {
                assert!(contrast(ink, stop) >= 3.0, "{} {mode:?}", preset.id);
            }
        }
    }
}
