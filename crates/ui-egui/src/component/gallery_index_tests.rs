// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the index's rows walked by the keyboard, left alone, and read for its boards.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{nav_spoken, nav_states, page, size};
use crate::component::tests::{builder, keyboard_walk, painted, rests};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The page in `mode`, motion reduced or not.
fn harness(mode: Mode, reduce: bool) -> Harness<'static> {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), reduce);
        page(ui, &theme);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_row_of_the_index_page_is_reached_by_tab() {
    let mut harness = harness(Mode::Light, true);
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "the rows Tab never reaches: {missed:?}");
}

#[test]
fn the_index_page_rests() {
    for reduce in [true, false] {
        let mut harness = harness(Mode::Dark, reduce);
        assert!(rests(&mut harness, 8), "the page keeps asking for frames with motion reduced: {reduce}");
    }
}

#[test]
fn the_boards_of_the_index_page_stand_on_bg_with_every_state() {
    for mode in [Mode::Light, Mode::Dark] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = harness(mode, true);
        assert_eq!(harness.query_all_by_label("Place row").count(), 1, "{mode:?}: the place row's board");
        assert_eq!(painted(&harness, "Place row").ground, Some(theme.palette.bg), "{mode:?}");
        let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
        let names = nav_states().iter().map(nav_spoken).collect::<Vec<_>>();
        for name in names {
            let node = harness.get_by_label(&name);
            assert!(page.contains_rect(node.rect()), "{name} leaves the page");
        }
    }
}
