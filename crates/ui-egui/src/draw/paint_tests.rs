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

#[test]
fn a_shadow_mesh_far_off_or_not_a_number_returns_at_once_and_small() {
    let most = 60 * 60;
    for at in [1.0e9, f32::MAX / 4.0, f32::MAX, -f32::MAX, f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        for rect in [
            egui::Rect::from_min_size(pos2(at, at), vec2(63.0, 24.0)),
            egui::Rect::from_min_size(pos2(10.0, 10.0), vec2(at, 24.0)),
            egui::Rect::from_min_max(pos2(10.0, 10.0), pos2(at, at)),
        ] {
            let mesh = super::shadow_mesh(rect);
            assert!(mesh.vertices.len() <= most, "{rect:?}: {} vertices", mesh.vertices.len());
        }
    }
}

/// The alpha (0 to 255) the mesh paints at `at`: the triangle that holds it, its corners mixed by where `at` lies.
fn sampled(mesh: &egui::epaint::Mesh, at: egui::Pos2) -> f32 {
    let cross = |a: egui::Pos2, b: egui::Pos2, p: egui::Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [a, b, c] = [0, 1, 2].map(|i| mesh.vertices[triangle[i] as usize]);
        let area = cross(a.pos, b.pos, c.pos);
        let (wa, wb, wc) = (cross(b.pos, c.pos, at) / area, cross(c.pos, a.pos, at) / area, cross(a.pos, b.pos, at) / area);
        if wa >= 0.0 && wb >= 0.0 && wc >= 0.0 {
            return wa * f32::from(a.color.a()) + wb * f32::from(b.color.a()) + wc * f32::from(c.color.a());
        }
    }
    0.0
}

#[test]
fn the_shadow_is_the_sheets_0_12_blur_32_black_at_40_percent() {
    // The sheet: dropShadow 0, 12, blur 32, spread 0, #000000 at 0.4. Read as CSS reads a blur, a normal fall-off
    // whose standard deviation is 16: the numbers below are worked out from that, not read from the code.
    let rect = egui::Rect::from_min_size(pos2(100.0, 100.0), vec2(400.0, 400.0));
    let mesh = super::shadow_mesh(rect);
    assert!(mesh.vertices.iter().all(|v| v.color.r() == 0 && v.color.g() == 0 && v.color.b() == 0), "black");
    let (centre_x, centre_y) = (300.0, 312.0);
    let checks = [
        (pos2(centre_x, centre_y), 102.0, "the middle: 40 %"),
        (pos2(100.0, centre_y), 51.0, "the left edge: half"),
        (pos2(500.0, centre_y), 51.0, "the right edge: half"),
        (pos2(centre_x, 112.0), 51.0, "the top edge, 12 down: half"),
        (pos2(centre_x, 512.0), 51.0, "the bottom edge, 12 down: half"),
        (pos2(centre_x, 100.0), 23.1, "the float's own top, 12 over the shadow's: 102 × Φ(−0.75)"),
        (pos2(84.0, centre_y), 16.2, "16 out: 102 × Φ(−1)"),
        (pos2(68.0, centre_y), 2.3, "32 out: 102 × Φ(−2)"),
        (pos2(centre_x, 544.0), 2.3, "32 under: 102 × Φ(−2)"),
        (pos2(100.0, 112.0), 25.5, "the corner: a quarter"),
    ];
    for (at, want, what) in checks {
        let got = sampled(&mesh, at);
        assert!((got - want).abs() <= 1.5, "{what}: {got} at {at:?}, want {want}");
    }
}

/// The shapes `draw` adds to one headless frame at `scale` pixels per point.
fn frame_shapes(scale: f32, draw: impl FnMut(&mut egui::Ui)) -> Vec<Shape> {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);
    let mut output = ctx.run_ui(egui::RawInput::default(), draw);
    output.textures_delta.clear();
    output.shapes.into_iter().map(|clipped| clipped.shape).collect()
}

