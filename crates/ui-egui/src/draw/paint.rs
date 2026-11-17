// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The small pieces the panes are drawn from: one line of text cut to a width, the selection, a navigation row, a
//! badge, and the flat grounds: a pane, a float with the one shadow, a hairline.

use std::sync::Arc;

use egui::layers::ShapeIdx;
use egui::text::{LayoutJob, TextWrapping};
use egui::{Color32, Frame, Galley, Margin, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::iconset::IconId;
use crate::theme::{FLOAT_SHADOW, Font, Theme};

/// One line of `sections` (text, font token and colour), cut with an ellipsis at `width`.
pub(super) fn line(ui: &Ui, sections: &[(&str, Font, Color32)], width: f32) -> Arc<Galley> {
    let mut job = LayoutJob {
        wrap: TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') },
        break_on_newline: false,
        ..LayoutJob::default()
    };
    for (text, font, colour) in sections {
        job.append(&font.text(text), 0.0, font.format(*colour));
    }
    ui.painter().layout_job(job)
}

/// The edge of a float: whether it takes the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatEdge {
    /// A float that takes the keyboard (a menu, a popup, the palette, a picker, the settings sheet): `cyan_edge`.
    Keyboard,
    /// A float that only shows (a tooltip, the link status): `line`.
    Quiet,
}

/// The shapes of [`float`], for a place kept before the float's content was drawn: the one shadow, the `surface`
/// fill and a 1 px edge inside, all square.
pub(crate) fn float_shapes(theme: &Theme, rect: Rect, edge: FloatEdge) -> Vec<Shape> {
    let p = theme.palette;
    let colour = match edge {
        FloatEdge::Keyboard => p.cyan_edge,
        FloatEdge::Quiet => p.line,
    };
    vec![
        Shape::mesh(shadow_mesh(rect)),
        Shape::rect_filled(rect, 0.0, p.surface),
        Shape::rect_stroke(rect, 0.0, Stroke::new(1.0, colour), StrokeKind::Inside),
    ]
}

/// The share of a normal distribution below `z` standard deviations (Abramowitz and Stegun 7.1.26, within 1e-7).
fn below(z: f32) -> f32 {
    let x = f64::from(z.abs()) / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    let half = 0.5 * (1.0 + erf);
    (if z < 0.0 { 1.0 - half } else { half }) as f32
}

/// The points of one axis a [`shadow_mesh`] is cut at for a box from `low` to `high`: every [`SHADOW_STEP`] across
/// each blurred edge, both ends of the reach, nothing in between where the shade is even.
fn shadow_cuts(low: f32, high: f32, reach: f32) -> Vec<f32> {
    let (first, last) = (low - reach, high + reach);
    let mut cuts = vec![first, last];
    // A fixed count of steps per edge, never a sum that stops growing far from the origin.
    let steps = (2.0 * reach / SHADOW_STEP).floor().clamp(0.0, 64.0) as usize;
    for edge in [low, high] {
        for step in 0..=steps {
            let at = edge - reach + step as f32 * SHADOW_STEP;
            if at > first && at < last {
                cuts.push(at);
            }
        }
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 0.25);
    cuts
}

/// The farthest from the origin a [`shadow_mesh`]'s box may lie, in points.
const SHADOW_FARTHEST: f32 = 1.0e7;

/// The distance between the cuts of a [`shadow_mesh`] across a blurred edge, in points.
const SHADOW_STEP: f32 = 4.0;

