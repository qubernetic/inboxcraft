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
    /// Hover actions fading in over a row, a control's hover fill.
    pub hover_fade: f32,
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
    pub const fn all(&self) -> [f32; 5] {
        [self.hover_fade, self.tooltip, self.row_slide, self.card, self.toast]
    }
}

/// The motion tokens.
pub const MOTION: Motion = Motion { hover_fade: 0.12, tooltip: 0.1, row_slide: 0.18, card: 0.16, toast: 0.2 };

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

#[cfg(test)]
#[path = "motion_tests.rs"]
pub(crate) mod tests;
