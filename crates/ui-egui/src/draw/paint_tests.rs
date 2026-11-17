// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::epaint::RectShape;
use egui::{CornerRadius, Shape, pos2, vec2};

use super::{FloatEdge, float_shapes};
use crate::theme::{Mode, TextScale, Theme};

fn rects(shapes: &[Shape]) -> Vec<&RectShape> {
    shapes.iter().filter_map(|shape| if let Shape::Rect(rect) = shape { Some(rect) } else { None }).collect()
}

#[test]
fn a_keyboard_float_has_the_cyan_edge_and_the_one_shadow() {
    let rect = egui::Rect::from_min_size(pos2(40.0, 30.0), vec2(200.0, 120.0));
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (edge, colour) in [(FloatEdge::Keyboard, p.cyan_edge), (FloatEdge::Quiet, p.line)] {
            let shapes = float_shapes(&theme, rect, edge);
            let rects = rects(&shapes);
            assert!(rects.iter().all(|shape| shape.blur_width == 0.0), "{mode:?} {edge:?}: no egui shadow");
            let meshes: Vec<_> = shapes.iter().filter_map(|shape| if let Shape::Mesh(mesh) = shape { Some(mesh) } else { None }).collect();
            assert_eq!(meshes.len(), 1, "{mode:?} {edge:?}: one shadow");
            assert_eq!(**meshes[0], super::shadow_mesh(rect), "{mode:?} {edge:?}: the float's shadow");
            assert!(
                rects.iter().any(|shape| shape.rect == rect && shape.fill == p.surface && shape.corner_radius == CornerRadius::ZERO),
                "{mode:?} {edge:?}: a square surface"
            );
            let edges: Vec<_> = rects.iter().filter(|shape| shape.stroke.width > 0.0).collect();
            assert_eq!(edges.len(), 1, "{mode:?} {edge:?}: one edge");
            assert_eq!((edges[0].stroke.width, edges[0].stroke.color), (1.0, colour), "{mode:?} {edge:?}");
            assert_eq!(edges[0].corner_radius, CornerRadius::ZERO);
            assert!(rects.iter().all(|shape| shape.corner_radius == CornerRadius::ZERO), "square");
        }
    }
}

/// How far the shadow reaches past its own box, offset 12 down: three standard deviations of the blur, which is half
/// the blur radius, as CSS reads a blur.
fn reach() -> f32 {
    1.5 * f32::from(crate::theme::FLOAT_SHADOW.blur)
}

#[test]
fn the_shadow_mesh_tiles_its_outline_once_with_no_triangle_over_another() {
    let offset = vec2(0.0, f32::from(crate::theme::FLOAT_SHADOW.offset[1]));
    for size in [vec2(63.0, 24.0), vec2(88.0, 28.0), vec2(280.0, 200.0)] {
        let rect = egui::Rect::from_min_size(pos2(40.25, 30.5), size);
        let mesh = super::shadow_mesh(rect);
        let outline = rect.translate(offset).expand(reach());
        assert!(!mesh.indices.is_empty(), "{size:?}: a mesh");
        assert_eq!(egui::Rect::from_points(&mesh.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()), outline, "{size:?}: the outline");
        assert!(mesh.vertices.iter().all(|v| v.color.r() == 0 && v.color.g() == 0 && v.color.b() == 0), "{size:?}: black");
        // Every sample point of the outline lies in exactly one triangle (points off the grid, so none is on an edge).
        let cross = |a: egui::Pos2, b: egui::Pos2, p: egui::Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        let (columns, rows) = (outline.width().floor() as usize, outline.height().floor() as usize);
        let mut hits = vec![0_u8; columns * rows];
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = [0, 1, 2].map(|at| mesh.vertices[triangle[at] as usize].pos);
            let low = a.min(b).min(c) - outline.min;
            let high = a.max(b).max(c) - outline.min;
            for y in (low.y.floor().max(0.0) as usize)..(high.y.ceil() as usize).min(rows) {
                for x in (low.x.floor().max(0.0) as usize)..(high.x.ceil() as usize).min(columns) {
                    let p = outline.min + vec2(x as f32 + 0.437, y as f32 + 0.611);
                    let (u, v, w) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
                    if (u > 0.0 && v > 0.0 && w > 0.0) || (u < 0.0 && v < 0.0 && w < 0.0) {
                        hits[y * columns + x] += 1;
                    }
                }
            }
        }
        let wrong: Vec<_> = hits.iter().enumerate().filter(|(_, n)| **n != 1).map(|(at, n)| (at % columns, at / columns, *n)).take(4).collect();
        assert!(wrong.is_empty(), "{size:?}: points not covered exactly once, e.g. {wrong:?}");
    }
}

#[test]
fn the_shadow_keeps_its_blur_whatever_the_floats_height() {
    // A blur cut to a 24 px float's height would end 12 px past its side; the sheet's blur still shades 24 px out.
    let nearest = |mesh: &egui::epaint::Mesh, at: egui::Pos2| {
        mesh.vertices.iter().min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at))).map(|v| (v.pos, v.color.a()))
    };
    for height in [24.0, 200.0] {
        let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, height));
        let mesh = super::shadow_mesh(rect);
        let core = rect.translate(vec2(0.0, 12.0));
        let (at, alpha) = nearest(&mesh, pos2(core.left() - 24.0, core.center().y)).unwrap_or((pos2(0.0, 0.0), 0));
        assert!(alpha > 0, "{height}: {at:?} {alpha}");
        let (_, edge) = nearest(&mesh, core.expand(reach()).min).unwrap_or((pos2(0.0, 0.0), 1));
        assert_eq!(edge, 0, "{height}: clear at the outline");
    }
}
