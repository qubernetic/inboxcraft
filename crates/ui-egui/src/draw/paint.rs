// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The small pieces the panes are drawn from: one line of text cut to a width, the selection, a navigation row, a
//! badge, and the flat grounds: a pane, a float with the one shadow, a hairline and a divider made of one.

use std::sync::Arc;

use egui::layers::ShapeIdx;
use egui::text::{LayoutJob, TextWrapping};
use egui::{Color32, Frame, Galley, Margin, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::iconset::IconId;
use crate::theme::{FLOAT_SHADOW, Font, Theme};

/// `galley` with every row's baseline on the whole pixel nearest the design's, one half way between two pixels going to
/// the upper one. A text box of the design puts its baseline half the leading (line height less the face's ascent and
/// descent) under its top, plus the ascent, a fraction of a pixel included, and a row's top is the line heights over
/// it. egui puts a baseline and a row's top on whole pixels too, but rounds a half down the page, where the hinting it
/// draws with sets the letters a little lower still: a caption (Plex Sans 12/18, its baseline 13.5 under its top) stood
/// 0.6 to 0.7 px low; on 13 it stands 0.3 to 0.4 px high. A run's second row at 125 % stood 0.6 px off.
///
/// Whole pixels only: egui draws a glyph's pixels on whole pixel rows, and a glyph put between two (sampled between
/// texels) loses its top row, since the font atlas keeps no clear texel over its first row of glyphs. A row of glyphs
/// of more than one line height is left as egui lays it out.
pub(crate) fn on_baseline(galley: Arc<Galley>) -> Arc<Galley> {
    let ppp = if galley.pixels_per_point.is_finite() && galley.pixels_per_point > 0.0 { galley.pixels_per_point } else { 1.0 };
    let mut top = 0.0_f32;
    let mut shifts = Vec::with_capacity(galley.rows.len());
    for row in &galley.rows {
        let first = row.glyphs.first();
        let even = first.is_some_and(|glyph| row.glyphs.iter().all(|other| other.line_height == glyph.line_height));
        let off = match first {
            Some(glyph) if even => {
                let exact =
                    top + glyph.font_face_ascent + 0.5 * (glyph.line_height - glyph.font_height) + 0.5 * (glyph.font_height - glyph.font_face_height);
                // The nearest whole pixel, a half going up.
                let nearest = (exact * ppp - 0.5).ceil() / ppp;
                ((nearest - (row.pos.y + glyph.pos.y)) * ppp).round() / ppp
            }
            _ => 0.0,
        };
        shifts.push(if off.is_finite() { off } else { 0.0 });
        top += match first {
            Some(glyph) if even => glyph.line_height,
            _ => row.size.y,
        };
    }
    if shifts.iter().all(|off| *off == 0.0) {
        return galley;
    }
    let mut moved = (*galley).clone();
    let reach = shifts.iter().fold(0.0_f32, |most, off| most.max(off.abs()));
    for (row, off) in moved.rows.iter_mut().zip(shifts) {
        row.pos.y += off;
    }
    moved.mesh_bounds = moved.mesh_bounds.expand2(vec2(0.0, reach));
    Arc::new(moved)
}

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
    on_baseline(tight_cut(ui.painter(), ui.painter().layout_job(job)))
}

/// A one-line `galley` cut with an ellipsis as the design cuts a line: the ellipsis follows the last letter kept, so
/// the spaces a cut leaves before it are dropped. It is laid out again just wide enough for the letters and the
/// ellipsis; the text stays whole. A galley that is not cut, or ends in a letter, is given back as it is.
pub(crate) fn tight_cut(painter: &Painter, galley: Arc<Galley>) -> Arc<Galley> {
    let shown = |galley: &Galley| -> String { galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect() };
    if !galley.elided || galley.rows.len() != 1 {
        return galley;
    }
    let Some((ellipsis, before)) = galley.rows.first().and_then(|row| row.glyphs.split_last()) else { return galley };
    let Some(last) = before.iter().rposition(|glyph| !glyph.chr.is_whitespace()) else { return galley };
    let Some(space) = before.get(last + 1) else { return galley };
    if ellipsis.chr != '…' {
        return galley;
    }
    let mut job = (*galley.job).clone();
    // Half the space to spare: the space still does not fit, and rounding to pixels later cuts nothing.
    job.wrap.max_width = space.pos.x + ellipsis.advance_width + space.advance_width / 2.0;
    let want: String = before.iter().take(last + 1).map(|glyph| glyph.chr).chain(std::iter::once('…')).collect();
    let again = on_baseline(painter.layout_job(job));
    if shown(&again) == want { again } else { galley }
}

