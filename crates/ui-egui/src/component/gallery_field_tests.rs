// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the character field: it has no Tab stop, its emblems twinkle, and its two emblems are drawn whole.

use egui_kittest::Harness;

use super::{page, placeholder, size};
use crate::component::tests::{builder, keyboard_walk, rests};
use crate::theme::{Mode, TextScale, Theme};

fn harness(mode: Mode) -> Harness<'static> {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = builder().with_size(size()).build_ui(move |ui| page(ui, &theme));
    harness.run_ok();
    harness
}

#[test]
fn the_field_page_has_no_tab_stop_and_its_emblems_twinkle() {
    for mode in [Mode::Light, Mode::Dark] {
        let mut harness = harness(mode);
        assert!(keyboard_walk(&mut harness).is_empty());
        // The boards are Home's field at the full level: their emblems twinkle by themselves, so the page asks for
        // frames.
        harness.run_steps(20);
        assert!(!rests(&mut harness, 1), "{mode:?}: the page's emblems do not twinkle");
        // The pointer on the first board's emblem: its cells warm, and the page does not panic.
        harness.hover_at(egui::pos2(300.0, 400.0));
        harness.step();
        harness.hover_at(egui::pos2(5.0, 5.0));
        harness.run_steps(20);
    }
}

#[test]
fn the_place_holder_is_the_emblems_grid() {
    let emblem = placeholder();
    assert_eq!(emblem.grid(), (76, 40));
    assert_eq!(emblem.size(), egui::vec2(547.0, 500.0));
}