/// The one shadow of a float over `rect`, as its own mesh: [`FLOAT_SHADOW`]'s offset and colour, blurred as CSS
/// blurs a box shadow (a normal distribution whose standard deviation is half the blur radius), out to three standard
/// deviations. The blur is never cut to the float's size, and the mesh is a grid of quads, so no two triangles lie
/// over one another and nothing is blended twice.
pub(crate) fn shadow_mesh(rect: Rect) -> egui::epaint::Mesh {
    let shadow = FLOAT_SHADOW;
    let sigma = f32::from(shadow.blur) / 2.0;
    let reach = 3.0 * sigma;
    let core = rect.translate(vec2(f32::from(shadow.offset[0]), f32::from(shadow.offset[1]))).expand(f32::from(shadow.spread));
    let mut mesh = egui::epaint::Mesh::default();
    // Nothing so far off is ever on a screen: a box past this, or not a number, casts no shadow.
    if !core.is_finite() || [core.min.x, core.min.y, core.max.x, core.max.y].iter().any(|at| at.abs() > SHADOW_FARTHEST) || sigma <= 0.0 {
        return mesh;
    }
    let shade = |at: f32, low: f32, high: f32| (below((at - low) / sigma) - below((at - high) / sigma)).clamp(0.0, 1.0);
    let (xs, ys) = (shadow_cuts(core.left(), core.right(), reach), shadow_cuts(core.top(), core.bottom(), reach));
    let token = shadow.color.to_array().map(f32::from);
    for y in &ys {
        for x in &xs {
            let share = shade(*x, core.left(), core.right()) * shade(*y, core.top(), core.bottom());
            // The premultiplied token scaled as a whole, so its colour holds whatever it is.
            let [r, g, b, a] = token.map(|channel| (channel * share).round() as u8);
            mesh.colored_vertex(pos2(*x, *y), Color32::from_rgba_premultiplied(r, g, b, a));
        }
    }
    let columns = xs.len();
    let at = |column: usize, row: usize| u32::try_from(row * columns + column).unwrap_or(u32::MAX);
    for row in 0..ys.len().saturating_sub(1) {
        for column in 0..columns.saturating_sub(1) {
            mesh.add_triangle(at(column, row), at(column + 1, row), at(column + 1, row + 1));
            mesh.add_triangle(at(column, row), at(column + 1, row + 1), at(column, row + 1));
        }
    }
    mesh
}

/// The box a [`shadow_mesh`] shades (the float's box moved by the offset), read back from `mesh`; `None` when the
/// mesh is not a shadow (one with a vertex that is not black, or none).
#[cfg(test)]
pub(crate) fn shadow_box(mesh: &egui::epaint::Mesh) -> Option<Rect> {
    // Every corner is the token scaled: its channels in the token's proportions, within a step of rounding.
    let token = FLOAT_SHADOW.color.to_array().map(i32::from);
    let scaled = |v: &egui::epaint::Vertex| {
        let c = v.color.to_array().map(i32::from);
        (0..3).all(|i| (c[i] * token[3] - c[3] * token[i]).abs() <= token[3])
    };
    let black = !mesh.vertices.is_empty() && mesh.vertices.iter().all(scaled);
    let reach = 1.5 * f32::from(FLOAT_SHADOW.blur);
    black.then(|| Rect::from_points(&mesh.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()).shrink(reach))
}

/// Paints a float over `rect`: the one shadow, the `surface` fill and its 1 px edge.
pub(crate) fn float(painter: &Painter, theme: &Theme, rect: Rect, edge: FloatEdge) {
    painter.extend(float_shapes(theme, rect, edge));
}

/// Paints a pane over `rect`: the flat `surface`, no edge (a [`hairline`] divides two panes).
pub(crate) fn pane(painter: &Painter, theme: &Theme, rect: Rect) {
    painter.rect_filled(rect, 0.0, theme.palette.surface);
}

/// The shape of a [`hairline`].
fn hairline_shape(a: Pos2, b: Pos2, colour: Color32) -> Shape {
    let snap = |at: Pos2| pos2(at.x.floor() + 0.5, at.y.floor() + 0.5);
    Shape::line_segment([snap(a), snap(b)], Stroke::new(1.0, colour))
}

/// A 1 px line in `colour` from `a` to `b`, on whole pixels.
pub(crate) fn hairline(painter: &Painter, a: Pos2, b: Pos2, colour: Color32) {
    painter.add(hairline_shape(a, b, colour));
}

