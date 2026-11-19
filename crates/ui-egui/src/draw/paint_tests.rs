// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::epaint::RectShape;
use egui::{Color32, CornerRadius, Shape, pos2, vec2};

use super::{FloatEdge, float_shapes};
use crate::theme::{Font, Mode, TextScale, Theme};

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

/// The physical pixels (columns when `across` is false, rows when true) the `line` hairlines of two panes `first` and
/// `second` cover near their seam at `seam`, at `ppp`: [`super::Panes`] in a window of 400 × 300, tessellated
/// without feathering.
fn seam_pixels(ppp: f32, first: egui::Rect, second: egui::Rect, seam: f32, across: bool) -> std::collections::BTreeSet<i32> {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))), ..Default::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut output = ctx.run_ui(input, |ui| {
        let mut panes = super::Panes::begin(ui.painter());
        for rect in [first, second] {
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| panes.card(ui, egui::Margin::ZERO, |_| ()));
        }
        panes.finish(ui.painter(), &theme);
    });
    output.textures_delta.clear();
    let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
    let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
    let mut covered = std::collections::BTreeSet::new();
    for clipped in tessellator.tessellate_shapes(output.shapes) {
        let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else { continue };
        for triangle in mesh.indices.chunks(3) {
            let corners: Vec<_> = triangle.iter().map(|at| mesh.vertices[*at as usize]).collect();
            if corners.iter().any(|vertex| vertex.color != theme.palette.line) {
                continue;
            }
            let span = egui::Rect::from_points(&corners.iter().map(|vertex| vertex.pos).collect::<Vec<_>>());
            let (low, high) = if across { (span.top(), span.bottom()) } else { (span.left(), span.right()) };
            if high - low > 4.0 || (low - seam).abs() > 4.0 {
                continue;
            }
            let (from, to) = ((low * ppp).round() as i32, (high * ppp).round() as i32);
            covered.extend(from..to);
        }
    }
    covered
}

#[test]
fn panes_that_meet_share_one_line_side_by_side_and_stacked() {
    for ppp in [1.0_f32, 1.5] {
        let wide = (ppp.round().max(1.0)) as usize;
        let left = egui::Rect::from_min_max(pos2(20.0, 20.0), pos2(200.0, 280.0));
        let right = egui::Rect::from_min_max(pos2(200.0, 20.0), pos2(380.0, 280.0));
        let beside = seam_pixels(ppp, left, right, 200.0, false);
        assert_eq!(beside.len(), wide, "{ppp}: one line between panes side by side: {beside:?}");
        let upper = egui::Rect::from_min_max(pos2(20.0, 20.0), pos2(380.0, 140.0));
        let lower = egui::Rect::from_min_max(pos2(20.0, 140.0), pos2(380.0, 280.0));
        let stacked = seam_pixels(ppp, upper, lower, 140.0, true);
        assert_eq!(stacked.len(), wide, "{ppp}: one line between stacked panes: {stacked:?}");
    }
}

#[test]
fn a_lone_pane_s_edges_end_at_its_corners() {
    for ppp in [1.0_f32, 1.5, 2.0] {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))), ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
        let theme = Theme::new(Mode::Dark, TextScale::default());
        let card = egui::Rect::from_min_max(pos2(40.0, 30.0), pos2(360.0, 270.0));
        let mut output = ctx.run_ui(input, |ui| {
            let mut panes = super::Panes::begin(ui.painter());
            ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| panes.card(ui, egui::Margin::ZERO, |_| ()));
            panes.finish(ui.painter(), &theme);
        });
        output.textures_delta.clear();
        let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
        let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
        let mut lines = Vec::new();
        for clipped in tessellator.tessellate_shapes(output.shapes) {
            let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else { continue };
            lines.extend(mesh.vertices.iter().filter(|vertex| vertex.color == theme.palette.line).map(|vertex| vertex.pos));
        }
        // The four edges lie inside the pane: no line runs a pixel past a corner.
        assert_eq!(egui::Rect::from_points(&lines), card, "{ppp}");
    }
}

#[test]
fn a_text_box_hugs_its_run_rounded_up_to_a_whole_point() {
    assert_eq!(super::run_box(6.6), 7.0);
    assert_eq!(super::run_box(13.2), 14.0);
    assert_eq!(super::run_box(14.0), 14.0);
    assert_eq!(super::run_box(39.6), 40.0);
    assert_eq!(super::run_box(0.0), 0.0);
    assert_eq!(super::run_box(f32::NAN), 0.0);
    assert_eq!(super::run_box(f32::INFINITY), 0.0);
}

/// The text shapes of one frame at `ppp` that paints `"7"` with [`super::run_at`] at `x`: their origins and the
/// leading space of each one's first section.
fn run_origins(ppp: f32, x: f32) -> Vec<(f32, f32)> {
    let shapes = frame_shapes(ppp, |ui| {
        let job = egui::text::LayoutJob::simple_singleline("7".to_owned(), egui::FontId::monospace(11.0), egui::Color32::WHITE);
        let galley = ui.painter().layout_job(job);
        super::run_at(ui.painter(), pos2(x, 3.0), galley, egui::Color32::WHITE);
    });
    shapes
        .iter()
        .filter_map(|shape| if let Shape::Text(text) = shape { Some(text) } else { None })
        .map(|text| (text.pos.x, text.galley.job.sections.first().map_or(f32::NAN, |first| first.leading_space)))
        .collect()
}

#[test]
fn a_run_at_a_fraction_of_a_pixel_stands_on_the_whole_pixel_with_the_rest_laid_out_before_it() {
    let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
    let one = run_origins(1.0, 4.5);
    assert!(one.len() == 1 && near(one[0].0, 4.0) && near(one[0].1, 0.5), "{one:?}");
    let whole = run_origins(1.0, 5.0);
    assert!(whole.len() == 1 && near(whole[0].0, 5.0) && near(whole[0].1, 0.0), "{whole:?}");
    // 4.6 points at 1.5 pixels a point: 6.9 pixels, so the origin is pixel 6 (4.0 points) and 0.6 points follow.
    let more = run_origins(1.5, 4.6);
    assert!(more.len() == 1 && near(more[0].0, 4.0) && near(more[0].1, 0.6), "{more:?}");
    assert!(run_origins(1.0, f32::NAN).is_empty() && run_origins(1.0, f32::INFINITY).is_empty(), "nothing at no place");
}

