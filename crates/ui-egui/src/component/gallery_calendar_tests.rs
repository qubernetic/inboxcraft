// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the calendar's sets walked by the keyboard, left alone, and read for its states.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{page, size};
use crate::component::gallery::keymap;
use crate::component::tests::{builder, keyboard_walk, rests};
use crate::motion;
use crate::theme::{Mode, Palette, TextScale, Theme};

fn harness(mode: Mode) -> Harness<'static> {
    let theme = Theme::new(mode, TextScale::default());
    let keymap = keymap();
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        page(ui, &theme, &keymap);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_state_of_both_sets_is_on_the_page_and_tab_reaches_each() {
    let mut harness = harness(Mode::Light);
    for state in ["timed", "band", "chip", "row", "picked", "hover", "focus"] {
        harness.get_by_label(&format!("Event {state}"));
    }
    assert_eq!(harness.query_all_by_label("Thursday 19 November 2026").count(), 3, "the three boards of November");
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "the controls Tab never reaches: {missed:?}");
}

#[test]
fn the_calendar_page_rests_and_draws_the_states_colours() {
    for mode in [Mode::Light, Mode::Dark] {
        let mut harness = harness(mode);
        assert!(rests(&mut harness, 8));
        let p = Palette::of(mode);
        let mut fills = Vec::new();
        fn open(shape: &egui::Shape, out: &mut Vec<egui::Color32>) {
            match shape {
                egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
                egui::Shape::Rect(rect) => out.push(rect.fill),
                _ => {}
            }
        }
        harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut fills));
        for (colour, what) in
            [(p.cyan, "the picked day"), (p.cyan_edge, "today's box, the picked event's edge"), (p.stage, "hover"), (p.panel_line, "the row's line")]
        {
            assert!(fills.contains(&colour), "{mode:?}: {what}");
        }
        let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
        for node in harness.query_all_by_role(egui::accesskit::Role::Button) {
            assert!(page.contains_rect(node.rect()), "{:?} leaves the page", node.accesskit_node().label());
        }
    }
}