/// The panes of one frame. [`Panes::begin`] keeps a place under everything drawn after it; each [`Panes::card`]
/// draws its content and notes its rectangle; [`Panes::finish`] puts every pane's flat `surface` into the kept place,
/// with a `line` hairline on each edge that does not lie on the window's border, once where two panes meet.
#[derive(Debug)]
pub(crate) struct Panes {
    slot: ShapeIdx,
    rects: Vec<Rect>,
}

impl Panes {
    /// Keeps the place for the panes in `painter`'s list, after what it holds now.
    #[must_use]
    pub(crate) fn begin(painter: &Painter) -> Self {
        Self { slot: painter.add(Shape::Noop), rects: Vec::new() }
    }

    /// A pane over the whole of `ui`, with `add` drawing its content `inner` from its edges.
    pub(crate) fn card<R>(&mut self, ui: &mut Ui, inner: Margin, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.rects.push(ui.max_rect());
        Frame::new().inner_margin(inner).show(ui, add).inner
    }

    /// Puts the panes into the kept place of `painter`'s list.
    pub(crate) fn finish(self, painter: &Painter, theme: &Theme) {
        let window = painter.clip_rect();
        let colour = theme.palette.line;
        let near = |a: f32, b: f32| (a - b).abs() < 0.5;
        let mut shapes: Vec<Shape> = self.rects.iter().map(|rect| Shape::rect_filled(*rect, 0.0, theme.palette.surface)).collect();
        for rect in &self.rects {
            let (top, bottom) = (rect.top(), rect.bottom() - 1.0);
            let (left, right) = (rect.left(), rect.right() - 1.0);
            if !near(rect.left(), window.left()) {
                shapes.push(hairline_shape(pos2(left, top), pos2(left, rect.bottom()), colour));
            }
            // The next pane's left edge is this one's right edge: drawn once, by the next.
            let followed = self.rects.iter().any(|other| near(other.left(), rect.right()));
            if !near(rect.right(), window.right()) && !followed {
                shapes.push(hairline_shape(pos2(right, top), pos2(right, rect.bottom()), colour));
            }
            if !near(rect.top(), window.top()) {
                shapes.push(hairline_shape(pos2(left, top), pos2(rect.right(), top), colour));
            }
            if !near(rect.bottom(), window.bottom()) {
                shapes.push(hairline_shape(pos2(left, bottom), pos2(rect.right(), bottom), colour));
            }
        }
        painter.set(self.slot, Shape::Vec(shapes));
    }
}

/// The selection over `rect`: one even tone.
pub(super) fn selection(painter: &Painter, theme: &Theme, rect: Rect, corner: f32) {
    painter.rect_filled(rect, corner, theme.palette.stage);
}

/// Where a [`focus_ring`] stands against its control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ring {
    /// 2 px clear outside the control: buttons, links, chips, toggles.
    Outside,
    /// 2 px inside the control's box, where there is no room outside: fields, cells, rows.
    Inside,
}

/// The focus ring of a control over `rect`: a 1 px `cyan_edge` line, square, 2 px clear of the control (`place`).
pub(crate) fn focus_ring(painter: &Painter, theme: &Theme, rect: Rect, place: Ring) {
    let stroke = Stroke::new(1.0, theme.palette.cyan_edge);
    match place {
        Ring::Outside => painter.rect_stroke(rect.expand(2.0), 0.0, stroke, StrokeKind::Outside),
        Ring::Inside => painter.rect_stroke(rect.shrink(2.0), 0.0, stroke, StrokeKind::Inside),
    };
}

/// A pill with `count` in it, its right edge at `right`, centred on `centre_y`; returns its left edge.
pub(super) fn badge(ui: &Ui, theme: &Theme, count: u64, right: f32, centre_y: f32) -> f32 {
    crate::component::badge::count(ui, theme, count, right, centre_y)
}

/// What a navigation row shows.
pub(super) struct NavRow<'a> {
    pub(super) text: &'a str,
    /// What a screen reader says for it.
    pub(super) spoken: String,
    pub(super) badge: u64,
    pub(super) selected: bool,
    pub(super) depth: usize,
    /// The icon before the text: `muted`, `ink` while selected.
    pub(super) icon: Option<IconId>,
    /// Whether it can be pressed; one that cannot has `muted` text and a `dim` icon and takes no press.
    pub(super) enabled: bool,
}