#[test]
fn a_run_at_half_a_pixel_is_drawn_half_way_between_the_two_whole_pixels() {
    use crate::component::tests::ink_centre;
    let mode = Mode::Dark;
    let theme = Theme::new(mode, TextScale::default());
    let centre = |x: f32| {
        let picture = crate::shot::paint(vec2(24.0, 20.0), 1.0, mode, |ui| {
            crate::theme::install_fonts(ui.ctx());
            let galley = crate::component::galley(ui, "7", theme.fonts.mono_s, theme.palette.ink);
            super::run_at(ui.painter(), pos2(x, 2.0), galley, theme.palette.ink);
        })
        .unwrap();
        let ground = picture.pixel(0, 0).unwrap();
        ink_centre(&picture, 0..24, 0..20, ground, theme.palette.ink).unwrap().0
    };
    let (left, half, right) = (centre(4.0), centre(4.5), centre(5.0));
    assert!((right - left - 1.0).abs() < 0.05, "{left} {right}");
    assert!((half - (left + 0.5)).abs() < 0.1, "x.5 is drawn at x.5: {left} {half} {right}");
}

#[test]
fn a_centred_run_s_box_is_in_the_middle_on_a_half_pixel_when_the_room_left_is_odd() {
    let shapes = frame_shapes(1.0, |ui| {
        let job = egui::text::LayoutJob::simple_singleline("7".to_owned(), egui::FontId::monospace(11.0), egui::Color32::WHITE);
        let galley = ui.painter().layout_job(job);
        let wide = galley.size().x;
        assert!(wide.fract() > 0.01, "a run of a fraction of a point: {wide}");
        super::run_centred(ui.painter(), egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(16.0, 16.0)), galley, egui::Color32::WHITE);
    });
    let texts: Vec<_> = shapes.iter().filter_map(|shape| if let Shape::Text(text) = shape { Some(text) } else { None }).collect();
    assert_eq!(texts.len(), 1);
    let text = texts[0];
    let lead = text.galley.job.sections.first().map_or(f32::NAN, |first| first.leading_space);
    let wide = text.galley.size().x - lead;
    let left = 8.0 - super::run_box(wide) / 2.0;
    assert!((text.pos.x + lead - left).abs() < 1e-4, "the box's left at {left}: {} + {lead}", text.pos.x);
}

/// The share of `ink` over `ground` of every pixel of a dashed box `wide` by `high` with corners of 2, painted at
/// (4, 4) at scale 1 in `mode`: rows of the picture.
fn dashed_shares(mode: Mode, wide: f32, high: f32) -> Vec<Vec<f32>> {
    let theme = Theme::new(mode, TextScale::default());
    let (ground, ink) = (theme.palette.bg, theme.palette.line);
    let size = vec2(wide + 8.0, high + 8.0);
    let picture = crate::shot::paint(size, 1.0, mode, |ui| {
        ui.painter().rect_filled(egui::Rect::from_min_size(pos2(0.0, 0.0), size), 0.0, ground);
        super::dashed_round(ui.painter(), egui::Rect::from_min_size(pos2(4.0, 4.0), vec2(wide, high)), ink, 2.0);
    })
    .unwrap();
    let channels = |colour: egui::Color32| [f32::from(colour.r()), f32::from(colour.g()), f32::from(colour.b())];
    let (g, i) = (channels(ground), channels(ink));
    let length: f32 = (0..3).map(|c| (i[c] - g[c]) * (i[c] - g[c])).sum();
    (0..picture.height())
        .map(|y| {
            (0..picture.width())
                .map(|x| {
                    let p = channels(picture.pixel(x, y).unwrap());
                    (0..3).map(|c| (p[c] - g[c]) * (i[c] - g[c])).sum::<f32>() / length
                })
                .collect()
        })
        .collect()
}

/// A dashed box of the board: its width and height, its sides and its corners.
type BoardDashes = (f32, f32, [&'static str; 4], [[f32; 4]; 4]);

/// The design board's dashed boxes with corners of 2 (the unknown avatar, 32 by 32; the key chip's none and disabled,
/// 37 by 20 and 17 by 20): each side's pixels clockwise from the top left, in quarters of the line's colour, and the
/// 2 by 2 pixels of each corner clockwise from the top left, row by row, as shares.
const DESIGN_DASHES: [BoardDashes; 3] = [
    (
        32.0,
        32.0,
        ["4400440044004400440044004400", "1430143014301430143014301430", "0143014301430143014301430143", "2024202420242024202420242024"],
        [[0.0, 0.46, 0.45, 0.0], [0.89, 0.26, 0.2, 0.07], [0.2, 0.26, 0.64, 0.2], [0.95, 0.07, 0.07, 0.0]],
    ),
    (
        37.0,
        20.0,
        ["440044004400440044004400440044004", "4301430143014301", "143014301430143014301430143014301", "2420242024202420"],
        [[0.26, 0.51, 0.51, 0.2], [0.76, 0.0, 0.0, 0.07], [0.13, 0.9, 0.07, 0.26], [0.0, 0.13, 0.2, 0.9]],
    ),
    (
        17.0,
        20.0,
        ["4400440044004", "4301430143014301", "1430143014301", "2420242024202420"],
        [[0.26, 0.51, 0.51, 0.2], [0.76, 0.0, 0.0, 0.07], [0.13, 0.9, 0.07, 0.26], [0.0, 0.13, 0.2, 0.9]],
    ),
];

#[test]
fn a_dashed_box_with_round_corners_carries_its_dashes_round_them_as_the_design_board_draws_it() {
    for mode in [Mode::Dark, Mode::Light] {
        for (wide, high, sides, corners) in DESIGN_DASHES {
            let shares = dashed_shares(mode, wide, high);
            let at = |x: usize, y: usize| shares.get(y).and_then(|row| row.get(x)).copied().unwrap_or(f32::NAN);
            let (left, top, right, bottom) = (4_usize, 4_usize, 4 + wide as usize, 4 + high as usize);
            let r = 2;
            let walked: [Vec<f32>; 4] = [
                (left + r..right - r).map(|x| at(x, top)).collect(),
                (top + r..bottom - r).map(|y| at(right - 1, y)).collect(),
                (left + r..right - r).rev().map(|x| at(x, bottom - 1)).collect(),
                (top + r..bottom - r).rev().map(|y| at(left, y)).collect(),
            ];
            for (side, (got, want)) in walked.iter().zip(sides).enumerate() {
                let want: Vec<f32> = want.chars().map(|c| c.to_digit(10).map_or(f32::NAN, |d| d as f32 / 4.0)).collect();
                assert_eq!(got.len(), want.len(), "{mode:?} {wide}x{high} side {side}");
                let off = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
                assert!(off <= 0.05, "{mode:?} {wide}x{high} side {side}: {got:?} against the board's {want:?}");
            }
            let squares = [(left, top), (right - r, top), (right - r, bottom - r), (left, bottom - r)];
            for (corner, ((x, y), want)) in squares.iter().zip(corners).enumerate() {
                let got = [at(*x, *y), at(x + 1, *y), at(*x, y + 1), at(x + 1, y + 1)];
                let off = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
                assert!(off <= 0.15, "{mode:?} {wide}x{high} corner {corner}: {got:?} against the board's {want:?}");
            }
        }
    }
}

/// The glyphs a galley shows, row by row.
fn shown(galley: &egui::Galley) -> String {
    galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect()
}

/// The glyphs laid out by `cut` at `width` and the glyphs painted when that galley goes through [`super::run_at`] at
/// `x`, at `ppp`.
fn cut_and_painted(ppp: f32, text: &'static str, width: f32, x: f32) -> (String, Vec<String>) {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let laid = std::cell::RefCell::new(String::new());
    let shapes = frame_shapes(ppp, |ui| {
        let galley = crate::component::cut(ui, text, theme.fonts.button, theme.palette.ink, width);
        *laid.borrow_mut() = shown(&galley);
        super::run_at(ui.painter(), pos2(x, 3.0), galley, theme.palette.ink);
    });
    let painted = shapes.iter().filter_map(|shape| if let Shape::Text(text) = shape { Some(shown(&text.galley)) } else { None }).collect();
    (laid.into_inner(), painted)
}

#[test]
fn a_run_painted_off_the_pixel_shows_the_glyphs_it_was_laid_out_with() {
    for (ppp, x) in [(1.0, 4.5), (1.5, 4.5), (1.0, 4.3)] {
        let (laid, painted) = cut_and_painted(ppp, "Zusammenfassung des Posteingangs", 136.0, x);
        assert!(laid.ends_with('…'), "{ppp} {x}: a cut run: {laid:?}");
        assert_eq!(painted, vec![laid.clone()], "{ppp} {x}: the same glyphs, the same ellipsis");
        let (whole, painted) = cut_and_painted(ppp, "Liste", 136.0, x);
        assert_eq!((whole.as_str(), painted), ("Liste", vec!["Liste".to_owned()]), "{ppp} {x}: a run that is not cut");
    }
}

#[test]
fn a_cut_row_line_ends_its_last_word_with_the_ellipsis_and_no_space_before_it() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let got = std::cell::RefCell::new(Vec::new());
    frame_shapes(1.0, |ui| {
        crate::theme::install_fonts(ui.ctx());
        let (f, ink) = (theme.fonts.button, theme.palette.ink);
        let cut = super::line(ui, &[("Zusammenfassung ", f, ink), ("des Posteingangs", theme.fonts.mono, ink)], 136.0);
        let fits = super::line(ui, &[("Liste", f, ink)], 136.0);
        *got.borrow_mut() = vec![shown(&cut), shown(&fits), cut.job.text.clone()];
    });
    let got = got.into_inner();
    assert_eq!(got[0], "Zusammenfassung…", "as `cut` and the design cut a line");
    assert_eq!(got[1], "Liste");
    assert_eq!(got[2], "Zusammenfassung des Posteingangs", "the text stays whole");
}

/// The ring [`super::focus_ring`] paints round `rect` in `place` at `ppp`: its rect, stroke width and colour, and kind.
fn ring_at(rect: egui::Rect, place: super::Ring, ppp: f32) -> Vec<RectShape> {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(ppp);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 200.0))), ..egui::RawInput::default() };
    let mut output = ctx.run_ui(input, |ui| super::focus_ring(ui.painter(), &theme, rect, place));
    output.textures_delta.clear();
    let shapes: Vec<Shape> = output.shapes.into_iter().map(|clipped| clipped.shape).collect();
    rects(&shapes).into_iter().cloned().collect()
}

