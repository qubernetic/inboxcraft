// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery walked by the keyboard and left alone: every control Tab reaches, and a page that stops asking for
//! frames once nothing moves.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT as _;
use egui_kittest::kittest::Queryable as _;

use super::button::IconButton;
use super::gallery::{keymap, page, size};
use super::tests::{Look, builder, keyboard_walk, painted, rests, stage};
use crate::iconset::IconId;
use crate::keymap::Keymap;
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
    // The coming-soon board is in the walk's tree, each control disabled (so neither reached nor a gap), and its
    // tag shows on it.
    for (role, label) in [(Role::Button, "Export"), (Role::Switch, "Sync contacts"), (Role::MenuItem, "Scheduled messages")] {
        assert!(harness.get_by_role_and_label(role, label).accesskit_node().is_disabled(), "{label} is coming soon");
    }
    // The tag once on the coming-soon board, and once on the tag's board as one of its states.
    assert_eq!(harness.query_all_by_label("coming soon").count(), 2, "the tag once on each board");
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
    let paint = painted(&harness, "Unread");
    let rect = harness.get_by_label("Unread").rect();
    let fill = paint.rects().find_map(|shape| match shape.look {
        Look::Rect { fill, radius, .. } if shape.inside && fill.a() > 0 => Some((fill, radius)),
        _ => None,
    });
    let (fill, radius) = fill.unwrap_or_else(|| panic!("the chosen pill is filled: {paint:?}"));
    assert!(radius.abs() < f32::EPSILON, "the pill is square");
    assert!(paint.ground.is_some_and(|ground| ground != fill), "the pill stands out from its card: {paint:?}");
    let muted = Theme::new(Mode::Light, TextScale::default()).palette.muted;
    let edge = painted(&harness, "Flagged");
    assert!(
        edge.rects().any(|shape| shape.inside
            && shape.look
                == Look::Rect {
                    fill: egui::Color32::TRANSPARENT,
                    stroke: Some((1.0, muted)),
                    stroke_kind: egui::StrokeKind::Inside,
                    radius: 0.0,
                    dashed: false
                }),
        "an unchosen pill has a 1 px muted edge inside: {edge:#?}"
    );
    assert!(paint.shapes.iter().any(|shape| matches!(&shape.look, Look::Text { text, .. } if text == "Unread")), "{paint:?}");
    assert!(paint.shapes.iter().all(|shape| rect.expand(4.0).contains_rect(shape.rect)), "{paint:?} outside {rect:?}");
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

/// A page of drawn widgets for the painted tests: "Box" on a grey ground with a ring 3 points out, a shadow, a
/// caption, a half-clipped mark and a toast-like area over it; "Dashed" drawn with `paint::dashed`; "Plain" boxed
/// by four plain hairlines.
fn drawn() -> Harness<'static> {
    let mut harness = builder().with_size(egui::vec2(400.0, 300.0)).build_ui(|ui| {
        let painter = ui.painter().clone();
        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0)), 0.0, egui::Color32::GRAY);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(120.0, 40.0), egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Box"));
        let shadow = egui::Shadow { offset: [0, 4], blur: 12, spread: 2, color: egui::Color32::BLACK };
        painter.add(shadow.as_shape(rect, 6.0));
        painter.rect(rect, 6.0, egui::Color32::from_rgb(10, 20, 30), egui::Stroke::new(1.0, egui::Color32::RED), egui::StrokeKind::Inside);
        painter.rect_stroke(rect.expand(2.0), 8.0, egui::Stroke::new(1.0, egui::Color32::YELLOW), egui::StrokeKind::Outside);
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Caption", egui::FontId::proportional(14.0), egui::Color32::WHITE);
        let half = egui::Rect::from_min_max(rect.min, egui::pos2(rect.center().x, rect.max.y));
        painter.with_clip_rect(half).rect_filled(egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 10.0)), 0.0, egui::Color32::GREEN);
        egui::Area::new(egui::Id::unique("over-the-box")).fixed_pos(rect.min).show(ui.ctx(), |ui| {
            ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(30.0, 30.0)), 0.0, egui::Color32::BLUE);
        });
        ui.add_space(20.0);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(80.0, 30.0), egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Dashed"));
        crate::draw::paint::dashed(ui.painter(), rect, egui::Color32::LIGHT_BLUE);
        ui.add_space(20.0);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(80.0, 30.0), egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Plain"));
        let stroke = egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE);
        let r = rect.shrink(0.5);
        for (a, b) in
            [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())]
        {
            ui.painter().line_segment([a, b], stroke);
        }
    });
    harness.run();
    harness
}