/// A row of the folder pane: a unified view or a folder, with its unread badge.
pub(super) fn nav_row(ui: &mut Ui, theme: &Theme, row: &NavRow<'_>) -> Response {
    let s = &theme.spacing;
    let p = &theme.palette;
    let height = theme.fonts.body.line + s.s2 * 1.5;
    let sense = if row.enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), sense);
    response.widget_info(|| WidgetInfo::selected(Role::Button, row.enabled, row.selected, &row.spoken));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if row.selected {
            selection(painter, theme, rect, 0.0);
        } else if response.hovered() && row.enabled {
            painter.rect_filled(rect, 0.0, p.stage);
        }
        let depth = u16::try_from(row.depth.min(8)).unwrap_or(8);
        let mut left = rect.left() + s.s2 + f32::from(depth) * s.s3;
        if let Some(icon) = row.icon {
            let side = theme.icon_side();
            let at = Rect::from_center_size(pos2(left + side / 2.0, rect.center().y), vec2(side, side));
            // An icon is `muted`, `ink` on the current row, `dim` when the row cannot be pressed: never faded.
            let ink = match (row.enabled, row.selected) {
                (false, _) => p.dim,
                (true, true) => p.ink,
                (true, false) => p.muted,
            };
            crate::iconset::paint(painter, at, icon, ink);
            left += side + s.s2;
        }
        let right = if row.badge > 0 { badge(ui, theme, row.badge, rect.right() - s.s2, rect.center().y) - s.s2 } else { rect.right() - s.s2 };
        let ink = if row.enabled { p.ink } else { p.muted };
        let galley = line(ui, &[(row.text, theme.fonts.body, ink)], right - left);
        ui.painter().galley(pos2(left, rect.center().y - galley.size().y / 2.0), galley, ink);
        if response.has_focus() {
            focus_ring(ui.painter(), theme, rect, Ring::Inside);
        }
    }
    response
}

/// The edges [`dashed`] painted in one pass, `(pass, [(layer, rect, colour)])`; a later pass starts the list afresh,
/// so it never holds more than one frame's edges. Tests read it to tell a dashed edge from plain hairlines.
#[derive(Clone, Default)]
pub(crate) struct DashedEdges(pub u64, pub Vec<(egui::LayerId, Rect, Color32, f32)>);

/// Where [`DashedEdges`] lives in the context's memory.
pub(crate) fn dashed_edges_id() -> egui::Id {
    egui::Id::unique("paint-dashed-edges")
}

/// The longest straight side of a [`dashed`] edge that is laid out, in pixels; the rest of a longer side is left
/// bare, so a box of any size makes a bounded mesh.
const LONGEST_SIDE: i64 = 4096;

/// How far from the origin, in pixels, a dashed edge is still drawn.
const FARTHEST: u64 = 1 << 40;

/// The largest corner radius of a [`dashed`] edge, in pixels: a corner is coloured pixel by pixel.
const LARGEST_CORNER: i64 = 32;

/// How many samples a side of a corner pixel takes to measure how much of it the arc covers.
const SAMPLES: i64 = 8;

/// Paints a dashed edge around `rect`, inside it: a line in `colour` as heavy as a solid 1 point edge, dashes 2 points
/// on and 2 off along one path clockwise from the top left (after the corner's arc), the phase carried round the
/// corners; square corners.
pub(crate) fn dashed(painter: &Painter, rect: Rect, colour: Color32) {
    dashed_round(painter, rect, colour, 0.0);
}

/// The weight and the dash of a [`dashed`] edge at a scale, in physical pixels.
#[derive(Debug, Clone, Copy)]
struct DashScale {
    /// The line's thickness: `max(1, round(ppp))`, the whole pixels nearest a solid 1 point edge (which egui draws
    /// as one full pixel and a quarter-covered one at 1.25, one full and a half-covered one at 1.5): 1, 1, 2, 2 at
    /// 1, 1.25, 1.5 and 2.
    thick: i64,
    /// The dash and the gap each: `max(1, round(2 × ppp))`, 2, 3, 3, 4 at 1, 1.25, 1.5 and 2.
    dash: i64,
}

