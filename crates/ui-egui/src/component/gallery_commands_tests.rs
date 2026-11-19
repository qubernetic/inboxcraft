// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the command line's sets walked by the keyboard, left alone, and read for its states.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{FIELDS, page, size};
use crate::component::command_field::spoken;
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
fn every_state_of_the_three_sets_is_on_the_page_and_tab_reaches_each() {
    let mut harness = harness(Mode::Light);
    assert_eq!(harness.query_all_by_label(spoken()).count(), FIELDS.len(), "the command field's twelve states");
    assert_eq!(harness.query_all_by_label("D done").count(), 2, "the key hint's two");
    assert_eq!(harness.query_all_by_label("Close the window").count(), 6, "the window controls' six");
    assert_eq!(harness.query_all_by_label("Restore").count(), 1, "one maximised");
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "the controls Tab never reaches: {missed:?}");
}

#[test]
fn the_command_page_rests_and_draws_the_states_colours() {
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
            [(p.term, "the field"), (p.cyan, "a scope's mark"), (p.grid, "a hover"), (p.minus, "close hovered"), (p.cyan_edge, "the caret")]
        {
            assert!(fills.contains(&colour), "{mode:?}: {what}");
        }
        let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
        let parts = [egui::accesskit::Role::Button, egui::accesskit::Role::TextInput, egui::accesskit::Role::Label];
        for node in harness.root().children_recursive().filter(|node| parts.contains(&node.accesskit_node().role())) {
            if let Some(b) = node.accesskit_node().bounding_box() {
                let rect = egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32));
                assert!(page.contains_rect(rect), "{:?} {:?} leaves the page", node.accesskit_node().label(), node.accesskit_node().value());
            }
        }
    }
}