/// The width the design gives a run of text `wide` points long: its text box hugs the run, rounded up to a whole
/// point.
pub(crate) fn run_box(wide: f32) -> f32 {
    if wide.is_finite() { (wide - 0.01).ceil().max(0.0) } else { 0.0 }
}

/// Paints `galley` with its left edge at `at.x` exactly, a fraction of a pixel included, and its top at `at.y`.
///
/// epaint puts a galley's origin on a whole physical pixel, but places the glyphs inside it on quarter pixels. So the
/// origin goes on the whole pixel at or left of `at.x` and the rest is laid out as leading space before the run: a
/// text box of the design at x.5 is drawn at x.5, as the design draws it, and every other run keeps its sharp edges.
pub(crate) fn run_at(painter: &Painter, at: Pos2, galley: Arc<Galley>, colour: Color32) {
    if !at.x.is_finite() || !at.y.is_finite() {
        return;
    }
    let ppp = painter.pixels_per_point();
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    let whole = (at.x * ppp).floor() / ppp;
    let rest = at.x - whole;
    let galley = match galley.job.sections.first() {
        Some(_) if rest > 0.01 => {
            let mut job = (*galley.job).clone();
            if let Some(first) = job.sections.first_mut() {
                first.leading_space += rest;
            }
            // epaint counts leading space against the wrap width: widen it by as much, so a cut run is not cut again.
            if job.wrap.max_width.is_finite() && job.wrap.max_width >= 0.0 {
                job.wrap.max_width += rest;
            }
            on_baseline(painter.layout_job(job))
        }
        _ => galley,
    };
    painter.galley(pos2(whole, at.y), galley, colour);
}

/// Paints `galley` in the middle of `rect` as the design centres a text box: the box ([`run_box`]) in the middle,
/// on a half pixel when the room left over is odd, and the run at the box's left ([`run_at`]).
pub(crate) fn run_centred(painter: &Painter, rect: Rect, galley: Arc<Galley>, colour: Color32) {
    let size = galley.size();
    let at = pos2(rect.center().x - run_box(size.x) / 2.0, rect.center().y - size.y / 2.0);
    run_at(painter, at, galley, colour);
}

/// Paints the rows of the wrapped `galley` in a text box `width` wide at `at` as the design places them: each row at the
/// box's left, or in its middle (the galley's `halign`), a fraction of a pixel included, as [`run_at`] puts a run.
///
/// egui places a wrapped row's glyphs at the sub-pixel phase they had in the unwrapped line and puts a centred row on a
/// whole pixel, so a second row starts up to a pixel right of the first. Each row is laid out again alone here (the
/// row's words from the galley's own text, in the written order, at the breaks egui chose) and put where the design
/// puts it. A galley of more than one section is painted as it is.
pub(crate) fn rows_at(painter: &Painter, at: Pos2, width: f32, galley: Arc<Galley>, colour: Color32) {
    let align = galley.job.halign;
    let (Some(section), 1) = (galley.job.sections.first(), galley.job.sections.len()) else {
        painter.galley(at, galley, colour);
        return;
    };
    let text = galley.job.text.as_str();
    let mut start = 0_usize;
    // A row's top as the design has it: the line heights of the rows over it (egui rounds each).
    let mut top = 0.0_f32;
    let count = galley.rows.len();
    for (index, row) in galley.rows.iter().enumerate() {
        let row_top = top;
        top += row.glyphs.first().map_or(row.size.y, |glyph| glyph.line_height);
        // The row's characters, the newline it ends with included, follow the rows before it in the text.
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
        let one = on_baseline(painter.layout_job(job));
        let wide = one.size().x;
        let x = match align {
            egui::Align::Center => at.x + (width - wide) / 2.0,
            egui::Align::Max => at.x + width - wide,
            egui::Align::Min => at.x,
        };
        run_at(painter, pos2(x, at.y + row_top), one, colour);
    }
}

/// How many rows a physical pixel is sampled at by [`sampled_band`], as the design's renderer samples it.
const BAND_SAMPLES: usize = 4;

