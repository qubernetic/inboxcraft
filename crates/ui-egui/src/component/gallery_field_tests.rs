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
    }
}

/// The texts the last frame drew.
fn texts(harness: &Harness<'static>) -> Vec<String> {
    fn walk(shape: &egui::Shape, found: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => found.push(text.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, &mut found);
    }
    found
}

#[test]
fn the_field_pages_cells_change_under_a_still_pointer() {
    // Two pages in step, one with the pointer standing on the first board's emblem: the twinkles are alike, so what
    // differs is what the pointer flipped.
    let (mut away, mut on) = (harness(Mode::Dark), harness(Mode::Dark));
    on.hover_at(egui::pos2(300.0, 400.0));
    let mut differ = 0;
    for _ in 0..8 {
        away.step();
        on.step();
        differ += usize::from(texts(&away) != texts(&on));
    }
    assert!(differ >= 6, "the pointer changed the page in {differ} of 8 frames");
}

#[test]
fn the_place_holder_is_the_emblems_grid() {
    let emblem = placeholder();
    assert_eq!(emblem.grid(), (76, 40));
    assert_eq!(emblem.size(), egui::vec2(547.0, 500.0));
}
