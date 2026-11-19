// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frame's four regions: the command line across the top, the index at the left, the body beside it and the
//! status line across the foot. Measured from the frame sheet of the shell (1440 × 900): the window's outline, one
//! hairline round everything; the command line 44 high; a hairline under it; the index 232 wide (56 when narrow), a
//! hairline, the body; a hairline over the status line, 26 high. At 1440 × 900 the body is 1205 × 826 at (234, 46).
//! The window lays itself out in them each frame; the status line holds nothing yet.

use egui::{Context, Id, Rect, Vec2, pos2};

use crate::theme::Theme;

/// The command line's height at 100 %.
pub(crate) const COMMAND_LINE: f32 = 44.0;
/// The status line's height at 100 %.
pub(crate) const STATUS_LINE: f32 = 26.0;
/// The index's width.
pub(crate) const INDEX: f32 = 232.0;
/// The index's width when narrow.
pub(crate) const INDEX_NARROW: f32 = 56.0;
/// Under this window width the index is the narrow one.
#[cfg_attr(not(test), expect(dead_code, reason = "the window keeps the wide index until the widths slice"))]
pub(crate) const NARROW_INDEX_UNDER: f32 = 1280.0;
/// The least width the window is laid out at; a narrower one is clipped.
pub(crate) const LEAST_WIDTH: f32 = 800.0;

/// The frame's four regions, in points, inside the window's outline with a hairline between each two.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Regions {
    /// The command line, across the top.
    pub command: Rect,
    /// The index, at the left between the command and status lines.
    pub index: Rect,
    /// The body, right of the index.
    pub body: Rect,
    /// The status line, across the foot.
    pub status: Rect,
}

/// Whole-pixel rects for a window of `size` points at `theme`'s text scale and `ppp` physical pixels per point; the
/// index narrow from the width alone (under [`NARROW_INDEX_UNDER`]).
#[cfg_attr(not(test), expect(dead_code, reason = "the window keeps the wide index until the widths slice"))]
pub(crate) fn regions(size: Vec2, theme: &Theme, ppp: f32) -> Regions {
    let narrow = number(size.x, LEAST_WIDTH) < NARROW_INDEX_UNDER;
    regions_with(size, theme, ppp, narrow)
}

/// [`regions`] with the index `narrow` or not, whatever the width.
pub(crate) fn regions_with(size: Vec2, theme: &Theme, ppp: f32, narrow: bool) -> Regions {
    // A scale from the desktop is kept to one a screen can have: far outside it, a pixel count overflows or rounds to
    // nothing, and the bounds below would cross.
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp.clamp(LEAST_PPP, MOST_PPP) } else { 1.0 };
    // In physical pixels; one hairline is one point rounded to whole pixels, at least one.
    let px = |points: f32| (points * ppp).round().max(0.0);
    let line = ppp.round().max(1.0);
    let factor = theme.text_scale.factor();
    let command = px(even(COMMAND_LINE * factor));
    let status = px(even(STATUS_LINE * factor));
    let index = px(if narrow { INDEX_NARROW } else { INDEX });
    // The least height: the outline, the two lines and their hairlines, and a body of none.
    let least_height = (command + status + 4.0 * line) / ppp;
    let width = px(number(size.x, LEAST_WIDTH).clamp(LEAST_WIDTH, MOST));
    let height = px(number(size.y, least_height).max(least_height).min(MOST.max(least_height)));
    let middle_top = line + command + line;
    let status_top = (height - line - status).max(middle_top + line);
    let middle_bottom = status_top - line;
    let body_left = line + index + line;
    let right = (width - line).max(body_left);
    let at = |left: f32, top: f32, right: f32, bottom: f32| Rect::from_min_max(pos2(left / ppp, top / ppp), pos2(right / ppp, bottom / ppp));
    Regions {
        command: at(line, line, right, line + command),
        index: at(line, middle_top, line + index, middle_bottom),
        body: at(body_left, middle_top, right, middle_bottom),
        status: at(line, status_top, right, status_top + status),
    }
}

/// The largest size laid out, in points: beyond it a size is no window's.
const MOST: f32 = 1.0e6;

/// The least and the most physical pixels per point laid out: any screen's scale lies between.
const LEAST_PPP: f32 = 0.25;
const MOST_PPP: f32 = 16.0;

/// `value`, or `or` when it is no number.
fn number(value: f32, or: f32) -> f32 {
    if value.is_nan() { or } else { value }
}

/// `value` to the nearest even whole number.
fn even(value: f32) -> f32 {
    (value / 2.0).round() * 2.0
}

impl Regions {
    /// The regions moved `by`: laid out from the window's top left.
    #[must_use]
    pub(crate) fn translate(self, by: Vec2) -> Self {
        Self {
            command: self.command.translate(by),
            index: self.index.translate(by),
            body: self.body.translate(by),
            status: self.status.translate(by),
        }
    }
}

/// Notes the regions laid out this frame.
pub(crate) fn lay(ctx: &Context, regions: Regions) {
    ctx.data_mut(|data| data.insert_temp(id(), regions));
}

/// The regions laid out in the last frame.
#[cfg(test)]
pub(crate) fn laid(ctx: &Context) -> Option<Regions> {
    ctx.data(|data| data.get_temp(id()))
}

fn id() -> Id {
    Id::unique("shell-regions")
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