#[test]
fn a_focus_ring_stands_on_whole_pixels_at_every_scale_its_clearance_kept() {
    let edge = Theme::new(Mode::Dark, TextScale::default()).palette.cyan_edge;
    for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
        for rect in [egui::Rect::from_min_size(pos2(10.0, 10.0), vec2(40.0, 20.0)), egui::Rect::from_min_size(pos2(10.5, 11.25), vec2(33.5, 17.75))] {
            for place in [super::Ring::Inside, super::Ring::Outside] {
                let at = format!("{ppp} {rect:?} {place:?}");
                let rings = ring_at(rect, place, ppp);
                assert_eq!(rings.len(), 1, "{at}");
                let ring = &rings[0];
                assert_eq!(ring.stroke.color, edge, "{at}");
                let thick = ring.stroke.width * ppp;
                assert!(thick >= 1.0 && (thick - thick.round()).abs() < 1e-3, "{at}: {thick} pixels thick");
                for side in [ring.rect.left(), ring.rect.top(), ring.rect.right(), ring.rect.bottom()] {
                    assert!((side * ppp - (side * ppp).round()).abs() < 1e-3, "{at}: {:?} off the pixel grid", ring.rect);
                }
                // Inside: the line starts 2 in from the control; outside: it ends 2 clear of it (half a pixel either way).
                let (inner, gap) = match place {
                    super::Ring::Inside => {
                        assert_eq!(ring.stroke_kind, egui::StrokeKind::Inside, "{at}");
                        (ring.rect, ring.rect.min - rect.min)
                    }
                    super::Ring::Outside => {
                        assert_eq!(ring.stroke_kind, egui::StrokeKind::Outside, "{at}");
                        (ring.rect, rect.min - ring.rect.min)
                    }
                };
                let _ = inner;
                assert!((gap.x - 2.0).abs() <= 0.5 / ppp + 1e-3 && (gap.y - 2.0).abs() <= 0.5 / ppp + 1e-3, "{at}: {gap:?}");
                if ppp == 1.0 && rect.min.x.fract() == 0.0 {
                    // Nothing moves at 1.0 on a whole-point control: the ring of before, exactly.
                    let before = match place {
                        super::Ring::Inside => rect.shrink(2.0),
                        super::Ring::Outside => rect.expand(2.0),
                    };
                    assert_eq!((ring.rect, ring.stroke.width), (before, 1.0), "{at}");
                }
            }
        }
    }
}

#[test]
fn a_focus_ring_round_a_hostile_rect_never_panics_and_stays_one_shape() {
    let (nan, inf) = (f32::NAN, f32::INFINITY);
    let hostile = [
        egui::Rect::from_min_max(pos2(nan, nan), pos2(nan, nan)),
        egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(nan, 10.0)),
        egui::Rect::from_min_max(pos2(-inf, -inf), pos2(inf, inf)),
        egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(inf, 10.0)),
        egui::Rect::from_min_max(pos2(50.0, 50.0), pos2(10.0, 10.0)),
        egui::Rect::from_min_max(pos2(20.0, 20.0), pos2(20.0, 20.0)),
        egui::Rect::from_min_max(pos2(-1.0e9, -1.0e9), pos2(1.0e9, 1.0e9)),
        egui::Rect::NOTHING,
        egui::Rect::EVERYTHING,
    ];
    let theme = Theme::new(Mode::Dark, TextScale::default());
    for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
        for rect in hostile {
            for place in [super::Ring::Inside, super::Ring::Outside] {
                assert!(ring_at(rect, place, ppp).len() <= 1, "{rect:?} {place:?} at {ppp}: one ring at most");
                // Painted all the way to triangles, it stays a bounded mesh.
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(ppp);
                let input =
                    egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 200.0))), ..egui::RawInput::default() };
                let mut output = ctx.run_ui(input, |ui| super::focus_ring(ui.painter(), &theme, rect, place));
                output.textures_delta.clear();
                let meshes = ctx.tessellate(output.shapes, ppp);
                let vertices: usize = meshes
                    .iter()
                    .map(|mesh| match &mesh.primitive {
                        egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                        egui::epaint::Primitive::Callback(_) => 0,
                    })
                    .sum();
                assert!(vertices <= 64, "{rect:?} {place:?} at {ppp}: {vertices} vertices");
            }
        }
    }
}

