// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery walked by the keyboard and left alone: every control Tab reaches, and a page that stops asking for
//! frames once nothing moves.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::gallery::{keymap, page, size};
use super::tests::{builder, keyboard_walk, painted, rests};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The gallery's controls Tab never reaches today, by label. A component task that fixes one removes its name here;
/// the walk must return exactly this list, so a fixed gap fails the test until its name goes.
const KNOWN_GAPS: &[&str] = &[];

/// The gallery in light mode, motion reduced or not.
fn gallery(reduce: bool) -> Harness<'static> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let keymap = keymap();
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), reduce);
        page(ui, &theme, &keymap);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_control_of_the_gallery_is_reached_by_tab() {
    let mut harness = gallery(true);
    let mut missed = keyboard_walk(&mut harness);
    missed.sort();
    let mut known: Vec<String> = KNOWN_GAPS.iter().map(|name| (*name).to_owned()).collect();
    known.sort();
    assert_eq!(missed, known, "the controls Tab never reaches");
}

#[test]
fn the_gallery_rests() {
    for reduce in [true, false] {
        let mut harness = gallery(reduce);
        assert!(rests(&mut harness, 8), "the gallery keeps asking for frames with motion reduced: {reduce}");
    }
}

#[test]
fn painted_finds_the_shapes_of_a_control() {
    let harness = gallery(true);
    let shapes = painted(&harness, "Unread");
    assert!(shapes.iter().any(|shape| shape.fill.a() > 0), "the chosen pill is filled: {shapes:?}");
    let rect = harness.get_by_label("Unread").rect();
    assert!(shapes.iter().all(|shape| rect.expand(1.0).contains_rect(shape.rect)), "{shapes:?} outside {rect:?}");
}

#[test]
fn the_walk_names_a_control_tab_skips_and_not_one_it_reaches() {
    let mut harness = builder().with_size(egui::vec2(300.0, 200.0)).build_ui(|ui| {
        let _ = ui.button("Reached");
        let skipped = ui.allocate_response(egui::vec2(80.0, 20.0), egui::Sense::CLICK);
        skipped.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Skipped"));
        let _ = ui.add_enabled(false, egui::Button::new("Disabled"));
    });
    harness.run();
    assert_eq!(keyboard_walk(&mut harness), vec!["Skipped".to_owned()]);
}

#[test]
fn a_page_that_asks_for_every_frame_does_not_rest() {
    let mut harness = builder().build_ui(|ui| {
        ui.label("Busy");
        ui.ctx().request_repaint();
    });
    harness.run_ok();
    assert!(!rests(&mut harness, 8));
    let mut harness = builder().build_ui(|ui| {
        ui.label("Still");
    });
    harness.run();
    assert!(rests(&mut harness, 8));
}

#[test]
fn painted_reads_fills_strokes_corners_and_dashes() {
    let harness = builder().with_size(egui::vec2(300.0, 200.0)).build_ui(|ui| {
        // Something else on the page, so the harness's own background is wider than the box.
        ui.label("A line wider than the box beside it, for the page around it");
        let (rect, response) = ui.allocate_exact_size(egui::vec2(120.0, 40.0), egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Box"));
        let colour = egui::Color32::from_rgb(10, 20, 30);
        ui.painter().rect(rect, 6.0, colour, egui::Stroke::new(1.0, egui::Color32::RED), egui::StrokeKind::Inside);
        let y = rect.center().y;
        ui.painter().extend(egui::Shape::dashed_line(
            &[egui::pos2(rect.left() + 4.0, y), egui::pos2(rect.right() - 4.0, y)],
            egui::Stroke::new(1.0, egui::Color32::BLUE),
            4.0,
            4.0,
        ));
        ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::GREEN);
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(250.0, 150.0), egui::vec2(10.0, 10.0)), 0.0, egui::Color32::RED);
    });
    let shapes = painted(&harness, "Box");
    assert_eq!(shapes.len(), 3, "{shapes:#?}");
    assert_eq!(shapes[0].fill, egui::Color32::from_rgb(10, 20, 30));
    assert!((shapes[0].radius - 6.0).abs() < f32::EPSILON);
    assert_eq!(shapes[0].stroke, Some((1.0, egui::Color32::RED, false)));
    assert_eq!(shapes[1].stroke.map(|(_, colour, dashed)| (colour, dashed)), Some((egui::Color32::BLUE, true)));
    assert!(shapes[1].rect.width() > 100.0, "the dashes are one shape: {:?}", shapes[1]);
    assert_eq!(shapes[2].fill, egui::Color32::GREEN);
    assert!((shapes[2].radius - 5.0).abs() < f32::EPSILON);
}
