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

/// The dash and the gap of a [`dashed`] edge, in points.
const DASH: f32 = 2.0;

/// One piece of the outline a dashed edge follows: a straight run from `from` to `to`, or a quarter arc round
/// `centre` from angle `start` (radians, clockwise on screen) over a quarter turn.
#[derive(Debug, Clone, Copy)]
enum Run {
    Line { from: Pos2, to: Pos2 },
    Arc { centre: Pos2, start: f32 },
}

/// Paints a dashed edge around `rect`, inside it: a 1 px line in `colour`, dashes 2 on and 2 off along one path
/// clockwise from the top left (after the corner's arc), the phase carried round the corners; square corners.
pub(crate) fn dashed(painter: &Painter, rect: Rect, colour: Color32) {
    dashed_round(painter, rect, colour, 0.0);
}

/// [`dashed`] with corners of `radius`. The path runs along the box's own edge and the line lies 1 px inside it, so
/// a box on whole pixels has its straight dashes on whole pixels; an arc is filled as a ring and anti-aliased.
pub(crate) fn dashed_round(painter: &Painter, rect: Rect, colour: Color32, radius: f32) {
    // The rect first: `clamp` panics on a negative or NaN bound, so only a finite, positive box reaches the radius.
    if !rect.is_finite() || !rect.is_positive() {
        return;
    }
    let r = if radius.is_finite() { radius.max(0.0).min(rect.width().min(rect.height()) / 2.0) } else { 0.0 };
    let (x0, y0, x1, y1) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let quarter = std::f32::consts::FRAC_PI_2;
    let runs = [
        Run::Line { from: pos2(x0 + r, y0), to: pos2(x1 - r, y0) },
        Run::Arc { centre: pos2(x1 - r, y0 + r), start: -quarter },
        Run::Line { from: pos2(x1, y0 + r), to: pos2(x1, y1 - r) },
        Run::Arc { centre: pos2(x1 - r, y1 - r), start: 0.0 },
        Run::Line { from: pos2(x1 - r, y1), to: pos2(x0 + r, y1) },
        Run::Arc { centre: pos2(x0 + r, y1 - r), start: quarter },
        Run::Line { from: pos2(x0, y1 - r), to: pos2(x0, y0 + r) },
        Run::Arc { centre: pos2(x0 + r, y0 + r), start: 2.0 * quarter },
    ];
    let length = |run: &Run| match run {
        Run::Line { from, to } => from.distance(*to),
        Run::Arc { .. } => quarter * r,
    };
    // The straight dashes as plain triangles: a box on whole pixels fills whole pixels and nothing beside them.
    let mut straight = egui::epaint::Mesh::default();
    let mut start = 0.0_f32;
    for run in &runs {
        let long = length(run);
        if long <= 0.0 {
            continue;
        }
        // The dashes [4k, 4k + 2] of the whole path that fall on this run, counted, never summed.
        let first = (start / (2.0 * DASH)).floor().max(0.0) as usize;
        let last = ((start + long) / (2.0 * DASH)).ceil().clamp(0.0, 4096.0) as usize;
        for k in first..=last {
            let on = k as f32 * 2.0 * DASH;
            let (a, b) = ((on - start).max(0.0), (on + DASH - start).min(long));
            if b <= a {
                continue;
            }
            match *run {
                Run::Line { from, to } => {
                    let along = (to - from) / long;
                    // Inward: the outline runs clockwise, so the inside lies to its right on screen.
                    let inward = vec2(-along.y, along.x);
                    let (p, q) = (from + along * a, from + along * b);
                    straight.add_colored_rect(Rect::from_two_pos(p, q + inward), colour);
                }
                Run::Arc { centre, start: angle } => {
                    let (from_angle, to_angle) = (angle + a / r, angle + b / r);
                    let steps = (((to_angle - from_angle) / 0.15).ceil() as usize).clamp(1, 16);
                    let at = |i: usize, radius: f32| {
                        let t = from_angle + (to_angle - from_angle) * i as f32 / steps as f32;
                        centre + vec2(t.cos(), t.sin()) * radius
                    };
                    let mut points: Vec<Pos2> = (0..=steps).map(|i| at(i, r)).collect();
                    points.extend((0..=steps).rev().map(|i| at(i, (r - 1.0).max(0.0))));
                    painter.add(Shape::convex_polygon(points, colour, Stroke::NONE));
                }
            }
        }
        start += long;
    }
    if !straight.is_empty() {
        painter.add(Shape::mesh(straight));
    }
    let pass = painter.ctx().cumulative_pass_nr();
    let layer = painter.layer_id();
    painter.ctx().data_mut(|data| {
        let edges = data.get_temp_mut_or_default::<DashedEdges>(dashed_edges_id());
        if edges.0 != pass {
            *edges = DashedEdges(pass, Vec::new());
        }
        edges.1.push((layer, rect, colour, r));
    });
}

#[cfg(test)]
#[path = "paint_tests.rs"]
mod tests;