/// The ink centre of each row of a run.
type Centres = Vec<(f32, f32)>;

/// The ink centres of the rows of `text` in the caption wrapped at `width` and painted by [`super::rows_at`] at `at`
/// in `align`, and of each row's words laid out alone and put where the design puts that row: at the box's left, or
/// in the middle of `width`, with [`super::run_at`]. `(rows, alone)`, one centre each per row.
fn rows_and_alone(text: &'static str, at: egui::Pos2, width: f32, align: egui::Align) -> (Centres, Centres) {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let (ground, ink) = (theme.palette.surface, theme.palette.ink);
    let font = theme.fonts.caption;
    let line = font.line;
    let centres = |picture: &crate::shot::Picture, rows: usize| -> Vec<(f32, f32)> {
        (0..rows)
            .filter_map(|row| {
                let top = (at.y + row as f32 * line) as usize;
                crate::component::tests::ink_centre(picture, 0..picture.width(), top..top + line as usize, ground, ink)
            })
            .collect()
    };
    let size = vec2(width + 2.0 * at.x, at.y * 2.0 + 4.0 * line);
    let mut rows = 0;
    let wrapped = crate::shot::paint(size, 1.0, Mode::Dark, |ui| {
        crate::theme::install_fonts(ui.ctx());
        ui.painter().rect_filled(ui.max_rect().expand(100.0), 0.0, ground);
        let galley = crate::component::wrapped(ui, text, font, ink, width, 4, align);
        rows = galley.rows.len();
        super::rows_at(ui.painter(), at, width, galley, ink);
    });
    let wrapped = wrapped.unwrap_or_else(|error| panic!("{error:?}"));
    let mut pieces: Vec<String> = Vec::new();
    let alone = crate::shot::paint(size, 1.0, Mode::Dark, |ui| {
        crate::theme::install_fonts(ui.ctx());
        ui.painter().rect_filled(ui.max_rect().expand(100.0), 0.0, ground);
        let galley = crate::component::wrapped(ui, text, font, ink, width, 4, egui::Align::LEFT);
        pieces = galley.rows.iter().map(|row| row.glyphs.iter().map(|glyph| glyph.chr).collect::<String>().trim_end().to_owned()).collect();
        for (index, piece) in pieces.iter().enumerate() {
            let one = crate::component::galley(ui, piece, font, ink);
            let x = if align == egui::Align::Center { at.x + (width - one.size().x) / 2.0 } else { at.x };
            super::run_at(ui.painter(), pos2(x, at.y + index as f32 * line), one, ink);
        }
    });
    let alone = alone.unwrap_or_else(|error| panic!("{error:?}"));
    assert!(rows >= 2, "{text:?} wraps: {rows} rows");
    (centres(&wrapped, rows), centres(&alone, rows))
}

#[test]
fn every_row_of_a_wrapped_run_stands_where_the_design_puts_it() {
    // The settings row's help: its second row started 0.8 px right of the box's left; the empty state's centred rows
    // half a pixel right of the middle.
    let help = "Reduced keeps what explains a change and drops the text effects. Off: everything at once.";
    let centred = "This folder holds no unread messages. “Show done” brings back what you finished.";
    for (text, at, width, align) in [
        (help, pos2(24.0, 8.0), 397.0, egui::Align::LEFT),
        (help, pos2(24.5, 8.0), 397.0, egui::Align::LEFT),
        (centred, pos2(24.0, 8.0), 392.0, egui::Align::Center),
        (centred, pos2(23.0, 8.0), 391.0, egui::Align::Center),
    ] {
        let (rows, alone) = rows_and_alone(text, at, width, align);
        assert_eq!(rows.len(), alone.len(), "{text:?}");
        for (index, (row, want)) in rows.iter().zip(&alone).enumerate() {
            assert!(
                (row.0 - want.0).abs() <= 0.05 && (row.1 - want.1).abs() <= 0.05,
                "row {index} of {text:?} at {at:?}: {row:?}, the design's {want:?}"
            );
        }
    }
}

/// The baselines of `font`'s rows as [`crate::component::galley`] and [`crate::component::wrapped`] lay them out at
/// `ppp`, against the top of their galley, with the design's top of each row (the line heights over it), and the
/// design's baseline against a row's top: half the leading (the line height less the face's ascent
/// and descent) over the ascent.
fn baselines(font: crate::theme::Font, ppp: f32) -> (Vec<(f32, f32)>, f32) {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(ppp);
    crate::theme::install_fonts(&ctx);
    let mut got = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0))), ..egui::RawInput::default() };
        let mut output = ctx.run_ui(input, |ui| {
            let one = crate::component::galley(ui, "Hxg", font, egui::Color32::WHITE);
            let two = crate::component::wrapped(ui, "Hxg words that wrap onto a second row", font, egui::Color32::WHITE, 60.0, 4, egui::Align::LEFT);
            got = [one, two]
                .iter()
                .flat_map(|galley| galley.rows.iter().enumerate().map(|(index, row)| (index, row.pos.y, row.glyphs.first().map(|g| g.pos.y))))
                .filter_map(|(index, top, glyph)| glyph.map(|y| (top + y, index as f32 * font.line)))
                .collect();
        });
        output.textures_delta.clear();
    }
    let (ascent, descent) = font.face.extent();
    (got, (font.line - (ascent + descent) * font.size) / 2.0 + ascent * font.size)
}

#[test]
fn every_line_of_text_stands_on_the_whole_pixel_nearest_the_designs_baseline() {
    // The caption (Plex Sans 12/18) has its baseline on a half pixel, 13.5 under its top: egui rounded it down the page
    // to 14, where hinting sets the letters lower still; it goes on 13. A run's second row at 125 % stood 0.625 px off.
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let f = theme.fonts;
    for (name, font) in [
        ("caption", f.caption),
        ("body", f.body),
        ("strong", f.strong),
        ("read", f.read),
        ("heading", f.heading),
        ("title", f.title),
        ("label", f.label),
        ("mono", f.mono),
        ("mono_s", f.mono_s),
        ("button", f.button),
        ("key", f.key),
        ("display", f.display),
    ] {
        for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
            let (got, design) = baselines(font, ppp);
            assert!(got.len() >= 3, "{name}: one row and a wrapped run");
            for (baseline, top) in got {
                assert!(((baseline * ppp) - (baseline * ppp).round()).abs() < 1e-3, "{name} at {ppp}: on a whole pixel: {baseline}");
                let off = (top + design - baseline) * ppp;
                assert!(off > -0.5 + 1e-3 && off <= 0.5 + 1e-3, "{name} at {ppp}: the baseline at {baseline}, the design's at {top} + {design}");
            }
        }
    }
    let (got, _) = baselines(f.caption, 1.0);
    assert!(got.iter().all(|(baseline, top)| baseline - top == 13.0), "the caption's baseline 13 under its top: {got:?}");
}