impl DashScale {
    fn at(ppp: f32) -> Self {
        let pixels = |points: f32| ((points * ppp).round() as i64).clamp(1, 16);
        Self { thick: pixels(1.0), dash: pixels(2.0) }
    }

    /// Whether the path position `at` (pixels from the path's start) lies in a dash.
    fn on(self, at: f32) -> bool {
        let dash = self.dash as f32;
        at.rem_euclid(2.0 * dash) < dash
    }
}

/// [`dashed`] with corners of `radius` points.
///
/// The edge is laid out in physical pixels on the pixel grid: the box is rounded to it, the line lies inside the box,
/// as thick as a solid 1 point edge rounded to whole pixels, and the dashes and gaps are 2 points rounded to whole
/// pixels ([`DashScale`]). A pixel of a straight side is on when its centre falls in a dash of the path. A corner's
/// pixels are covered by the arc's ring, sampled, and drawn in that share of `colour`. Every pixel is painted once: at
/// a square corner, where two sides cross, a pixel is on when either side has it on.
pub(crate) fn dashed_round(painter: &Painter, rect: Rect, colour: Color32, radius: f32) {
    // The rect first: `clamp` panics on a negative or NaN bound, so only a finite, positive box reaches the radius.
    if !rect.is_finite() || !rect.is_positive() {
        return;
    }
    let ppp = painter.pixels_per_point();
    if !ppp.is_finite() || ppp <= 0.0 {
        return;
    }
    let r_points = if radius.is_finite() { radius.max(0.0).min(rect.width().min(rect.height()) / 2.0) } else { 0.0 };
    // The box in whole pixels.
    let px = |points: f32| (points * ppp).round() as i64;
    let (left, top, right, bottom) = (px(rect.left()), px(rect.top()), px(rect.right()), px(rect.bottom()));
    let (wide, high) = (right.saturating_sub(left), bottom.saturating_sub(top));
    // Far past any screen the pixel sums could overflow: such a box is not drawn.
    let near = [left, top, right, bottom].iter().all(|at| at.unsigned_abs() <= FARTHEST);
    if near && wide >= 1 && high >= 1 {
        let scale = DashScale::at(ppp);
        // A round corner is at least as wide as the line, so the sides' bands and the corners never leave a gap.
        let r = match ((r_points * ppp).round() as i64).clamp(0, LARGEST_CORNER) {
            0 => 0,
            r => r.max(scale.thick),
        }
        .min(wide.min(high) / 2);
        let mesh = dashed_mesh(ppp, scale, [left, top, right, bottom], r, colour);
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }
    let pass = painter.ctx().cumulative_pass_nr();
    let layer = painter.layer_id();
    painter.ctx().data_mut(|data| {
        let edges = data.get_temp_mut_or_default::<DashedEdges>(dashed_edges_id());
        if edges.0 != pass {
            *edges = DashedEdges(pass, Vec::new());
        }
        edges.1.push((layer, rect, colour, r_points));
    });
}

/// A pixel, or a step from one pixel to the next.
type Pixel = (i64, i64);

/// A side of a dashed edge and the corner after it, as [`dashed_mesh`] lists them.
type Piece = (i64, Pixel, Pixel, Pixel, Pixel, f32, Pixel);

