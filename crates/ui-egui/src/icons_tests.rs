// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::epaint::CornerRadiusF32;
use egui::{Color32, Rect, Shape, pos2, vec2};

use super::{
    EDGE, EXPORTED, Fill, GRADIENT, Part, SIDE, TILE, WINDOW_ICON_SIDE, app_icon, gradient_at, number, picture, shapes, svg, svg_rect, window_icon,
};
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
fn the_svg_defines_the_gradient_then_has_one_element_a_part() {
    let parts = app_icon();
    let text = svg(&parts);
    assert!(text.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\">\n"), "{text}");
    assert!(text.ends_with("</svg>\n"), "{text}");
    assert_eq!(text.lines().count(), parts.len() + 3, "the svg line, the gradient, the parts and the end");
    assert!(text.contains("<linearGradient id=\"electric\" gradientUnits=\"userSpaceOnUse\" x1=\"48\" y1=\"60\" x2=\"208\" y2=\"200\">"), "{text}");
    assert!(
        text.contains("<stop stop-color=\"#2f6bff\"/><stop offset=\"0.5\" stop-color=\"#8a3ffc\"/><stop offset=\"1\" stop-color=\"#ff2d95\"/>"),
        "{text}"
    );
    assert!(text.contains("<rect x=\"9\" y=\"9\" width=\"238\" height=\"238\" rx=\"59\" fill=\"#ffffff\"/>"), "{text}");
    // The deep ink at 16 %; kept premultiplied, its colour comes back a step off.
    assert!(text.contains("<rect x=\"7\" y=\"7\" width=\"242\" height=\"242\" rx=\"61\" fill=\"#0"), "{text}");
    assert!(text.contains("fill-opacity=\"0.161\""), "{text}");
    assert!(text.contains("<polygon points=\"41,135 95,135 111.7,159 41,159\" fill=\"url(#electric)\"/>"), "{text}");
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
    let black = Fill::Solid(Color32::BLACK);
    assert_eq!(svg_rect([1.0, 2.0], [11.0, 22.0], square, black), "<rect x=\"1\" y=\"2\" width=\"10\" height=\"20\" fill=\"#000000\"/>");
    let big = CornerRadiusF32 { nw: 99.0, ne: 99.0, sw: 99.0, se: 99.0 };
    assert_eq!(
        svg_rect([0.0, 0.0], [10.0, 20.0], big, Fill::Solid(Color32::WHITE)),
        "<rect x=\"0\" y=\"0\" width=\"10\" height=\"20\" rx=\"5\" fill=\"#ffffff\"/>"
    );
    let base = CornerRadiusF32 { nw: 0.0, ne: 0.0, sw: 4.0, se: 4.0 };
    assert_eq!(svg_rect([0.0, 0.0], [20.0, 10.0], base, black), "<path d=\"M0 0 H20 V6 A4 4 0 0 1 16 10 H4 A4 4 0 0 1 0 6 V0 Z\" fill=\"#000000\"/>");
    let odd = CornerRadiusF32 { nw: 2.0, ne: f32::NAN, sw: -1.0, se: 0.0 };
    assert_eq!(svg_rect([0.0, 0.0], [10.0, 10.0], odd, black), "<path d=\"M2 0 H10 V10 H0 V2 A2 2 0 0 1 2 0 Z\" fill=\"#000000\"/>");
}

#[test]
fn every_part_stays_on_the_square_and_every_polygon_is_convex_and_clockwise() {
    let inside = |[x, y]: [f32; 2]| (0.0..=SIDE).contains(&x) && (0.0..=SIDE).contains(&y);
    for part in app_icon() {
        match part {
            Part::Rect { min, max, .. } => assert!(inside(min) && inside(max) && min[0] < max[0] && min[1] < max[1], "{min:?} {max:?}"),
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
fn shapes_scale_the_side_square_into_the_rect_and_grade_the_mark() {
    let parts = app_icon();
    let shown = shapes(&parts, Rect::from_min_size(pos2(10.0, 20.0), vec2(128.0, 128.0)));
    assert_eq!(shown.len(), parts.len());
    let Shape::Rect(tile) = &shown[1] else { panic!("the tile is a rect: {:?}", shown[1]) };
    assert_eq!(tile.rect, Rect::from_min_max(pos2(14.5, 24.5), pos2(133.5, 143.5)));
    assert_eq!(tile.fill, TILE);
    assert!(shown[2..].iter().all(|shape| matches!(shape, Shape::Mesh(_))), "the mark is graded");
}

#[test]
fn the_gradient_runs_from_blue_through_violet_to_pink() {
    let [first, middle, last] = GRADIENT;
    assert_eq!(gradient_at([48.0, 60.0]), first);
    assert_eq!(gradient_at([128.0, 130.0]), middle);
    assert_eq!(gradient_at([208.0, 200.0]), last);
    assert_eq!(gradient_at([0.0, 0.0]), first, "clamped before the line");
    assert_eq!(gradient_at([f32::NAN, 0.0]), first);
}

#[test]
fn the_picture_shows_the_white_tile_and_the_graded_mark() {
    let side = SIDE as usize;
    let icon = picture(&app_icon(), side).unwrap();
    assert_eq!((icon.width(), icon.height()), (side, side));
    let at = |x: usize, y: usize| icon.pixel(x, y).unwrap();
    assert_eq!(at(2, 2), Color32::TRANSPARENT, "outside the tile");
    assert_eq!(at(14, 14), Color32::TRANSPARENT, "the rounded corner");
    assert_eq!(at(30, 128), TILE);
    assert_eq!(at(128, 85), TILE, "between the cards");
    assert_eq!(at(128, 150), TILE, "the notch");
    let near = |got: Color32, want: Color32| got.to_array().iter().zip(want.to_array()).all(|(a, b)| a.abs_diff(b) <= 12);
    for (x, y) in [(128, 65), (128, 106), (60, 180), (200, 180)] {
        let want = gradient_at([x as f32 + 0.5, y as f32 + 0.5]);
        assert!(near(at(x, y), want), "({x}, {y}): {:?} is not {want:?}", at(x, y));
    }
    assert!(at(128, 8).a() > 0 && at(128, 8) != TILE, "the faint edge");
    let _ = EDGE;
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
    // The middle of the tile is opaque white; the faint edge outside it is mostly see-through, never darkened by
    // premultiplied alpha into grey.
    let middle = (WINDOW_ICON_SIDE / 2 * WINDOW_ICON_SIDE + 30) * 4;
    assert_eq!(&icon.rgba[middle..middle + 4], &[255, 255, 255, 255]);
    let (pixels, _) = icon.rgba.as_chunks::<4>();
    let edge = pixels.iter().find(|px| px[3] > 0 && px[3] < 60).unwrap();
    assert!(edge[0] < 40 && edge[1] < 40 && edge[2] < 40, "{edge:?}: the edge keeps the ink's colour");
}