/// Paints the band from `top` to `bottom` (points) across `across` in `colour` as the design's renderer covers it: each
/// physical pixel row it touches in the share of its [`BAND_SAMPLES`] sample rows the band holds. A thin line (an
/// underline 0.66 high) then has the design's weight wherever its edges fall.
pub(crate) fn sampled_band(painter: &Painter, across: std::ops::RangeInclusive<f32>, (top, bottom): (f32, f32), colour: Color32) {
    let ppp = painter.pixels_per_point();
    if !ppp.is_finite() || ppp <= 0.0 || !top.is_finite() || !bottom.is_finite() || bottom <= top || bottom - top > 64.0 {
        return;
    }
    let (first, last) = ((top * ppp).floor(), (bottom * ppp).ceil());
    let mut row = first;
    while row < last {
        let hits = (0..BAND_SAMPLES).filter(|k| {
            let at = (row + (*k as f32 + 0.5) / BAND_SAMPLES as f32) / ppp;
            top <= at && at < bottom
        });
        let share = hits.count() as f32 / BAND_SAMPLES as f32;
        if share > 0.0 {
            // The colour itself over that share of the row's height: the row takes the share of it.
            let line = Rect::from_x_y_ranges(across.clone(), row / ppp..=(row + share) / ppp);
            painter.rect_filled(line, 0.0, colour);
        }
        row += 1.0;
    }
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

/// The scale `ppp` made safe (a scale that is not a positive number counts as 1), and how many whole physical pixels a
/// line one point wide takes there: one point rounded, at least one (1, 1, 2, 2 at 1, 1.25, 1.5 and 2). What a
/// [`hairline`] and a [`focus_ring`] share.
fn line_pixels(ppp: f32) -> (f32, f32) {
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    (ppp, ppp.round().max(1.0))
}

/// The shape of a [`hairline`] at `ppp` physical pixels per point: a line one point wide rounded to whole physical
/// pixels (at least one), across the physical pixels the ends fall in, its edges on pixel boundaries.
fn hairline_shape(a: Pos2, b: Pos2, colour: Color32, ppp: f32) -> Shape {
    let (ppp, pixels) = line_pixels(ppp);
    // The middle of a run of `pixels` physical pixels from the one `at` falls in.
    let snap = |at: f32| ((at * ppp).floor() + pixels / 2.0) / ppp;
    Shape::line_segment([pos2(snap(a.x), snap(a.y)), pos2(snap(b.x), snap(b.y))], Stroke::new(pixels / ppp, colour))
}

/// A line one point wide (whole physical pixels, at least one) in `colour` from `a` to `b`, on whole pixels.
pub(crate) fn hairline(painter: &Painter, a: Pos2, b: Pos2, colour: Color32) {
    painter.add(hairline_shape(a, b, colour, painter.pixels_per_point()));
}

/// A divider across the width of `ui`: a row one point high with a [`hairline`] in `colour` on it.
pub(crate) fn rule(ui: &mut Ui, colour: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    hairline(ui.painter(), rect.left_top(), rect.right_top(), colour);
    response
}

/// The panes of one frame. [`Panes::begin`] keeps a place under everything drawn after it; each [`Panes::card`]
/// draws its content and notes its rectangle; [`Panes::finish`] puts every pane's flat `surface` into the kept place,
/// with a `line` hairline on each edge that does not lie on the window's border, once where two panes meet, and none
/// where a pane meets a strip that draws its own edge ([`Panes::edged`], the rail).
#[derive(Debug)]
pub(crate) struct Panes {
    slot: ShapeIdx,
    rects: Vec<Rect>,
    edged: Vec<Rect>,
    capped: Vec<Rect>,
}

impl Panes {
    /// Keeps the place for the panes in `painter`'s list, after what it holds now.
    #[must_use]
    pub(crate) fn begin(painter: &Painter) -> Self {
        Self { slot: painter.add(Shape::Noop), rects: Vec::new(), edged: Vec::new(), capped: Vec::new() }
    }

    /// Notes `rect`, a strip above the panes that draws the line on its bottom edge itself (the error banner): a pane
    /// that starts under it draws no line on its top.
    pub(crate) fn capped(&mut self, rect: Option<Rect>) {
        self.capped.extend(rect);
    }

    /// Notes `rect`, a strip beside the panes that draws the line on its right edge itself (the rail, whose edge
    /// stands with or without a pane beside it): a pane that starts there draws no line on its left.
    pub(crate) fn edged(&mut self, rect: Rect) {
        self.edged.push(rect);
    }

    /// A pane over the whole of `ui`, with `add` drawing its content `inner` from its edges.
    pub(crate) fn card<R>(&mut self, ui: &mut Ui, inner: Margin, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.rects.push(ui.max_rect());
        Frame::new().inner_margin(inner).show(ui, add).inner
    }

    /// Puts the panes into the kept place of `painter`'s list.
    pub(crate) fn finish(self, painter: &Painter, theme: &Theme) {
        let window = painter.clip_rect();
        let ppp = painter.pixels_per_point();
        let colour = theme.palette.line;
        let near = |a: f32, b: f32| (a - b).abs() < 0.5;
        let mut shapes: Vec<Shape> = self.rects.iter().map(|rect| Shape::rect_filled(*rect, 0.0, theme.palette.surface)).collect();
        for rect in &self.rects {
            let (top, bottom) = (rect.top(), rect.bottom() - 1.0);
            let (left, right) = (rect.left(), rect.right() - 1.0);
            // A strip's own edge is the line between it and this pane.
            let beside_edge = self.edged.iter().any(|strip| near(strip.right(), rect.left()));
            if !near(rect.left(), window.left()) && !beside_edge {
                shapes.push(hairline_shape(pos2(left, top), pos2(left, rect.bottom()), colour, ppp));
            }
            // The next pane's left edge is this one's right edge: drawn once, by the next.
            let followed = self.rects.iter().any(|other| near(other.left(), rect.right()));
            if !near(rect.right(), window.right()) && !followed {
                shapes.push(hairline_shape(pos2(right, top), pos2(right, rect.bottom()), colour, ppp));
            }
            // A strip's own bottom edge is the line between it and this pane.
            let under_cap = self.capped.iter().any(|strip| near(strip.bottom(), rect.top()));
            if !near(rect.top(), window.top()) && !under_cap {
                shapes.push(hairline_shape(pos2(left, top), pos2(rect.right(), top), colour, ppp));
            }
            // The pane under this one starts at its bottom: that pane's top line is the one line between them.
            let stacked = self.rects.iter().any(|other| near(other.top(), rect.bottom()));
            if !near(rect.bottom(), window.bottom()) && !stacked {
                shapes.push(hairline_shape(pos2(left, bottom), pos2(rect.right(), bottom), colour, ppp));
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

/// The focus ring of a control over `rect`: a `cyan_edge` line one point wide, square, 2 points clear of the control
/// (`place`), on whole physical pixels at every scale: its clear edge put on the pixel nearest 2 points from the
/// control, its weight one point rounded to whole pixels, as a [`hairline`]'s (2 px at 1.5, where a 1 point line would
/// be 1.5 px and no pixel of it whole). At 1.0 a control on whole points keeps the ring it always had.
pub(crate) fn focus_ring(painter: &Painter, theme: &Theme, rect: Rect, place: Ring) {
    let (ppp, pixels) = line_pixels(painter.pixels_per_point());
    let snap = |at: f32| (at * ppp).round() / ppp;
    let on_pixels = |r: Rect| Rect::from_min_max(pos2(snap(r.left()), snap(r.top())), pos2(snap(r.right()), snap(r.bottom())));
    let stroke = Stroke::new(pixels / ppp, theme.palette.cyan_edge);
    match place {
        Ring::Outside => painter.rect_stroke(on_pixels(rect.expand(2.0)), 0.0, stroke, StrokeKind::Outside),
        Ring::Inside => painter.rect_stroke(on_pixels(rect.shrink(2.0)), 0.0, stroke, StrokeKind::Inside),
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

/// How many samples a pixel of a straight side takes along the side to measure how much of it a dash covers.
const SIDE_SAMPLES: usize = 4;

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

    /// The share of the pixel of a straight side from path position `at` to `at + 1` that lies in a dash, sampled at
    /// [`SIDE_SAMPLES`] even places along it, as the design's renderer samples it: a dash that begins or ends inside
    /// the pixel (after a round corner the phase is a fraction) covers it in part.
    fn share(self, at: f32) -> f32 {
        let step = 1.0 / SIDE_SAMPLES as f32;
        let hits = (0..SIDE_SAMPLES).filter(|k| self.on(at + (*k as f32 + 0.5) * step)).count();
        hits as f32 / SIDE_SAMPLES as f32
    }
}

/// [`dashed`] with corners of `radius` points.
///
/// The edge is laid out in physical pixels on the pixel grid: the box is rounded to it, the line lies inside the box,
/// as thick as a solid 1 point edge rounded to whole pixels, and the dashes and gaps are 2 points rounded to whole
/// pixels ([`DashScale`]). The dashes run along one path, their phase carried round each corner by the arc's length,
/// so after a round corner a dash may begin or end inside a pixel: a pixel of a straight side is drawn in the share of
/// it its dashes cover ([`DashScale::share`]), a corner's pixels in the share the arc's ring covers, both sampled.
/// Every pixel is painted once: at a square corner, where two sides cross, a pixel takes the larger share.
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
            let share = scale.share(start + d as f32);
            if share > 0.0 {
                for k in 0..thick {
                    let pixel = on.entry((x + dx * d + ix * k, y + dy * d + iy * k)).or_insert(0.0);
                    *pixel = pixel.max(share);
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
