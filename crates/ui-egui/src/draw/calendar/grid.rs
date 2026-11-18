// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar's grids on whole physical pixels. A grid line is one point wide rounded to whole pixels (one pixel at
//! 1.0 and 1.25, two at 1.5 and 2.0, as a hairline), and its left (or top) edge lies on a pixel boundary: the lines of
//! `count` columns between two edges are the edges cut at `count` evenly spaced places, each rounded to the nearest
//! pixel, so the columns differ by a pixel at most and none is a fraction of one.
//!
//! An event is a cell on the grid: its edges take the pixels of the grid's own lines. A cell from line `a` to line `b`
//! reaches from `a`'s first pixel to `b`'s last ([`Lines::cell`]), so the event's 1 px edge lies on the lines and no
//! second line stands beside them. Two events side by side share the line between them; two bands one above the other
//! share one ([`lane_pitch`]).

/// The most lines a grid is cut into: a week's seven columns, an hour grid's 24 rows, a month's six weeks, and the
/// sub-columns of overlapping events ([`crate::draw::calendar`] bounds them lower).
const MOST: usize = 64;

/// `ppp` made safe: a scale that is not a positive number counts as 1.
#[must_use]
pub(super) fn safe(ppp: f32) -> f32 {
    if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 }
}

/// `at` on the nearest pixel boundary at `ppp`, in points; a place that is not a number is 0.
#[must_use]
pub(super) fn snap(at: f32, ppp: f32) -> f32 {
    let ppp = safe(ppp);
    let px = (at * ppp).round();
    if px.is_finite() { px / ppp } else { 0.0 }
}

/// How wide a grid line is at `ppp`, in points: one point rounded to whole pixels, at least one pixel.
#[must_use]
pub(super) fn line(ppp: f32) -> f32 {
    let ppp = safe(ppp);
    ppp.round().max(1.0) / ppp
}

/// `points` rounded to whole pixels, at least one: a bar's width.
#[must_use]
pub(super) fn pixels(points: f32, ppp: f32) -> f32 {
    let ppp = safe(ppp);
    let px = (points * ppp).round();
    if px.is_finite() { px.max(1.0) / ppp } else { 1.0 / ppp }
}

/// The lines of a grid at one scale: where each starts, in points on pixel boundaries, and how wide they are.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Lines {
    /// Where each line starts, first to last.
    pub at: Vec<f32>,
    /// A line's width.
    pub width: f32,
}

impl Lines {
    /// The `count + 1` lines of `count` columns from `from` to `to` (`count` at most [`MOST`], at least one).
    #[must_use]
    pub(super) fn cut(from: f32, to: f32, count: usize, ppp: f32) -> Self {
        let count = count.clamp(1, MOST);
        let (from, to) = (if from.is_finite() { from } else { 0.0 }, if to.is_finite() { to } else { 0.0 });
        let to = to.max(from);
        let step = (to - from) / count as f32;
        let at = (0..=count).map(|index| snap(from + step * index as f32, ppp)).collect();
        Self { at, width: line(ppp) }
    }

    /// How many columns the lines hold.
    #[cfg(test)]
    #[must_use]
    pub(super) fn columns(&self) -> usize {
        self.at.len().saturating_sub(1)
    }

    /// Where line `index` starts; the last line for an index past it.
    #[must_use]
    pub(super) fn line(&self, index: usize) -> f32 {
        self.at.get(index).or_else(|| self.at.last()).copied().unwrap_or(0.0)
    }

    /// The span of a cell from line `from` to line `to`, both lines' pixels included: an event's left and right.
    #[must_use]
    pub(super) fn cell(&self, from: usize, to: usize) -> (f32, f32) {
        let (a, b) = (self.line(from), self.line(to));
        (a, b.max(a) + self.width)
    }

    /// The span inside column `index`, between its lines: what a column's own ground and text take.
    #[must_use]
    pub(super) fn inside(&self, index: usize) -> (f32, f32) {
        let (a, b) = (self.line(index) + self.width, self.line(index.saturating_add(1)));
        (a, b.max(a))
    }
}

/// From one band's top to the next one's: a band one `lane` high less its 1 px edge, so two bands share one line.
#[must_use]
pub(super) fn lane_pitch(lane: f32, ppp: f32) -> f32 {
    (snap(lane, ppp) - line(ppp)).max(line(ppp))
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
