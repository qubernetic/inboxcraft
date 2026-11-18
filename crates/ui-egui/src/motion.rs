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
    /// A row sliding out of the list when it is done.
    pub row_slide: f32,
    /// A card or a popup opening and closing.
    pub card: f32,
    /// A toast rising from the bottom edge.
    pub toast: f32,
}

impl Motion {
    /// Every token.
    #[must_use]
    pub const fn all(&self) -> [f32; 6] {
        [self.hover_fade, self.fast, self.tooltip, self.row_slide, self.card, self.toast]
    }
}

/// The motion tokens.
pub const MOTION: Motion = Motion { hover_fade: 0.12, fast: 0.125, tooltip: 0.1, row_slide: 0.18, card: 0.16, toast: 0.2 };

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

/// [`toggle`] on `easing`: a transition on a curve of its own (the calendar's ground on CSS's `ease`).
pub fn fade_eased(ctx: &Context, id: Id, on: bool, seconds: f32, easing: fn(f32) -> f32) -> f32 {
    let seconds = duration(ctx, seconds);
    ctx.animate_bool_with_time_and_easing(id, on, seconds, easing)
}

/// How far `id` has appeared over `seconds` since the first frame it was shown in, eased, 0 to 1; something that
/// missed a frame appears again. A repaint is asked for until it has.
pub fn appear(ctx: &Context, id: Id, seconds: f32) -> f32 {
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
    easing::cubic_out(shown)
}

/// The design's one curve, `cubic-bezier(.2, .7, .2, 1)`, at `t` (0 to 1): the share of the way at that share of the
/// time; a `t` that is not a number is the end (1).
#[must_use]
pub fn ease(t: f32) -> f32 {
    bezier(t, (0.2, 0.7, 0.2, 1.0))
}

/// CSS's `ease`, `cubic-bezier(.25, .1, .25, 1)`: the curve of a transition that names none, as the design's fade of
/// an event's ground (`background var(--fast)`) does.
#[must_use]
pub fn css_ease(t: f32) -> f32 {
    bezier(t, (0.25, 0.1, 0.25, 1.0))
}

/// The cubic Bézier from (0, 0) to (1, 1) through `(x1, y1, x2, y2)` at `t`, as CSS's `cubic-bezier` (x1 and x2
/// between 0 and 1).
fn bezier(t: f32, (x1, y1, x2, y2): (f32, f32, f32, f32)) -> f32 {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 1.0 };
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

/// One turn of the caret's blink, in seconds (the `caret` token, 1 000 ms): on for the first half, off for the second.
pub const CARET: f64 = 1.0;

/// From one line of an unfolding list to the next, in seconds (the `stagger` token, 30 ms).
pub const STAGGER: f32 = 0.03;

/// The longest wait [`blink`] asks a repaint after, in milliseconds: half a turn of [`CARET`].
const HALF_TURN_MS: f64 = 500.0;

/// Whether a caret that last moved at `since` (an input time, in seconds) shows now: on for the first half of each
/// turn of [`CARET`], off for the second, at every level the app has (the design's caret blinks at full and at
/// reduced). A repaint is asked for at its next change, never sooner. `still` keeps it on and asks for nothing (a
/// caret drawn on a board, or the design's `off` level once the app has it).
pub fn blink(ctx: &Context, since: f64, still: bool) -> bool {
    if still {
        return true;
    }
    let now = ctx.input(|input| input.time);
    let age = if (now - since).is_finite() { (now - since).max(0.0) } else { 0.0 };
    let phase = age.rem_euclid(CARET) * 1000.0;
    let (on, next) = if phase < HALF_TURN_MS { (true, HALF_TURN_MS - phase) } else { (false, HALF_TURN_MS * 2.0 - phase) };
    // Between 1 and 500 ms, so the cast keeps it.
    let wait = next.ceil().clamp(1.0, HALF_TURN_MS) as u64;
    ctx.request_repaint_after(std::time::Duration::from_millis(wait));
    on
}

/// How far line `index` of a list that opened at `opened` (an input time, in seconds) has come in, 0 to 1: each line
/// [`STAGGER`] after the one above, over `seconds` (the `fast` token), eased (the presentation's `line-in`). A
/// repaint is asked for until it is in. With reduced motion every line is in at once.
pub fn unfold(ctx: &Context, opened: f64, index: usize, seconds: f32) -> f32 {
    let seconds = duration(ctx, seconds);
    if seconds <= 0.0 {
        return 1.0;
    }
    let now = ctx.input(|input| input.time);
    // A list has a few dozen lines at most; past 1 000 they all start together.
    let delay = f64::from(STAGGER) * index.min(1000) as f64;
    let age = now - opened - delay;
    if !age.is_finite() {
        return 1.0;
    }
    // A share of a fraction of a second: the cast loses nothing that shows.
    let shown = ((age / f64::from(seconds)) as f32).clamp(0.0, 1.0);
    if shown < 1.0 {
        ctx.request_repaint();
    }
    easing::cubic_out(shown)
}

#[cfg(test)]
#[path = "motion_tests.rs"]
pub(crate) mod tests;
