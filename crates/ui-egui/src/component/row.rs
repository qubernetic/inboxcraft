// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What every list row shares (a mail row, a later row, an outbox row, a draft row): its height at the text scale,
//! the hover fill of the ground it stands on, the `stage` fill and the 2 px `cyan_edge` bar of the current row, a
//! `panel_line` hairline under it unless it is the last, and the focus edge 2 px inside it. The row's own content is
//! the caller's: [`row`] lays the row down and says where it is and how it stands, the caller draws its content, and
//! [`Row::finish`] then paints the focus edge over that content, so no fill, avatar or text of it can cover the edge.
//!
//! Forgetting [`Row::finish`] loses the focus edge, so it is hard to forget: `row` is `#[must_use]` and says so, and
//! `every_row_a_view_lays_down_is_finished` (`row_tests.rs`) fails when a source file lays down more rows (calls that
//! name a `RowGround`) than it finishes.
//!
//! The hover fill cross-fades over the `fast` token at full motion (the presentation's `.row` has `background
//! var(--fast)`), and comes at once with reduced motion; nothing else of the row moves. A row that is not in sight
//! paints nothing.

use egui::{Id, Rect, Response, Sense, Ui, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::motion;
use crate::theme::Theme;

use super::Shown;

/// A row laid down by [`row`]: where it is, its response (it takes clicks and the keyboard focus), and whether the
/// pointer is over it and whether it has the focus.
#[derive(Debug, Clone)]
pub struct Row {
    /// Its rectangle, the line under it included.
    pub rect: Rect,
    /// Its response: clicked when it was pressed; a caller names it with `widget_info`.
    pub response: Response,
    /// The pointer is over it (a hover action over it included).
    pub hovered: bool,
    /// It has the keyboard focus.
    pub focused: bool,
}

/// The ground a row stands on, which decides its hover fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowGround {
    /// A pane's `surface` (the mail list): hovered, the row takes `stage`.
    Surface,
    /// The `panel` (the index, a side panel): hovered, the row takes `grid`.
    Panel,
}

/// The width of the current row's bar at its left edge, in points.
const BAR: f32 = 2.0;

/// Lays down a row `id` across `ui`'s width, `height` points high at 100 % text (the theme's size of it), on `ground`;
/// `current` is the row shown in the reader (the `stage` fill and the cyan bar), `last` draws no line under it.
/// Paints its fill, bar and line; its focus edge comes with [`Row::finish`], after the caller's content. A row out of
/// sight is laid down and paints nothing. A row under 2 points high (one collapsing) paints no line.
#[must_use = "draw the row's content, then call `Row::finish`: it paints the focus edge"]
pub fn row(ui: &mut Ui, theme: &Theme, id: Id, height: f32, ground: RowGround, current: bool, last: bool) -> Row {
    row_with(ui, theme, id, height, ground, current, last, None)
}

/// [`row`], drawn hovered or focused as `shown` says whatever the pointer and the keyboard do (the gallery's board).
#[expect(clippy::too_many_arguments, reason = "the row's arguments and the state a board forces")]
#[must_use = "draw the row's content, then call `Row::finish`: it paints the focus edge"]
pub(crate) fn row_with(ui: &mut Ui, theme: &Theme, id: Id, height: f32, ground: RowGround, current: bool, last: bool, shown: Option<Shown>) -> Row {
    let p = &theme.palette;
    let points = if height.is_finite() { height.max(0.0) } else { 0.0 };
    let high = Theme::size(theme, points);
    let (_, rect) = ui.allocate_space(vec2(ui.available_width().max(0.0), high));
    let response = ui.interact(rect, id, Sense::click());
    let hovered = shown.map_or_else(|| response.contains_pointer(), |shown| shown == Shown::Hover);
    let focused = shown.map_or_else(|| response.has_focus(), |shown| shown == Shown::Focus);
    // The fade is kept for a row in sight only: one out of sight neither paints nor asks for frames.
    if !ui.is_rect_visible(rect) {
        return Row { rect, response, hovered, focused };
    }
    let (rest, lit) = match ground {
        RowGround::Surface => (p.surface, p.stage),
        RowGround::Panel => (p.panel, p.grid),
    };
    let fade = if shown.is_some() { f32::from(u8::from(hovered)) } else { motion::fade(ui.ctx(), id.with("hover"), hovered, theme.motion.fast) };
    let painter = ui.painter();
    if current {
        painter.rect_filled(rect, 0.0, p.stage);
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(BAR, rect.height())), 0.0, p.cyan_edge);
    } else if fade >= 1.0 {
        painter.rect_filled(rect, 0.0, lit);
    } else if fade > 0.0 {
        // Between the two token colours, never the hover colour faded by alpha.
        painter.rect_filled(rect, 0.0, rest.lerp_to_gamma(lit, fade));
    }
    if !last && rect.height() >= 2.0 {
        // From the middle of the row's first pixel to the middle of its last, on its last pixel row: inside the row.
        let y = rect.bottom() - 1.0;
        paint::hairline(painter, pos2(rect.left(), y), pos2(rect.right() - 1.0, y), p.panel_line);
    }
    Row { rect, response, hovered, focused }
}

/// The least height, in points, a row needs for its focus edge: 2 clear, the 1 px line, 2 clear, and the row's line.
const RING_ROOM: f32 = 6.0;

impl Row {
    /// Paints the row's focus edge when it has the focus: 2 px inside the row above its line (the sheet's 2,2 436 × 51
    /// on a row of 440 × 56), over everything the caller drew in the row. Call it once, after the row's content.
    pub fn finish(&self, ui: &Ui, theme: &Theme) {
        let rect = self.rect;
        if !self.focused || rect.height() < RING_ROOM || rect.width() < RING_ROOM || !ui.is_rect_visible(rect) {
            return;
        }
        let above = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 1.0));
        let painter = ui.painter();
        let ppp = painter.pixels_per_point();
        if (ppp - 1.0).abs() < f32::EPSILON || !ppp.is_finite() || ppp <= 0.0 {
            paint::focus_ring(painter, theme, above, Ring::Inside);
            return;
        }
        // At another scale [`paint::focus_ring`]'s 1 point line falls between pixels (1.5 px at 1.5): the edge is laid on
        // whole pixels instead, as a hairline is, its line one point rounded to whole pixels.
        let ring = above.shrink(2.0);
        let snap = |at: f32| (at * ppp).round() / ppp;
        let ring = Rect::from_min_max(pos2(snap(ring.left()), snap(ring.top())), pos2(snap(ring.right()), snap(ring.bottom())));
        let width = (ppp.round().max(1.0)) / ppp;
        painter.rect_stroke(ring, 0.0, egui::Stroke::new(width, theme.palette.cyan_edge), egui::StrokeKind::Inside);
    }
}

#[cfg(test)]
#[path = "row_tests.rs"]
mod tests;
