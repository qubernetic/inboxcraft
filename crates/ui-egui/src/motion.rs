// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Motion: short, snappy, nothing decorative. Every duration is a token of [`MOTION`], and every animation in the
//! crate runs through [`toggle`] or [`appear`], which read the one motion level ([`set_level`], [`level`]): at a
//! level that is [`Level::still`] everything lands at its end in the frame it changes. A test refuses egui's
//! animation calls anywhere else.
//!
//! The window sets the level each frame from the owner's choice ([`follow`]); while the owner has not chosen, the
//! computer's wish for less motion gives [`Level::Off`] where the computer can be read ([`set_system_wish`]; the
//! desktop cannot read it yet, and then the owner's default level holds). The movements of the design's table read
//! the level ([`level`]) and play the form it gives them; what has no reduced form of its own yet lands at its end at
//! Reduced and Off ([`Level::still`]). With a still level egui's own animations (areas fading in, scrolling to a place)
//! stop too. egui's smoothing of a mouse wheel's notches is fixed
//! inside egui and keeps running.

use egui::{Context, Id, Style, emath::easing, style::ScrollAnimation};
use inboxcraft_engine::appearance::{MotionLevel, effective};

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

/// The `medium` token, in seconds: a row decoding in, a snoozed row sliding out, an answered card leaving.
pub const MEDIUM: f64 = 0.3;

/// The `undo` token, in seconds: how long an act or a discard can be taken back from the status line.
pub const UNDO: f64 = 5.0;

/// The `notice` token, in seconds: how long a notice shows in the status line.
pub const NOTICE: f64 = 4.0;

/// No animation token is longer than this.
pub const LONGEST: f32 = 0.25;

/// How much the window moves (`docs/design.md`, "Motion"): the owner's level as the window shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Every movement.
    Full,
    /// The calm ones: the character field answers only on Home, and nearer; the emblem as at full.
    Reduced,
    /// None: still dots and a still emblem.
    Off,
}

impl Level {
    /// The window's level for the engine's kept one.
    #[must_use]
    pub const fn of(level: MotionLevel) -> Self {
        match level {
            MotionLevel::Full => Self::Full,
            MotionLevel::Reduced => Self::Reduced,
            MotionLevel::Off => Self::Off,
        }
    }

    /// The engine's level for this one, as `ui.motion.set level=` names it.
    #[must_use]
    pub const fn kept(self) -> MotionLevel {
        match self {
            Self::Full => MotionLevel::Full,
            Self::Reduced => MotionLevel::Reduced,
            Self::Off => MotionLevel::Off,
        }
    }

    /// Whether every movement lands at its end at once: at Reduced and at Off, until each level has movements of its
    /// own; Full runs them.
    #[must_use]
    pub const fn still(self) -> bool {
        !matches!(self, Self::Full)
    }
}

/// Where the level lives in the context.
fn flag() -> Id {
    Id::unique("inboxcraft-motion-level")
}

/// Where the computer's wish for less motion lives in the context.
fn wish() -> Id {
    Id::unique("inboxcraft-system-reduces-motion")
}

/// The computer's wish for less motion as a default: the wish where it can be read (`Some`, the web build's
/// `prefers-reduced-motion`), otherwise none.
#[must_use]
pub fn system_default(system: Option<bool>) -> bool {
    system.unwrap_or(false)
}

/// Tells `ctx` the computer's wish for less motion, where the front end can read it (`None`: it cannot). [`follow`]
/// reads it while the owner has not chosen a level.
pub fn set_system_wish(ctx: &Context, reduces: Option<bool>) {
    ctx.data_mut(|data| data.insert_temp(wish(), reduces));
}

/// The computer's wish for less motion as `ctx` was told it ([`set_system_wish`]); none when it was not.
#[must_use]
pub fn system_wish(ctx: &Context) -> bool {
    system_default(ctx.data(|data| data.get_temp::<Option<bool>>(wish())).flatten())
}

/// Sets the level in `ctx` from the owner's kept `level`, `chosen` or not ([`effective`]: not chosen, the computer's
/// wish for less motion gives Off), and gives back the level set.
pub fn follow(ctx: &Context, level: MotionLevel, chosen: bool) -> Level {
    let shown = Level::of(effective(level, chosen, system_wish(ctx)));
    set_level(ctx, shown);
    shown
}

/// Sets the level in `ctx`, egui's fades and scrolling to a place included, from this frame.
pub fn set_level(ctx: &Context, level: Level) {
    ctx.data_mut(|data| data.insert_temp(flag(), level));
    let still = level.still();
    let time = if still { 0.0 } else { MOTION.tooltip };
    let scroll = if still { ScrollAnimation::none() } else { ScrollAnimation::default() };
    let style = ctx.global_style();
    if style.animation_time != time || style.scroll_animation != scroll {
        ctx.all_styles_mut(|style| {
            style.animation_time = time;
            style.scroll_animation = scroll;
        });
    }
}

/// [`set_level`] for a board or a test that knows only on or off: `reduced` is [`Level::Off`], else [`Level::Full`].
pub fn set_reduced(ctx: &Context, reduced: bool) {
    set_level(ctx, if reduced { Level::Off } else { Level::Full });
}

/// The level in `ctx`; [`Level::Full`] where none was set (a board drawn without the window).
#[must_use]
pub(crate) fn level(ctx: &Context) -> Level {
    ctx.data(|data| data.get_temp(flag())).unwrap_or(Level::Full)
}