/// The vertices of every mesh in `shapes` and the points of every path, counted.
fn vertices(shapes: &[Shape]) -> usize {
    shapes
        .iter()
        .map(|shape| match shape {
            Shape::Mesh(mesh) => mesh.vertices.len(),
            Shape::Path(path) => path.points.len(),
            _ => 1,
        })
        .sum()
}

#[test]
fn a_dashed_edge_on_any_rect_and_radius_returns_and_stays_small() {
    use egui::Rect;
    let (nan, inf) = (f32::NAN, f32::INFINITY);
    let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y), vec2(w, h));
    let rects = [
        Rect::NOTHING,
        Rect::EVERYTHING,
        Rect::from_min_max(pos2(30.0, 30.0), pos2(10.0, 10.0)),
        Rect::from_min_max(pos2(10.0, 30.0), pos2(30.0, 10.0)),
        at(nan, 10.0, 20.0, 20.0),
        at(10.0, 10.0, nan, 20.0),
        at(10.0, 10.0, 20.0, nan),
        at(inf, 10.0, 20.0, 20.0),
        at(10.0, 10.0, inf, 20.0),
        at(-inf, -inf, inf, inf),
        at(10.0, 10.0, 0.0, 0.0),
        at(10.0, 10.0, 0.0, 20.0),
        at(10.0, 10.0, 1.0, 1.0),
        at(10.0, 10.0, 2.0, 2.0),
        at(10.0, 10.0, 3.0, 3.0),
        at(10.0, 10.0, 1.0, 20.0),
        at(10.0, 10.0, 3.0, 20.0),
        at(10.0, 10.0, 1e9, 20.0),
        at(-5e8, 10.0, 1e9, 1e9),
        at(1e30, 1e30, 40.0, 20.0),
        Rect::from_min_max(pos2(-1e30, -1e30), pos2(1e30, 1e30)),
        Rect::from_min_max(pos2(-f32::MAX, -f32::MAX), pos2(f32::MAX, f32::MAX)),
    ];
    let colour = egui::Color32::from_rgb(0x8b, 0x91, 0x96);
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for rect in rects {
            for radius in [0.0, 2.0, -3.0, nan, inf, -inf, 1e9] {
                let shapes = frame_shapes(scale, |ui| {
                    if radius == 0.0 {
                        super::dashed(ui.painter(), rect, colour);
                    }
                    super::dashed_round(ui.painter(), rect, colour, radius);
                });
                let count = vertices(&shapes);
                assert!(count <= 200_000, "{rect:?} radius {radius} at {scale}: {count} vertices");
            }
        }
    }
}

/// The rect a hairline from (10.3, 20.7) to (200.3, 20.7) tessellates to at `ppp` physical pixels per point, in
/// points: [`super::hairline`] painted in a pass of a context at that scale, its shapes tessellated without feathering.
fn hairline_at(ppp: f32) -> egui::Rect {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    let mut output = ctx.run_ui(input, |ui| super::hairline(ui.painter(), pos2(10.3, 20.7), pos2(200.3, 20.7), egui::Color32::WHITE));
    output.textures_delta.clear();
    assert_eq!(output.pixels_per_point, ppp, "the pass ran at {ppp}");
    let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
    let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
    let points: Vec<_> = tessellator
        .tessellate_shapes(output.shapes)
        .into_iter()
        .filter_map(|clipped| if let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive { Some(mesh) } else { None })
        .flat_map(|mesh| mesh.vertices.into_iter().map(|vertex| vertex.pos))
        .collect();
    assert!(!points.is_empty(), "{ppp}: the hairline was painted");
    egui::Rect::from_points(&points)
}

#[test]
fn a_hairline_stands_on_whole_pixels() {
    for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
        let rect = hairline_at(ppp);
        let (top, bottom) = (rect.top() * ppp, rect.bottom() * ppp);
        let whole = |at: f32| (at - at.round()).abs() < 1e-3;
        assert!(whole(top) && whole(bottom), "{ppp}: top {top} and bottom {bottom} in physical pixels");
        assert!(((bottom - top) - ppp.round().max(1.0)).abs() < 1e-3, "{ppp}: {} physical pixels high", bottom - top);
    }
}