/// The mesh of a dashed edge on the pixel box `[left, top, right, bottom]` with corners of `r` pixels, at `ppp`
/// pixels per point: one square per pixel on, each pixel once.
fn dashed_mesh(ppp: f32, scale: DashScale, [left, top, right, bottom]: [i64; 4], r: i64, colour: Color32) -> egui::epaint::Mesh {
    let quarter = std::f32::consts::FRAC_PI_2;
    let (wide, high) = (right - left, bottom - top);
    // The sides clockwise from the top left, each followed by its corner: the side's length, its first pixel on the
    // box's edge, the step along it and the step inward; the corner's centre (a pixel corner), the angle its arc
    // starts at (radians, clockwise on screen) and the top left of the r by r pixel square it lies in.
    let pieces: [Piece; 4] = [
        (wide - 2 * r, (left + r, top), (1, 0), (0, 1), (right - r, top + r), -quarter, (right - r, top)),
        (high - 2 * r, (right - 1, top + r), (0, 1), (-1, 0), (right - r, bottom - r), 0.0, (right - r, bottom - r)),
        (wide - 2 * r, (right - 1 - r, bottom - 1), (-1, 0), (0, -1), (left + r, bottom - r), quarter, (left, bottom - r)),
        (high - 2 * r, (left, bottom - 1 - r), (0, -1), (1, 0), (left + r, top + r), 2.0 * quarter, (left, top)),
    ];
    let thick = scale.thick.min(wide).min(high);
    let mut on = std::collections::BTreeMap::<Pixel, f32>::new();
    let mut start = 0.0_f32;
    for (long, (x, y), (dx, dy), (ix, iy), centre, from, square) in pieces {
        for d in 0..long.clamp(0, LONGEST_SIDE) {
            if scale.on(start + d as f32 + 0.5) {
                for k in 0..thick {
                    on.insert((x + dx * d + ix * k, y + dy * d + iy * k), 1.0);
                }
            }
        }
        // The phase only, so a long side keeps every pixel of it exact.
        start = (start + long.rem_euclid(2 * scale.dash) as f32).rem_euclid(2.0 * scale.dash as f32);
        if r > 0 {
            let arc = Corner { centre: (centre.0 as f32, centre.1 as f32), r: r as f32, thick: thick as f32, from, start, scale };
            arc.cover(&mut on, square, r);
            start = (start + quarter * r as f32).rem_euclid(2.0 * scale.dash as f32);
        }
    }
    let mut mesh = egui::epaint::Mesh::default();
    let side = vec2(1.0, 1.0) / ppp;
    for ((x, y), cover) in on {
        let shade = if cover >= 1.0 { colour } else { colour.gamma_multiply(cover) };
        mesh.add_colored_rect(Rect::from_min_size(pos2(x as f32 / ppp, y as f32 / ppp), side), shade);
    }
    mesh
}

/// The quarter arc at a corner of a dashed edge: a ring `thick` pixels wide inside radius `r` round `centre` (pixels),
/// from angle `from` (radians, clockwise on screen) over a quarter turn, its dashes placed by the path position
/// `start` it begins at.
struct Corner {
    centre: (f32, f32),
    r: f32,
    thick: f32,
    from: f32,
    start: f32,
    scale: DashScale,
}

impl Corner {
    /// Adds to `on` each pixel of the `side` by `side` square at `(left, top)` that the arc's dashes cover, with the
    /// share of it they cover.
    fn cover(&self, on: &mut std::collections::BTreeMap<Pixel, f32>, (left, top): Pixel, side: i64) {
        let quarter = std::f32::consts::FRAC_PI_2;
        let inner = (self.r - self.thick).max(0.0);
        let step = 1.0 / SAMPLES as f32;
        for y in top..top + side {
            for x in left..left + side {
                let mut hits = 0_i64;
                for j in 0..SAMPLES {
                    for i in 0..SAMPLES {
                        let dx = x as f32 + (i as f32 + 0.5) * step - self.centre.0;
                        let dy = y as f32 + (j as f32 + 0.5) * step - self.centre.1;
                        let distance = dx.hypot(dy);
                        if distance < inner || distance > self.r {
                            continue;
                        }
                        let turned = (dy.atan2(dx) - self.from).rem_euclid(4.0 * quarter).min(quarter);
                        if self.scale.on(self.start + turned * self.r) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    on.insert((x, y), hits as f32 / (SAMPLES * SAMPLES) as f32);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "paint_tests.rs"]
mod tests;