#[test]
fn a_lone_pane_s_edges_meet_at_every_corner_on_odd_points_too() {
    for ppp in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
        for card in [egui::Rect::from_min_max(pos2(40.0, 30.0), pos2(360.0, 270.0)), egui::Rect::from_min_max(pos2(41.0, 31.0), pos2(361.0, 271.0))] {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))), ..Default::default() };
            input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
            let theme = Theme::new(Mode::Dark, TextScale::default());
            let mut output = ctx.run_ui(input, |ui| {
                let mut panes = super::Panes::begin(ui.painter());
                ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| panes.card(ui, egui::Margin::ZERO, |_| ()));
                panes.finish(ui.painter(), &theme);
            });
            output.textures_delta.clear();
            let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
            let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
            // Each edge is a band of four corners, in physical pixels.
            let mut bands = Vec::new();
            let mut fill = egui::Rect::NOTHING;
            for clipped in tessellator.tessellate_shapes(output.shapes) {
                let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else { continue };
                for vertex in mesh.vertices.iter().filter(|vertex| vertex.color == theme.palette.surface) {
                    fill.extend_with(pos2(vertex.pos.x * ppp, vertex.pos.y * ppp));
                }
                let lines: Vec<egui::Pos2> =
                    mesh.vertices.iter().filter(|vertex| vertex.color == theme.palette.line).map(|vertex| vertex.pos).collect();
                bands.extend(
                    lines.chunks(4).map(|band| egui::Rect::from_points(&band.iter().map(|at| pos2(at.x * ppp, at.y * ppp)).collect::<Vec<_>>())),
                );
            }
            assert_eq!(bands.len(), 4, "{ppp} {card:?}");
            let outline = bands.iter().fold(egui::Rect::NOTHING, |all, band| all.union(*band));
            // The fill stands on whole pixels, and the edges are its outermost pixels: no column or row of fill outside
            // an edge, no edge running past the fill or past another edge.
            let whole = |at: f32| (at - at.round()).abs() < 0.01;
            assert!([fill.left(), fill.top(), fill.right(), fill.bottom()].into_iter().all(whole), "{ppp} {card:?}: fill {fill:?}");
            // And it stands where its card rounds to: each side on the pixel boundary nearest the card's.
            let rounded = [card.left(), card.top(), card.right(), card.bottom()].map(|at| (at * ppp).round());
            assert_eq!([fill.left(), fill.top(), fill.right(), fill.bottom()].map(f32::round), rounded, "{ppp} {card:?}: where the pane lands");
            let near = |a: egui::Rect, b: egui::Rect| (a.min - b.min).length() < 0.01 && (a.max - b.max).length() < 0.01;
            assert!(near(outline, fill), "{ppp} {card:?}: edges {outline:?} against the fill {fill:?}");
            let covered = |x: f32, y: f32| bands.iter().any(|band| band.contains(pos2(x + 0.5, y + 0.5)));
            let (left, top, right, bottom) =
                (outline.left().round(), outline.top().round(), outline.right().round() - 1.0, outline.bottom().round() - 1.0);
            // Every pixel of the outline is drawn: no corner left out, no gap.
            let mut x = left;
            while x <= right {
                assert!(covered(x, top) && covered(x, bottom), "{ppp} {card:?}: column {x} of {outline:?}");
                x += 1.0;
            }
            let mut y = top;
            while y <= bottom {
                assert!(covered(left, y) && covered(right, y), "{ppp} {card:?}: row {y} of {outline:?}");
                y += 1.0;
            }
        }
    }
}

/// Two cards that touch, side by side and stacked, split on whole, half and other points: at every scale each pane
/// stands where its card rounds to, the two fills meet with no gap, and one line a hairline's pixels thick stands
/// between them, on the second pane's first pixels.
#[test]
fn touching_panes_share_one_line_on_whole_pixels_at_every_scale() {
    for ppp in [1.0_f32, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5] {
        for split in [200.0_f32, 201.0, 200.5, 200.3] {
            for stacked in [false, true] {
                let (a, b) = if stacked {
                    (
                        egui::Rect::from_min_max(pos2(40.0, 30.0), pos2(360.0, split - 60.0)),
                        egui::Rect::from_min_max(pos2(40.0, split - 60.0), pos2(360.0, 270.0)),
                    )
                } else {
                    (egui::Rect::from_min_max(pos2(40.0, 30.0), pos2(split, 270.0)), egui::Rect::from_min_max(pos2(split, 30.0), pos2(360.0, 270.0)))
                };
                let ctx = egui::Context::default();
                let mut input =
                    egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))), ..Default::default() };
                input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
                let theme = Theme::new(Mode::Dark, TextScale::default());
                let mut output = ctx.run_ui(input, |ui| {
                    let mut panes = super::Panes::begin(ui.painter());
                    for card in [a, b] {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| panes.card(ui, egui::Margin::ZERO, |_| ()));
                    }
                    panes.finish(ui.painter(), &theme);
                });
                output.textures_delta.clear();
                let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
                let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
                let (mut fills, mut bands) = (Vec::new(), Vec::new());
                for clipped in tessellator.tessellate_shapes(output.shapes) {
                    let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else { continue };
                    let quads = |colour: egui::Color32| -> Vec<egui::Rect> {
                        let at: Vec<egui::Pos2> = mesh
                            .vertices
                            .iter()
                            .filter(|vertex| vertex.color == colour)
                            .map(|vertex| pos2(vertex.pos.x * ppp, vertex.pos.y * ppp))
                            .collect();
                        at.chunks(4).map(egui::Rect::from_points).collect()
                    };
                    fills.extend(quads(theme.palette.surface));
                    bands.extend(quads(theme.palette.line));
                }
                let case = format!("{ppp} split {split} stacked {stacked}");
                assert_eq!(fills.len(), 2, "{case}");
                for (fill, card) in fills.iter().zip([a, b]) {
                    let rounded = [card.left(), card.top(), card.right(), card.bottom()].map(|at| (at * ppp).round());
                    let got = [fill.left(), fill.top(), fill.right(), fill.bottom()];
                    assert!(got.iter().zip(rounded).all(|(got, want)| (got - want).abs() < 0.01), "{case}: {got:?} against {rounded:?}");
                }
                let shared = if stacked { fills[0].bottom() } else { fills[0].right() };
                let meets = if stacked { fills[1].top() } else { fills[1].left() };
                assert!((shared - meets).abs() < 0.01, "{case}: no gap between the fills");
                // Seven edges: four of each pane less the one they share.
                assert_eq!(bands.len(), 7, "{case}");
                let pixels = super::line_pixels(ppp).1;
                let between: Vec<&egui::Rect> = bands
                    .iter()
                    .filter(|band| {
                        if stacked {
                            (band.top() - shared).abs() < 0.01 && band.width() > 100.0
                        } else {
                            (band.left() - shared).abs() < 0.01 && band.height() > 100.0
                        }
                    })
                    .collect();
                assert_eq!(between.len(), 1, "{case}: one line between them: {bands:?}");
                let thick = if stacked { between[0].height() } else { between[0].width() };
                assert!((thick - pixels).abs() < 0.01, "{case}: the line {thick} px, a hairline's {pixels}");
            }
        }
    }
}

