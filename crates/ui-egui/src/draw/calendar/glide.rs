// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar's motion, as the design's presentation moves an event: a timed event whose time changed in place (an
//! edit saved, an Undo) glides from where it stood to its new cell over `medium` (300 ms) on the one curve,
//! `cubic-bezier(.2, .7, .2, 1)`; a block's ground fades to `stage` and back over `fast` (125 ms), linearly. Each
//! asks for repaints only while it runs, and with motion reduced or off the event is at once where it goes. A change
//! of view or of the days shown moves nothing (M12: no motion for another view or range).

use egui::{Context, Id, Rect, pos2};

use crate::calendar::{Day, EventRow};

/// How long a glide takes, in seconds: the design's `medium`.
const MEDIUM: f32 = 0.3;

/// The design's one curve, `cubic-bezier(.2, .7, .2, 1)`, at `t` (0 to 1): the share of the way at that share of the
/// time. A local helper until the motion module has one of its own.
#[must_use]
pub(super) fn ease(t: f32) -> f32 {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 1.0 };
    let (x1, y1, x2, y2) = (0.2_f32, 0.7_f32, 0.2_f32, 1.0_f32);
    let bezier = |s: f32, a: f32, b: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s;
    // x grows with s on this curve: halving finds the s whose x is t, in a fixed count of steps.
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if bezier(middle, x1, x2) < t {
            low = middle;
        } else {
            high = middle;
        }
    }
    bezier((low + high) / 2.0, y1, y2)
}

/// Where an event's glide is: from, to, since when, the pass it was last drawn in and the days shown then.
#[derive(Debug, Clone, Copy)]
struct Glide {
    from: Rect,
    to: Rect,
    since: f64,
    pass: u64,
    span: (Day, Day),
}

impl Glide {
    /// Where it stands at `now`: its cell once landed, else on its way (the caller puts it on whole pixels).
    fn at(&self, now: f64) -> Rect {
        // A share of a fraction of a second: the cast loses nothing that shows.
        let share = ease((((now - self.since) / f64::from(MEDIUM)) as f32).clamp(0.0, 1.0));
        if share >= 1.0 || self.from == self.to {
            return self.to;
        }
        let lerp = |a: f32, b: f32| a + (b - a) * share;
        Rect::from_min_max(
            pos2(lerp(self.from.left(), self.to.left()), lerp(self.from.top(), self.to.top())),
            pos2(lerp(self.from.right(), self.to.right()), lerp(self.from.bottom(), self.to.bottom())),
        )
    }
}

/// Where the timed `event`, whose cell in its day `day` (the column it is drawn in) is `rect` among the days `span`
/// (measured from the top of the hours, so a scroll moves nothing), is drawn now: its cell, or on its way there from where it stood when its time changed that day. A repeating event's
/// occurrences share one id, so they never glide.
pub(super) fn glide(ctx: &Context, event: &EventRow, (span, day): ((Day, Day), i64), rect: Rect) -> Rect {
    if event.repeat.is_some() || crate::motion::reduced(ctx) {
        return rect;
    }
    let id = Id::unique(("calendar-glide", event.id, day));
    let now = ctx.input(|input| input.time);
    let pass = ctx.cumulative_pass_nr();
    let kept = ctx.data(|data| data.get_temp::<Glide>(id)).filter(|kept| kept.span == span && kept.pass.saturating_add(1) >= pass);
    let next = match kept {
        // Wider or narrower (the window, a neighbour coming or going): at once where it goes.
        Some(kept) if kept.to.x_range() != rect.x_range() => Glide { from: rect, to: rect, since: now, pass, span },
        // Its time moved: from where it stands now to the new cell.
        Some(kept) if kept.to != rect => Glide { from: kept.at(now), to: rect, since: now, pass, span },
        Some(kept) => Glide { pass, ..kept },
        None => Glide { from: rect, to: rect, since: now, pass, span },
    };
    ctx.data_mut(|data| data.insert_temp(id, next));
    let shown = next.at(now);
    if shown != rect {
        ctx.request_repaint();
    }
    shown
}

#[cfg(test)]
#[path = "glide_tests.rs"]
mod tests;