#[test]
fn painted_reads_ground_fill_stroke_ring_shadow_text_and_clip_from_the_widgets_layer() {
    let harness = drawn();
    let paint = painted(&harness, "Box");
    assert_eq!(paint.ground, Some(egui::Color32::GRAY));
    let rects: Vec<_> = paint.rects().collect();
    assert_eq!(rects.len(), 3, "box, ring and the clipped mark; the area over it is another layer's: {paint:#?}");
    assert_eq!(
        rects[0].look,
        Look::Rect {
            fill: egui::Color32::from_rgb(10, 20, 30),
            stroke: Some((1.0, egui::Color32::RED)),
            stroke_kind: egui::StrokeKind::Inside,
            radius: 6.0,
            dashed: false
        }
    );
    assert!(rects[0].inside && !rects[0].clipped);
    assert!(!rects[1].inside, "the ring stands outside the box");
    assert!(matches!(rects[1].look, Look::Rect { stroke: Some((_, egui::Color32::YELLOW)), stroke_kind: egui::StrokeKind::Outside, .. }));
    assert!(rects[2].clipped && rects[2].inside, "{:?}", rects[2]);
    let shadow = paint
        .shapes
        .iter()
        .find_map(|shape| if let Look::Shadow { offset, blur, spread, colour } = shape.look { Some((offset, blur, spread, colour)) } else { None });
    assert_eq!(shadow, Some((egui::vec2(0.0, 4.0), 12.0, 2.0, egui::Color32::BLACK)));
    let text = paint.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text, colour, family, size, row_height } => Some((text.clone(), *colour, family.clone(), *size, *row_height)),
        _ => None,
    });
    let (text, colour, family, size, row_height) = text.unwrap();
    assert_eq!((text.as_str(), colour, family.as_str(), size), ("Caption", egui::Color32::WHITE, "Proportional", 14.0));
    assert!(row_height > 0.0);
}

#[test]
fn painted_calls_dashed_only_what_paint_dashed_drew() {
    let harness = drawn();
    let dashed = painted(&harness, "Dashed");
    assert_eq!(dashed.shapes.len(), 1, "{dashed:#?}");
    assert!(matches!(dashed.shapes[0].look, Look::Rect { stroke: Some((1.0, egui::Color32::LIGHT_BLUE)), dashed: true, .. }));
    assert_eq!(dashed.shapes[0].rect, harness.get_by_label("Dashed").rect());
    let plain = painted(&harness, "Plain");
    assert_eq!(plain.shapes.len(), 4, "{plain:#?}");
    assert!(plain.shapes.iter().all(|shape| matches!(shape.look, Look::Rect { dashed: false, .. })));
}

#[test]
fn a_button_on_a_stage_stands_on_its_ground_with_no_fill_of_its_own() {
    let ground = egui::Color32::from_rgb(40, 50, 60);
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().with_size(egui::vec2(200.0, 120.0)).build_ui(move |ui| {
        stage(ui, ground, |ui| IconButton::new(IconId::Done, "Done").show(ui, &theme, &Keymap::default()));
    });
    harness.run();
    let paint = painted(&harness, "Done");
    assert_eq!(paint.ground, Some(ground));
    assert!(!paint.rects().any(|shape| shape.inside && matches!(shape.look, Look::Rect { fill, .. } if fill.a() > 0)), "{paint:#?}");
}

#[test]
#[should_panic(expected = "Done: no ground under the widget; place it with component::tests::stage")]
fn a_widget_alone_in_a_harness_is_refused() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().build_ui(move |ui| {
        IconButton::new(IconId::Done, "Done").show(ui, &theme, &Keymap::default());
    });
    harness.run();
    let _ = painted(&harness, "Done");
}

/// A button of a fixed width on a stage with `label` in it, left aligned.
fn fixed(label: &'static str) -> Harness<'static> {
    let mut harness = builder().with_size(egui::vec2(400.0, 120.0)).build_ui(move |ui| {
        stage(ui, egui::Color32::GRAY, |ui| {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(60.0, 24.0), egui::Sense::click());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Fixed"));
            ui.painter().text(rect.left_center(), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), egui::Color32::WHITE);
        });
    });
    harness.run();
    harness
}

#[test]
fn text_that_leaves_its_button_is_kept_and_marked() {
    let texts = |harness: &Harness<'static>| -> Vec<bool> {
        painted(harness, "Fixed").shapes.iter().filter(|shape| matches!(shape.look, Look::Text { .. })).map(|shape| shape.overflows).collect()
    };
    assert_eq!(texts(&fixed("A label much longer than its button")), vec![true]);
    assert_eq!(texts(&fixed("Ok")), vec![false]);
}