/// The physical pixels across of the `cyan_edge` painted by [`super::float`] over `rect` at `ppp`, tessellated without
/// feathering: the distinct columns and rows its vertices stand on.
fn float_edge_at(ppp: f32, rect: egui::Rect) -> (Vec<f32>, Vec<f32>) {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1000.0))), ..Default::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut output = ctx.run_ui(input, |ui| super::float(ui.painter(), &theme, rect, FloatEdge::Keyboard));
    output.textures_delta.clear();
    assert_eq!(output.pixels_per_point, ppp, "the pass ran at {ppp}");
    let options = egui::epaint::TessellationOptions { feathering: false, ..Default::default() };
    let mut tessellator = egui::epaint::Tessellator::new(ppp, options, [1, 1], Vec::new());
    let mut points = Vec::new();
    for clipped in tessellator.tessellate_shapes(output.shapes) {
        let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else { continue };
        points.extend(
            mesh.vertices.iter().filter(|vertex| vertex.color == theme.palette.cyan_edge).map(|vertex| pos2(vertex.pos.x * ppp, vertex.pos.y * ppp)),
        );
    }
    let distinct = |mut all: Vec<f32>| {
        all.sort_by(f32::total_cmp);
        all.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        all
    };
    (distinct(points.iter().map(|at| at.x).collect()), distinct(points.iter().map(|at| at.y).collect()))
}

#[test]
fn a_float_s_edge_stands_on_whole_pixels_at_every_scale() {
    // The settings sheet's place in a window of 1440 × 900: whole points, not whole pixels at 1.25, 1.5 and 1.75.
    let rect = egui::Rect::from_min_size(pos2(203.0, 61.0), vec2(1035.0, 780.0));
    for ppp in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
        let (columns, rows) = float_edge_at(ppp, rect);
        let pixels = super::line_pixels(ppp).1;
        for (across, ends) in [(&columns, (rect.left(), rect.right())), (&rows, (rect.top(), rect.bottom()))] {
            assert!(across.iter().all(|at| (at - at.round()).abs() < 1e-3), "{ppp}: the edge on whole pixels: {across:?}");
            // Each side one band a hairline's pixels thick, the outermost of the float: two lines either side.
            let (low, high) = ((ends.0 * ppp).round(), (ends.1 * ppp).round());
            let want = [low, low + pixels, high - pixels, high];
            assert_eq!(across.len(), 4, "{ppp}: {across:?} against {want:?}");
            assert!(across.iter().zip(want).all(|(got, want)| (got - want).abs() < 1e-3), "{ppp}: {across:?} against {want:?}");
        }
    }
}

#[test]
fn a_float_a_box_and_an_outline_at_a_scale_of_0_nan_or_infinity_have_finite_rects() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let rect = egui::Rect::from_min_size(pos2(10.3, 20.7), vec2(120.4, 80.2));
    let (fill, edge) = (egui::Color32::WHITE, egui::Color32::BLACK);
    for ppp in [0.0, f32::NAN, f32::INFINITY] {
        let mut shapes = super::float_at(&theme, rect, FloatEdge::Keyboard, ppp);
        shapes.extend(super::boxed_at(rect, fill, edge, ppp));
        shapes.extend(super::outline_at(rect, edge, ppp));
        for shape in &shapes {
            let bounds = shape.visual_bounding_rect();
            assert!(bounds.is_finite(), "{ppp}: {shape:?}");
        }
        // As at a scale of 1.
        assert_eq!(super::outline_at(rect, edge, ppp), super::outline_at(rect, edge, 1.0), "{ppp}");
    }
}

/// The text shapes `shapes` hold, nested ones included, and the rows they paint.
fn text_rows(shapes: &[egui::epaint::ClippedShape]) -> (usize, usize) {
    fn walk(shape: &Shape, found: &mut (usize, usize)) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            Shape::Text(text) => {
                found.0 += 1;
                found.1 += text.galley.rows.len();
            }
            _ => {}
        }
    }
    let mut found = (0, 0);
    shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

#[test]
fn a_wrapped_run_paints_only_the_rows_in_sight() {
    // An open quote of thousands of lines was copied and laid out row by row every frame, in sight or not.
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let line = "The quoted words of a long message, wrapped onto many rows in a narrow box.\n";
    let text = line.repeat(4_000);
    let mut harness =
        egui_kittest::Harness::builder().with_size(vec2(400.0, 300.0)).with_context_setup(crate::theme::install_fonts).build_ui(move |ui| {
            let galley = crate::component::wrapped(ui, &text, theme.fonts.body, theme.palette.ink, 360.0, usize::MAX, egui::Align::LEFT);
            assert!(galley.rows.len() > 4_000, "{} rows", galley.rows.len());
            // The rows from 2 000 points above the clip to far below it: only those over the clip are painted.
            let painter = ui.painter().with_clip_rect(egui::Rect::from_min_size(pos2(0.0, 100.0), vec2(400.0, 110.0)));
            super::rows_at(&painter, pos2(20.0, -2_000.0), 360.0, galley, theme.palette.ink);
        });
    harness.run();
    let (shapes, rows) = text_rows(&harness.output().shapes);
    // 110 points of 22-point rows: five, and one more for each edge the clip cuts through.
    assert!((5..=7).contains(&shapes), "{shapes} text shapes painted");
    assert_eq!(rows, shapes, "one row each");
}

