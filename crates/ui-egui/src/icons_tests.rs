// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::epaint::CornerRadiusF32;
use egui::{Color32, Rect, Shape, pos2, vec2};

use super::{DOT, EXPORTED, PAPER, Part, SIDE, TILE, TRAY, WINDOW_ICON_SIDE, app_icon, number, picture, shapes, svg, svg_rect, window_icon};
use crate::shot::MAX_SIDE;

fn workspace_file(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

#[test]
fn the_exported_svg_is_the_icon_the_code_draws() {
    let exported = std::fs::read_to_string(workspace_file(EXPORTED)).unwrap_or_default();
    assert_eq!(exported, svg(&app_icon()), "run `just icon` to export the icon again");
}

#[test]
fn the_svg_has_one_element_a_part_on_the_side_square() {
    let parts = app_icon();
    let text = svg(&parts);
    assert!(text.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\">\n"), "{text}");
    assert!(text.ends_with("</svg>\n"), "{text}");
    assert_eq!(text.lines().count(), parts.len() + 2);
    assert!(text.contains("<rect x=\"16\" y=\"16\" width=\"224\" height=\"224\" rx=\"52\" fill=\"#1d6a64\"/>"), "{text}");
    assert!(text.contains("<polygon points=\"44,124 100,124 114,152 44,152\" fill=\"#6cc4ba\"/>"), "{text}");
    assert!(text.contains("<circle cx=\"180\" cy=\"52\" r=\"20\" fill=\"#f0a06a\"/>"), "{text}");
}

#[test]
fn numbers_are_short_and_finite() {
    assert_eq!(number(44.0), "44");
    assert_eq!(number(0.5), "0.5");
    assert_eq!(number(1.234_56), "1.235");
    assert_eq!(number(-0.0), "0");
    assert_eq!(number(-0.000_1), "0");
    assert_eq!(number(-3.0), "-3");
    assert_eq!(number(f32::NAN), "0");
    assert_eq!(number(f32::INFINITY), "0");
}

#[test]
fn rectangles_become_rects_or_paths_with_clamped_corners() {
    let square = CornerRadiusF32 { nw: 0.0, ne: 0.0, sw: 0.0, se: 0.0 };
    assert_eq!(svg_rect([1.0, 2.0], [11.0, 22.0], square, Color32::BLACK), "<rect x=\"1\" y=\"2\" width=\"10\" height=\"20\" fill=\"#000000\"/>");
    let big = CornerRadiusF32 { nw: 99.0, ne: 99.0, sw: 99.0, se: 99.0 };
    assert_eq!(
        svg_rect([0.0, 0.0], [10.0, 20.0], big, Color32::WHITE),
        "<rect x=\"0\" y=\"0\" width=\"10\" height=\"20\" rx=\"5\" fill=\"#ffffff\"/>"
    );
    let base = CornerRadiusF32 { nw: 0.0, ne: 0.0, sw: 4.0, se: 4.0 };
    assert_eq!(
        svg_rect([0.0, 0.0], [20.0, 10.0], base, Color32::BLACK),
        "<path d=\"M0 0 H20 V6 A4 4 0 0 1 16 10 H4 A4 4 0 0 1 0 6 V0 Z\" fill=\"#000000\"/>"
    );
    let odd = CornerRadiusF32 { nw: 2.0, ne: f32::NAN, sw: -1.0, se: 0.0 };
    assert_eq!(svg_rect([0.0, 0.0], [10.0, 10.0], odd, Color32::BLACK), "<path d=\"M2 0 H10 V10 H0 V2 A2 2 0 0 1 2 0 Z\" fill=\"#000000\"/>");
}

#[test]
fn every_part_stays_on_the_tile_and_every_polygon_is_convex_and_clockwise() {
    let inside = |[x, y]: [f32; 2]| (16.0..=240.0).contains(&x) && (16.0..=240.0).contains(&y);
    for part in app_icon() {
        match part {
            Part::Rect { min, max, .. } => assert!(inside(min) && inside(max) && min[0] < max[0] && min[1] < max[1], "{min:?} {max:?}"),
            Part::Circle { centre: [x, y], radius, .. } => assert!(inside([x - radius, y - radius]) && inside([x + radius, y + radius])),
            Part::Polygon { points, .. } => {
                assert!(points.len() >= 3);
                for (i, a) in points.iter().enumerate() {
                    assert!(inside(*a));
                    let b = points[(i + 1) % points.len()];
                    let c = points[(i + 2) % points.len()];
                    let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
                    assert!(cross > 0.0, "{points:?} turns the wrong way at {b:?}");
                }
            }
        }
    }
}

#[test]
fn shapes_scale_the_side_square_into_the_rect() {
    let parts = app_icon();
    let shown = shapes(&parts, Rect::from_min_size(pos2(10.0, 20.0), vec2(128.0, 128.0)));
    assert_eq!(shown.len(), parts.len());
    let Shape::Rect(tile) = &shown[0] else { panic!("the tile is a rect: {:?}", shown[0]) };
    assert_eq!(tile.rect, Rect::from_min_max(pos2(18.0, 28.0), pos2(130.0, 140.0)));
    assert_eq!(tile.corner_radius.nw, 26);
    let Shape::Circle(dot) = &shown[parts.len() - 1] else { panic!("the dot is a circle") };
    assert_eq!((dot.center, dot.radius, dot.fill), (pos2(100.0, 46.0), 10.0, DOT));
    assert!(matches!(shown[4], Shape::Path(_)));
}

#[test]
fn the_picture_shows_the_tile_the_letter_the_tray_and_the_dot() {
    let side = SIDE as usize;
    let icon = picture(&app_icon(), side).unwrap();
    assert_eq!((icon.width(), icon.height()), (side, side));
    let at = |x: usize, y: usize| icon.pixel(x, y).unwrap();
    assert_eq!(at(2, 2), Color32::TRANSPARENT, "outside the tile");
    assert_eq!(at(20, 20), Color32::TRANSPARENT, "the rounded corner");
    assert_eq!(at(30, 128), TILE);
    assert_eq!(at(128, 60), PAPER);
    assert_eq!(at(128, 140), PAPER, "the letter shows through the slot");
    assert_eq!(at(60, 180), TRAY);
    assert_eq!(at(180, 52), DOT);
    assert_eq!(at(180, 78), TILE, "the ring around the dot");
}

#[test]
fn a_picture_of_no_size_or_too_large_is_refused() {
    assert!(picture(&app_icon(), 0).is_err());
    assert!(picture(&app_icon(), MAX_SIDE + 1).is_err());
}

#[test]
fn the_window_icon_has_straight_alpha() {
    let icon = window_icon().unwrap();
    assert_eq!((icon.width as usize, icon.height as usize), (WINDOW_ICON_SIDE, WINDOW_ICON_SIDE));
    assert_eq!(icon.rgba.len(), WINDOW_ICON_SIDE * WINDOW_ICON_SIDE * 4);
    assert_eq!(&icon.rgba[..4], &[0, 0, 0, 0]);
    let (pixels, _) = icon.rgba.as_chunks::<4>();
    let edge = pixels.iter().find(|px| px[3] >= 100 && px[3] < 250).unwrap();
    for (got, want) in edge.iter().zip([TILE.r(), TILE.g(), TILE.b()]) {
        assert!(got.abs_diff(want) <= 4, "{edge:?}: the edge keeps the tile colour, not a darker premultiplied one");
    }
}
