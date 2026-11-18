// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What every list row shares (a mail row, a later row, an outbox row, a draft row): its height at the text scale,
//! the hover fill of the ground it stands on, the `stage` fill and the 2 px `cyan_edge` bar of the current row, a
//! `panel_line` hairline under it unless it is the last, and the focus edge 2 px inside it. The row's own content is
//! the caller's: [`row`] lays the row down and says where it is and how it stands, the caller draws its content, and
//! [`Row::finish`] then paints the focus edge over that content, so no fill, avatar or text of it can cover the edge.
//!
//! Forgetting [`Row::finish`] loses the focus edge, so it is made hard to forget: `row` is `#[must_use]` and says so,
//! and `every_row_a_view_lays_down_is_finished` (`row_tests.rs`) fails when a source file binds a row with `let` and
//! never calls `.finish(` on that name later. That scan is a heuristic; its doc says what it does not see.
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
    /// How high the foot of the row is that its focus edge stays above, in points: its line (1), or what a row draws
    /// along its foot over the line (the outbox row's hold, 2; the sheet's focus edge of 436 × 50).
    pub foot: f32,
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
    // Rows touch: a row owns its top edge and not its bottom one, so the pointer on a shared edge is the lower row's.
    let below_top = || ui.ctx().pointer_latest_pos().is_some_and(|at| at.y < rect.bottom());
    let hovered = shown.map_or_else(|| response.contains_pointer() && below_top(), |shown| shown == Shown::Hover);
    let focused = shown.map_or_else(|| response.has_focus(), |shown| shown == Shown::Focus);
    // The fade is kept for a row in sight only: one out of sight neither paints nor asks for frames.
    if !ui.is_rect_visible(rect) {
        return Row { rect, response, hovered, focused, foot: 1.0 };
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
    Row { rect, response, hovered, focused, foot: 1.0 }
}

/// The least height, in points, a row needs for its focus edge: 2 clear, the 1 px line, 2 clear, and the row's line.
const RING_ROOM: f32 = 6.0;

impl Row {
    /// Paints the row's focus edge when it has the focus: 2 px inside the row above its foot (the sheet's 2,2 436 × 51
    /// on a row of 440 × 56 over its line), over everything the caller drew in the row. Call it once, after the row's
    /// content.
    pub fn finish(&self, ui: &Ui, theme: &Theme) {
        let rect = self.rect;
        let foot = if self.foot.is_finite() { self.foot.clamp(0.0, rect.height().max(0.0)) } else { 1.0 };
        if !self.focused || rect.height() < RING_ROOM - 1.0 + foot || rect.width() < RING_ROOM || !ui.is_rect_visible(rect) {
            return;
        }
        let above = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - foot));
        paint::focus_ring(ui.painter(), theme, above, Ring::Inside);
    }
}

#[cfg(test)]
#[path = "row_tests.rs"]
mod tests;