/// The rows of a wrapped run as `rows_at` painted them before it left out the rows outside the clip: each row's words
/// cut from the job's text, the last row the rest of it (so a cut text is cut again in the row's width).
fn rows_at_as_before(painter: &egui::Painter, at: egui::Pos2, width: f32, galley: std::sync::Arc<egui::Galley>, colour: egui::Color32) {
    use egui::text::{LayoutJob, TextWrapping};
    let align = galley.job.halign;
    let Some(section) = galley.job.sections.first() else { return };
    let text = galley.job.text.as_str();
    let mut start = 0_usize;
    let mut top = 0.0_f32;
    let count = galley.rows.len();
    for (index, row) in galley.rows.iter().enumerate() {
        let row_top = top;
        top += row.glyphs.first().map_or(row.size.y, |glyph| glyph.line_height);
        let end = if index + 1 == count {
            text.len()
        } else {
            let chars: usize = row.char_count_including_newline().into();
            text.get(start..).and_then(|rest| rest.char_indices().nth(chars)).map_or(text.len(), |(at, _)| start + at)
        };
        let words = text.get(start..end).unwrap_or_default().trim_end();
        start = end;
        let mut job = LayoutJob::single_section(words.to_owned(), section.format.clone());
        job.wrap = TextWrapping { max_width: width.max(1.0), max_rows: 1, ..galley.job.wrap.clone() };
        job.break_on_newline = false;
        let one = super::on_baseline(painter.layout_job(job));
        let wide = one.size().x;
        let x = match align {
            egui::Align::Center => at.x + (width - wide) / 2.0,
            egui::Align::Max => at.x + width - wide,
            egui::Align::Min => at.x,
        };
        super::run_at(painter, pos2(x, at.y + row_top), one, colour);
    }
}

#[test]
fn rows_in_sight_are_painted_as_before_also_where_the_text_is_cut() {
    // The callers that cap a run's rows (a popup, an empty state, a settings row's help) paint what they did.
    let samples = [
        ("wrapped", "A help line under a settings row that wraps onto a second row and a third.", 120.0, usize::MAX, egui::Align::LEFT),
        ("cut", "A long line that is cut with an ellipsis after two rows of words, more and more words.", 120.0, 2, egui::Align::LEFT),
        ("cut centred", "An empty state's words, centred, wrapping onto rows and cut after the second of them.", 140.0, 2, egui::Align::Center),
        ("tabs", "Tab\there and\tthere, CR\r\nLF\nend", 90.0, usize::MAX, egui::Align::LEFT),
        ("combining", "e\u{301}e\u{301}e\u{301} zero\u{200B}width and \u{202E}bidi\u{202C} words that wrap", 80.0, usize::MAX, egui::Align::Max),
        ("one word", "Averyveryverylongwordthatcannotbreakanywhereatallandmore", 60.0, 3, egui::Align::LEFT),
    ];
    for (name, text, width, rows, align) in samples {
        for ppp in [1.0_f32, 1.5] {
            let mut painted = Vec::new();
            for before in [true, false] {
                let theme = Theme::new(Mode::Dark, TextScale::default());
                let mut harness = egui_kittest::Harness::builder()
                    .with_size(vec2(400.0, 300.0))
                    .with_pixels_per_point(ppp)
                    .with_context_setup(crate::theme::install_fonts)
                    .build_ui(move |ui| {
                        let galley = crate::component::wrapped(ui, text, theme.fonts.body, theme.palette.ink, width, rows, align);
                        if before {
                            rows_at_as_before(ui.painter(), pos2(10.0, 10.0), width, galley, theme.palette.ink);
                        } else {
                            super::rows_at(ui.painter(), pos2(10.0, 10.0), width, galley, theme.palette.ink);
                        }
                    });
                harness.run();
                fn walk(shape: &Shape, out: &mut Vec<String>) {
                    match shape {
                        Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, out)),
                        Shape::Text(text) => out.push(format!("{:?} {:?} {:?}", text.pos, text.galley.text(), text.galley.size())),
                        _ => {}
                    }
                }
                let mut found = Vec::new();
                harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
                painted.push(found);
            }
            assert_eq!(painted.first(), painted.get(1), "{name} at ppp {ppp}");
        }
    }
}

/// One line [`crate::component::fitted`] laid out of `text` in `font` for `room`: the text it shows, how far its
/// letters reach as drawn (the right edge of the rightmost glyph) and how wide the cutting rule measures it.
struct Fitted {
    shown: String,
    reach: f32,
    measured: f32,
}

/// Every text of `texts` fitted in `font` to every room of `rooms`, at `ppp` pixels per point, and each text laid out
/// whole: `(text, room, fitted, whole)`.
fn fitted_lines(font: Font, texts: &[String], rooms: &[f32], ppp: f32) -> Vec<(String, f32, Fitted, Fitted)> {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(ppp);
    crate::theme::install_fonts(&ctx);
    let reach = |galley: &egui::Galley| {
        galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.pos.x + glyph.advance_width)).fold(0.0_f32, f32::max)
    };
    let mut found = Vec::new();
    for _ in 0..2 {
        found.clear();
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 200.0))), ..egui::RawInput::default() };
        let mut output = ctx.run_ui(input, |ui| {
            for text in texts {
                let whole = crate::component::galley(ui, text, font, Color32::WHITE);
                for room in rooms {
                    let galley = crate::component::fitted(ui, text, font, Color32::WHITE, *room);
                    found.push((
                        text.clone(),
                        *room,
                        Fitted { shown: shown(&galley), reach: reach(&galley), measured: super::measured(&galley) },
                        Fitted { shown: shown(&whole), reach: reach(&whole), measured: super::measured(&whole) },
                    ));
                }
            }
        });
        output.textures_delta.clear();
    }
    found
}

/// `count` characters of `pattern` over and over.
fn repeated(pattern: &str, count: usize) -> String {
    pattern.chars().cycle().take(count).collect()
}

/// What is wrong with a line fitted to `room` at `ppp` in `font`: wider than its room as drawn, a run measured
/// narrower than it is drawn (cut or whole), an ellipsis that is not the one at its end, a cut that is not needed or
/// that splits a grapheme cluster. The only slack is what adding up the advances in `f32` drifts by.
fn faults(font: Font, ppp: f32, texts: &[String], rooms: &[f32]) -> Vec<String> {
    use unicode_segmentation::UnicodeSegmentation as _;
    let mut faults = Vec::new();
    for (text, room, fitted, whole) in fitted_lines(font, texts, rooms, ppp) {
        let name = format!("{} chars of {:?}… in {room} at ppp {ppp}", text.chars().count(), text.chars().take(6).collect::<String>());
        if fitted.reach > room + 0.001 {
            faults.push(format!("{name}: {} wide", fitted.reach));
        }
        for (which, line) in [("cut", &fitted), ("whole", &whole)] {
            if line.measured + 0.001 < line.reach {
                faults.push(format!("{name}: {which} measured {} but drawn {} wide", line.measured, line.reach));
            }
        }
        if fitted.shown == whole.shown {
            continue;
        }
        if whole.reach + 1.0 <= room {
            faults.push(format!("{name}: cut although {} fits", whole.reach));
        }
        if fitted.shown.is_empty() {
            continue;
        }
        let Some(kept) = fitted.shown.strip_suffix('…') else {
            faults.push(format!("{name}: cut with no ellipsis at its end: {:?}", fitted.shown));
            continue;
        };
        if kept.contains('…') {
            faults.push(format!("{name}: more than one ellipsis"));
        }
        let ends: Vec<usize> = whole.shown.grapheme_indices(true).map(|(at, _)| at).collect();
        if !whole.shown.starts_with(kept) || !(kept.is_empty() || ends.contains(&kept.len()) || kept.len() == whole.shown.len()) {
            faults.push(format!("{name}: not cut at a grapheme boundary: {:?}", kept.chars().rev().take(4).collect::<String>()));
        }
    }
    faults
}

