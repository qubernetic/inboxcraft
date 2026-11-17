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
                let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 200.0))), ..egui::RawInput::default() };
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
