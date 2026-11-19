// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of Home's sets walked by the keyboard, left alone, and read for its names, its boards and its ground.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{page, size, tile_of, tile_states};
use crate::component::count_tile::spoken;
use crate::component::gallery::keymap;
use crate::component::tests::{builder, keyboard_walk, painted, rests};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The page in `mode`, motion reduced or not.
fn harness(mode: Mode, reduce: bool) -> Harness<'static> {
    let theme = Theme::new(mode, TextScale::default());
    let keymap = keymap();
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), reduce);
        page(ui, &theme, &keymap);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_control_of_the_home_page_is_reached_by_tab() {
    let mut harness = harness(Mode::Light, true);
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "the controls Tab never reaches: {missed:?}");
}

#[test]
fn the_home_page_rests() {
    for reduce in [true, false] {
        let mut harness = harness(Mode::Light, reduce);
        assert!(rests(&mut harness, 8), "the page keeps asking for frames with motion reduced: {reduce}");
    }
}

#[test]
fn every_control_of_the_home_page_has_a_name_and_fits_the_page() {
    let harness = harness(Mode::Dark, true);
    let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
    for role in [Role::Button, Role::Label, Role::Link] {
        for node in harness.query_all_by_role(role) {
            let label = node.accesskit_node().label();
            if role != Role::Label {
                assert!(label.as_ref().is_some_and(|label| !label.is_empty()), "{role:?} without a name");
            }
            assert!(page.contains_rect(node.rect()), "{label:?} {:?} leaves the page", node.rect());
        }
    }
}

#[test]
fn the_boards_of_the_home_page_stand_on_bg_with_every_state() {
    for mode in [Mode::Light, Mode::Dark] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = harness(mode, true);
        assert!(harness.query_all_by_label("Count tile").count() >= 1, "{mode:?}: the board");
        assert_eq!(painted(&harness, "Count tile").ground, Some(theme.palette.bg), "{mode:?}");
        for state in tile_states() {
            assert!(harness.query_all_by_label(&spoken(&tile_of(&state))).count() >= 1, "{mode:?}: {}", state.name);
        }
    }
}
