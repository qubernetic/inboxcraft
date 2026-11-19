// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the character field: it has no Tab stop, rests, and its two emblems are drawn whole.

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
fn the_field_page_has_no_tab_stop_and_rests() {
    for mode in [Mode::Light, Mode::Dark] {
        let mut harness = harness(mode);
        assert!(keyboard_walk(&mut harness).is_empty());
        assert!(rests(&mut harness, 8), "{mode:?}: the page keeps asking for frames");
        // The pointer on the first board's emblem: its cells warm, cool once it leaves, and the page rests again.
        harness.hover_at(egui::pos2(300.0, 400.0));
        harness.step();
        harness.hover_at(egui::pos2(5.0, 5.0));
        assert!(rests(&mut harness, 20), "{mode:?}: the page does not rest after the pointer left");
    }
}

#[test]
fn the_place_holder_is_the_emblems_grid() {
    let emblem = placeholder();
    assert_eq!(emblem.grid(), (76, 40));
    assert_eq!(emblem.size(), egui::vec2(547.0, 500.0));
}
