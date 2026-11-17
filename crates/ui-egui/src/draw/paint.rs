// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The small pieces the panes are drawn from: one line of text cut to a width, the selection, a navigation row, a
//! badge, and the flat grounds: a pane, a float with the one shadow, a hairline.

use std::sync::Arc;

use egui::layers::ShapeIdx;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, Frame, Galley, Margin, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::component::button::DISABLED;
use crate::iconset::{IconId, Style};
use crate::theme::{FLOAT_SHADOW, Font, Theme};

/// One line of `sections` (text, font token and colour), cut with an ellipsis at `width`.
pub(super) fn line(ui: &Ui, sections: &[(&str, Font, Color32)], width: f32) -> Arc<Galley> {
    let mut job = LayoutJob {
        wrap: TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') },
        break_on_newline: false,
        ..LayoutJob::default()
    };
    for (text, font, colour) in sections {
        job.append(text, 0.0, TextFormat::simple(font.id(), *colour));
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
        Shape::Rect(FLOAT_SHADOW.as_shape(rect, 0.0)),
        Shape::rect_filled(rect, 0.0, p.surface),
        Shape::rect_stroke(rect, 0.0, Stroke::new(1.0, colour), StrokeKind::Inside),
    ]
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

/// The height of one line of `font`, from the font's own metrics.
pub(super) fn line_height(ui: &Ui, font: Font) -> f32 {
    ui.fonts_mut(|fonts| fonts.row_height(&font.id()))
}

/// The selection over `rect`: one even tone.
pub(super) fn selection(painter: &Painter, theme: &Theme, rect: Rect, corner: f32) {
    painter.rect_filled(rect, corner, theme.palette.stage);
}

/// The focus ring around a control that has the keyboard focus.
pub(super) fn focus_ring(ui: &Ui, theme: &Theme, response: &Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(response.rect.shrink(1.0), 0.0, Stroke::new(2.0, theme.palette.cyan_edge), StrokeKind::Inside);
    }
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
    /// The icon before the text, in the muted colour (the accent while selected).
    pub(super) icon: Option<IconId>,
    /// Whether it can be pressed; one that cannot is faded and takes no press.
    pub(super) enabled: bool,
}

/// A row of the folder pane: a unified view or a folder, with its unread badge.
pub(super) fn nav_row(ui: &mut Ui, theme: &Theme, row: &NavRow<'_>) -> Response {
    let s = &theme.spacing;
    let p = &theme.palette;
    let height = line_height(ui, theme.fonts.body) + s.s2 * 1.5;
    let sense = if row.enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), sense);
    response.widget_info(|| WidgetInfo::selected(Role::Button, row.enabled, row.selected, &row.spoken));
    if ui.is_rect_visible(rect) {
        let mut faded = ui.painter().clone();
        faded.multiply_opacity(if row.enabled { 1.0 } else { DISABLED });
        let painter = &faded;
        if row.selected {
            selection(ui.painter(), theme, rect, 0.0);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, p.stage);
        }
        let depth = u16::try_from(row.depth.min(8)).unwrap_or(8);
        let mut left = rect.left() + s.s2 + f32::from(depth) * s.s3;
        if let Some(icon) = row.icon {
            let side = theme.fonts.body.size * 1.15;
            let at = Rect::from_center_size(pos2(left + side / 2.0, rect.center().y), vec2(side, side));
            let (style, ink) = if row.selected { (Style::Fill, p.cyan_text) } else { (Style::Regular, p.muted) };
            crate::iconset::paint(painter, at, icon, style, ink);
            left += side + s.s2;
        }
        let right = if row.badge > 0 { badge(ui, theme, row.badge, rect.right() - s.s2, rect.center().y) - s.s2 } else { rect.right() - s.s2 };
        let galley = line(ui, &[(row.text, theme.fonts.body, p.ink)], right - left);
        painter.galley(pos2(left, rect.center().y - galley.size().y / 2.0), galley, p.ink);
        focus_ring(ui, theme, &response);
    }
    response
}

#[cfg(test)]
#[path = "paint_tests.rs"]
mod tests;