/// The texts of the sweep: Latin words, letters with combining marks, an emoji ZWJ sequence, and clusters whose second
/// part has a width of its own (flags, Hangul jamo, Thai with sara am: a cut between their parts would be narrower,
/// so only cutting between clusters keeps them whole), and letters between zero-width non-joiners (each one a shaped
/// cluster of no advance, inside its letter's grapheme); 10, 40 and 10 000 characters of each.
fn sweep_texts() -> Vec<String> {
    let patterns = [
        "Quarterly studio planning, kiln rota ",
        "e\u{301}a\u{308}o\u{302} ",
        "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467} ab ",
        "\u{1F1ED}\u{1F1FA}\u{1F1EB}\u{1F1F7} \u{1112}\u{1161}\u{11AB} \u{0E01}\u{0E33}\u{0E19}\u{0E33} ",
        "A\u{200C}B\u{200C}",
    ];
    patterns.iter().flat_map(|pattern| [10, 40, 10_000].map(|count| repeated(pattern, count))).collect()
}

const SWEEP_ROOMS: [f32; 7] = [0.0, 20.0, 60.0, 100.0, 150.0, 240.0, 1000.0];

#[test]
fn a_fitted_label_in_the_tracked_style_is_never_wider_than_its_room() {
    let texts = sweep_texts();
    for percent in [100, 130] {
        let scale = TextScale::from_percent(percent).unwrap_or_default();
        let theme = Theme::new(Mode::Dark, scale);
        assert!(theme.fonts.label.tracking > 0.0, "the label style is tracked");
        for ppp in [1.0, 1.5] {
            let found = faults(theme.fonts.label, ppp, &texts, &SWEEP_ROOMS);
            assert!(found.is_empty(), "{percent} %: {} faults: {:#?}", found.len(), found.iter().take(6).collect::<Vec<_>>());
        }
    }
}

#[test]
fn a_fitted_line_in_an_untracked_or_tightened_style_is_never_wider_than_its_room() {
    let texts = sweep_texts();
    for percent in [100, 130] {
        let scale = TextScale::from_percent(percent).unwrap_or_default();
        let f = Theme::new(Mode::Dark, scale).fonts;
        for (name, font) in [("body", f.body), ("mono_s", f.mono_s), ("caption", f.caption), ("display", f.display)] {
            for ppp in [1.0, 1.5] {
                let found = faults(font, ppp, &texts, &SWEEP_ROOMS);
                assert!(found.is_empty(), "{name} at {percent} %: {} faults: {:#?}", found.len(), found.iter().take(6).collect::<Vec<_>>());
            }
        }
    }
}

#[test]
fn within_measures_a_tracked_run_with_its_letter_spacing_and_none_after_the_last_letter() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let font = theme.fonts.label;
    let spacing = font.tracking * font.size;
    let (mut plain, mut tracked) = (None, None);
    let shapes = frame_shapes(1.0, |ui| {
        crate::theme::install_fonts(ui.ctx());
        let mut job = egui::text::LayoutJob::single_section("WORDS".to_owned(), font.format(Color32::WHITE));
        job.wrap.max_width = f32::INFINITY;
        tracked = Some(ui.painter().layout_job(job));
        let mut job =
            egui::text::LayoutJob::single_section("WORDS".to_owned(), egui::TextFormat { extra_letter_spacing: 0.0, ..font.format(Color32::WHITE) });
        job.wrap.max_width = f32::INFINITY;
        plain = Some(ui.painter().layout_job(job));
    });
    drop(shapes);
    let (Some(plain), Some(tracked)) = (plain, tracked) else { panic!("laid out") };
    let bare: f32 = plain.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.advance_width)).sum();
    // How far the letters reach as drawn, on their rounded places: the measure is never less.
    let drawn = |galley: &egui::Galley| {
        galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| row.pos.x + glyph.pos.x + glyph.advance_width)).fold(0.0_f32, f32::max)
    };
    // Four gaps between five letters, none after the last.
    let wide = bare + 4.0 * spacing;
    assert!(super::within(&tracked, wide.max(drawn(&tracked)) + 0.0005), "fits its letters and the four gaps");
    assert!(!super::within(&tracked, wide - 0.01), "not with less room than its letters and the four gaps");
    let bare_box = bare.max(drawn(&plain));
    assert!(super::within(&plain, bare_box + 0.0005) && !super::within(&plain, bare - 0.01), "an untracked run: its advances");
}

#[test]
fn a_tracked_label_is_never_measured_narrower_than_drawn_whatever_zero_width_signs_it_holds() {
    // Signs of no advance that may shape as clusters of their own, each taking the label's spacing before it: the
    // zero-width non-joiner, a joiner outside an emoji sequence, the word joiner, the soft hyphen, two variation
    // selectors, the byte order mark, the zero-width space and the direction marks (`plain` lets them through).
    let signs = ['\u{200C}', '\u{200D}', '\u{2060}', '\u{00AD}', '\u{FE0F}', '\u{FE00}', '\u{FEFF}', '\u{200B}', '\u{200E}', '\u{200F}', '\u{061C}'];
    let texts: Vec<String> = signs
        .iter()
        .flat_map(|sign| [format!("A{sign}B{sign}"), format!("Kiln{sign} rota{sign}{sign} ")])
        .flat_map(|pattern| [10, 40].map(|count| repeated(&pattern, count)))
        .collect();
    for percent in [100, 130] {
        let font = Theme::new(Mode::Dark, TextScale::from_percent(percent).unwrap_or_default()).fonts.label;
        for ppp in [1.0, 1.5] {
            // A joiner or a selector after a letter is counted as a gap egui may not draw (the over-count of
            // `advances`, one spacing each at most): such a line may be cut a letter before it would have to be.
            let over_counted = |fault: &String| {
                fault.contains("cut although")
                    && ['\u{200D}', '\u{FE0F}', '\u{FE00}'].iter().any(|sign| fault.contains(&sign.escape_unicode().to_string()))
            };
            let found: Vec<String> =
                faults(font, ppp, &texts, &[60.0, 100.0, 150.0, 240.0]).into_iter().filter(|fault| !over_counted(fault)).collect();
            assert!(found.is_empty(), "{percent} % at ppp {ppp}: {} faults: {:#?}", found.len(), found.iter().take(8).collect::<Vec<_>>());
        }
    }
}