/// Whether every movement lands at its end at once in `ctx` ([`Level::still`]).
#[must_use]
pub fn reduced(ctx: &Context) -> bool {
    level(ctx).still()
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

/// The state of a movement [`run`] plays: when it started, the pass it was last asked in, the pass it started in, and
/// whether it is over.
#[derive(Debug, Clone, Copy)]
struct Run {
    started: f64,
    last: u64,
    first: u64,
    over: bool,
}

/// The seconds a movement `id` of `seconds` has run, from the first frame it was asked in, at whatever level (the
/// caller picks the movement of its level); `None` once it is over: its time is up, or a key or a click came in a frame
/// after its first (the owner never waits: the movement jumps to its end, and the key acts as it would at rest). A
/// movement that missed a frame starts again. A repaint is asked for while it runs.
pub(crate) fn run(ctx: &Context, id: Id, seconds: f64) -> Option<f64> {
    let now = ctx.input(|input| input.time);
    let pass = ctx.cumulative_pass_nr();
    let key = id.with("run");
    let was: Option<Run> = ctx.data(|data| data.get_temp(key)).filter(|was: &Run| was.last.saturating_add(1) >= pass);
    let mut state = was.unwrap_or(Run { started: now, last: pass, first: pass, over: false });
    state.last = pass;
    // A key ends it in this pass; a click in the next frame, once the frame that reads it is drawn.
    let mut ending = false;
    match interrupted(ctx).filter(|_| pass != state.first && !state.over) {
        Some(Came::Key) => state.over = true,
        Some(Came::Click) => ending = true,
        None => {}
    }
    let age = now - state.started;
    if !age.is_finite() || age < 0.0 || !seconds.is_finite() || age >= seconds {
        state.over = true;
    }
    if ending && !state.over {
        // Drawn as it stands in the frame of the click, so the click lands on what the owner pressed; gone in the next.
        state.over = true;
        ctx.data_mut(|data| data.insert_temp(key, state));
        ctx.request_repaint();
        return Some(age);
    }
    ctx.data_mut(|data| data.insert_temp(key, state));
    if state.over {
        return None;
    }
    ctx.request_repaint();
    Some(age)
}

/// Where the pass is kept in which a key or a click came, and which, as the frame's start saw it ([`note_input`]).
fn input_id() -> Id {
    Id::unique("inboxcraft-motion-input")
}

/// What came in a pass that ends a movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Came {
    /// A key pressed or text typed.
    Key,
    /// The pointer let go: a click lands on what was under the pointer when it was pressed, so the press itself moves
    /// nothing under it, and what lies under the pointer stays put for the frame that reads the click.
    Click,
}

/// Notes, at the start of a frame before any key is taken, whether a key or a click came in it: a movement [`run`]
/// asks later in the frame, after the keys were taken, still jumps to its end.
pub(crate) fn note_input(ctx: &Context) {
    if let Some(came) = input_came(ctx) {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| data.insert_temp(input_id(), (pass, came)));
    }
}

/// What came in this pass as its input reads now.
fn input_came(ctx: &Context) -> Option<Came> {
    ctx.input(|input| {
        if input.events.iter().any(|event| matches!(event, egui::Event::Key { pressed: true, .. } | egui::Event::Text(_))) {
            Some(Came::Key)
        } else {
            input.pointer.any_released().then_some(Came::Click)
        }
    })
}

/// Whether a key or a click came in this pass, taken by now or not: a movement that leaves the layout as it is (a new
/// row coming in) ends in it.
pub(crate) fn input_now(ctx: &Context) -> bool {
    interrupted(ctx).is_some()
}

/// What came in this pass, taken by now or not.
fn interrupted(ctx: &Context) -> Option<Came> {
    let pass = ctx.cumulative_pass_nr();
    let noted = ctx.data(|data| data.get_temp::<(u64, Came)>(input_id())).filter(|(at, _)| *at == pass).map(|(_, came)| came);
    match (input_came(ctx), noted) {
        (Some(Came::Key), _) | (_, Some(Came::Key)) => Some(Came::Key),
        (came, noted) => came.or(noted),
    }
}

/// The share of `span` seconds gone at `age` seconds, from `from`: 0 before it, 1 after it (or for an empty span).
#[must_use]
pub fn part(age: f64, from: f64, span: f64) -> f32 {
    if span > 0.0 && span.is_finite() {
        // A share of a fraction of a second: the cast loses nothing that shows.
        (((age - from) / span) as f32).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// The design's one curve, `cubic-bezier(.2, .7, .2, 1)`: how far a movement has come at `share` of its time (0 to
/// 1; below 0 counts as 0, above 1 or not a number as 1, the end of the movement). The curve's time is found by
/// halving its interval a fixed number of times (its x rises from 0 to 1), so it always ends.
#[must_use]
pub fn ease(share: f32) -> f32 {
    if share.is_nan() {
        return 1.0;
    }
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
    let share = if share.is_nan() { 1.0 } else { f64::from(share.clamp(0.0, 1.0)) };
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

/// One turn of the caret's blink, in seconds (the `caret` token, 1 000 ms): on for the first half, off for the second.
pub const CARET: f64 = 1.0;

/// From one line of an unfolding list to the next, in seconds (the `stagger` token, 30 ms).
pub const STAGGER: f32 = 0.03;

/// The longest wait [`blink`] asks a repaint after, in milliseconds: half a turn of [`CARET`].
const HALF_TURN_MS: f64 = 500.0;

/// Whether a caret that last moved at `since` (an input time, in seconds) shows now: on for the first half of each
/// turn of [`CARET`], off for the second, at full and at reduced, as the design's caret blinks. A repaint is asked for
/// at its next change, never sooner. At the off level, or `still` (a caret drawn on a board), it is on and asks for
/// nothing.
pub fn blink(ctx: &Context, since: f64, still: bool) -> bool {
    if still || level(ctx) == Level::Off {
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
