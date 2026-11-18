// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Motion: short, snappy, nothing decorative. Every duration is a token of [`MOTION`], and every animation in the
//! crate runs through [`toggle`] or [`appear`], which read the one reduced-motion flag ([`set_reduced`]): with it
//! on, everything lands at its end in the frame it changes. A test refuses egui's animation calls anywhere else.
//!
//! The flag's default follows the system where the system can be read ([`system_default`]); otherwise motion is on.
//! With the flag on, egui's own animations (areas fading in, scrolling to a place) stop too. egui's smoothing of a
//! mouse wheel's notches is fixed inside egui and keeps running.

use egui::{Context, Id, Style, emath::easing, style::ScrollAnimation};

/// The durations of the motion tokens, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// Hover actions fading in over a row, and the hover of the controls not yet restyled to the design (checkbox,
    /// pill, select, segmented control, popup rows, card, time picker); the restyled ones take [`Motion::fast`] or
    /// change at once, as the presentation has them.
    pub hover_fade: f32,
    /// The `fast` token: a control's colours cross-fading between rest and hover, linear.
    pub fast: f32,
    /// A tooltip appearing.
    pub tooltip: f32,
    /// The `row_collapse` token: the gap of a row that left the list (Done, Snooze) closing.
    pub row_collapse: f32,
    /// A card or a popup opening and closing.
    pub card: f32,
    /// A toast rising from the bottom edge.
    pub toast: f32,
}

impl Motion {
    /// Every token.
    #[must_use]
    pub const fn all(&self) -> [f32; 6] {
        [self.hover_fade, self.fast, self.tooltip, self.row_collapse, self.card, self.toast]
    }
}

/// The motion tokens.
pub const MOTION: Motion = Motion { hover_fade: 0.12, fast: 0.125, tooltip: 0.1, row_collapse: 0.18, card: 0.16, toast: 0.2 };

/// No token is longer than this.
pub const LONGEST: f32 = 0.25;

/// Where the flag lives in the context.
fn flag() -> Id {
    Id::unique("inboxcraft-reduced-motion")
}

/// The reduced-motion flag's default: the system's wish where it can be read (`Some`, the web build's
/// `prefers-reduced-motion`), otherwise motion on.
#[must_use]
pub fn system_default(system: Option<bool>) -> bool {
    system.unwrap_or(false)
}

/// Turns reduced motion on or off in `ctx`, egui's fades and scrolling to a place included, from this frame.
pub fn set_reduced(ctx: &Context, reduced: bool) {
    ctx.data_mut(|data| data.insert_temp(flag(), reduced));
    let time = if reduced { 0.0 } else { MOTION.tooltip };
    let scroll = if reduced { ScrollAnimation::none() } else { ScrollAnimation::default() };
    let style = ctx.global_style();
    if style.animation_time != time || style.scroll_animation != scroll {
        ctx.all_styles_mut(|style| {
            style.animation_time = time;
            style.scroll_animation = scroll;
        });
    }
}

/// Whether reduced motion is on in `ctx`.
#[must_use]
pub fn reduced(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp(flag())).unwrap_or(false)
}

/// Sets egui's own animation time in `style` to the tooltip token: egui fades its tooltips in over it.
pub(crate) fn tune(style: &mut Style) {
    style.animation_time = MOTION.tooltip;
}

/// How long an animation of `seconds` really runs in `ctx`: nothing with reduced motion or a duration that is not
/// a real positive number.
fn duration(ctx: &Context, seconds: f32) -> f32 {
    if reduced(ctx) || !seconds.is_finite() || seconds <= 0.0 { 0.0 } else { seconds }
}

/// How far `id` has moved towards `on` (1) or off (0) over `seconds`, eased; a repaint is asked for while it moves.
/// The first value of an id is its end.
pub fn toggle(ctx: &Context, id: Id, on: bool, seconds: f32) -> f32 {
    let seconds = duration(ctx, seconds);
    ctx.animate_bool_with_time_and_easing(id, on, seconds, easing::cubic_out)
}

/// [`toggle`] at an even pace: a cross-fade of colours, as the design's `linear` transitions run.
pub fn fade(ctx: &Context, id: Id, on: bool, seconds: f32) -> f32 {
    let seconds = duration(ctx, seconds);
    ctx.animate_bool_with_time_and_easing(id, on, seconds, easing::linear)
}

/// How far `id` has appeared over `seconds` since the first frame it was shown in, eased, 0 to 1; something that
/// missed a frame appears again. A repaint is asked for until it has.
pub fn appear(ctx: &Context, id: Id, seconds: f32) -> f32 {
    easing::cubic_out(elapsed(ctx, id, seconds))
}

/// The share of `seconds` gone since the first frame `id` was shown in, at an even pace, 0 to 1; something that missed
/// a frame starts again. A repaint is asked for until it is 1; with reduced motion it is 1 at once.
pub fn elapsed(ctx: &Context, id: Id, seconds: f32) -> f32 {
    let now = ctx.input(|input| input.time);
    let pass = ctx.cumulative_pass_nr();
    let key = id.with("appear");
    let start = ctx.data_mut(|data| {
        let start = match data.get_temp::<(f64, u64)>(key) {
            Some((start, last)) if last.saturating_add(1) >= pass => start,
            _ => now,
        };
        data.insert_temp(key, (start, pass));
        start
    });
    let seconds = duration(ctx, seconds);
    if seconds <= 0.0 {
        return 1.0;
    }
    // A share of a fraction of a second: the cast loses nothing that shows.
    let shown = (((now - start) / f64::from(seconds)) as f32).clamp(0.0, 1.0);
    if shown < 1.0 {
        ctx.request_repaint();
    }
    shown
}

/// The design's one curve, `cubic-bezier(.2, .7, .2, 1)`: how far a movement has come at `share` of its time (0 to
/// 1; below 0 or not a number counts as 0, above 1 as 1). The curve's time is found by halving its interval a fixed
/// number of times (its x rises from 0 to 1), so it always ends.
#[must_use]
pub fn ease(share: f32) -> f32 {
    cubic_bezier([0.2, 0.7, 0.2, 1.0], share)
}

/// CSS's `ease`, `cubic-bezier(.25, .1, .25, 1)`, the curve of a transition that names none: the presentation fades a
/// leaving row's opacity on it beside its height on [`ease`]. Ends and nonsense as [`ease`].
#[must_use]
pub fn css_ease(share: f32) -> f32 {
    cubic_bezier([0.25, 0.1, 0.25, 1.0], share)
}

/// The curve `cubic-bezier(x1, y1, x2, y2)` (x1 and x2 within 0 to 1) at `share` of its time.
fn cubic_bezier([x1, y1, x2, y2]: [f64; 4], share: f32) -> f32 {
    let share = if share.is_nan() { 0.0 } else { f64::from(share.clamp(0.0, 1.0)) };
    // The ends exactly: a movement starts where it was and lands where it goes.
    if share <= 0.0 {
        return 0.0;
    }
    if share >= 1.0 {
        return 1.0;
    }
    let bezier = |p1: f64, p2: f64, t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t * t * t;
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for _ in 0..48 {
        let middle = (low + high) / 2.0;
        if bezier(x1, x2, middle) < share {
            low = middle;
        } else {
            high = middle;
        }
    }
    // A share of 0 to 1: the cast loses nothing that shows.
    (bezier(y1, y2, (low + high) / 2.0) as f32).clamp(0.0, 1.0)
}

#[cfg(test)]
#[path = "motion_tests.rs"]
pub(crate) mod tests;
