// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scramble (`docs/design.md`, "Motion", M3 and M12): a text resolving from the glyphs of the code set
//! ([`crate::field::CODE`]) from its left, or going to them from its right. Each character changes once, so nothing
//! flashes: the glyph that stands for a character is the same in every frame ([`glyph`]). White space and line breaks
//! stay as they are.
//!
//! M3 at full: a button's label resolves on hover, [`SCRAMBLE_CHAR`] a character and at most [`MEDIUM`] in all
//! ([`hover`]); a page's headline resolves over [`SLOW`] each time the page opens ([`headline`]). At reduced only the
//! headline, once per launch; at off nothing. A key or a press during a scramble ends it at once ([`interrupted`]).

use std::ops::Range;
use std::sync::Arc;

use egui::text::{ByteIndex, LayoutJob};
use egui::{Context, Galley, Id, Painter, Shape};

use crate::motion::{self, Level};

/// The `scramble_char` token, in seconds: one character of a hover scramble.
pub const SCRAMBLE_CHAR: f32 = 0.07;

/// The `medium` token, in seconds: the longest a hover scramble runs, and a place's new text resolving (M12).
pub const MEDIUM: f32 = 0.3;

/// The `slow` token, in seconds: a headline resolving on load.
pub const SLOW: f32 = 0.7;

/// The longest text, in bytes, that scrambles: a longer one stays as it is (a shape's text is cut to its box long
/// before; this bounds the work of one frame whatever a message holds).
pub const MOST: usize = 4096;

/// The glyph that stands for character `index` of a text while it scrambles under `seed`: the same in every frame.
#[must_use]
pub fn glyph(seed: u64, index: usize) -> char {
    let set = crate::field::CODE;
    // A usize fits in a u64 on every target the app builds for.
    let mut x = seed ^ (index as u64).wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    // The remainder is below the set's length, so the cast keeps it.
    let at = (x % (set.len().max(1) as u64)) as usize;
    set.get(at).map_or('#', |byte| char::from(*byte))
}

/// The character at `index` of a scrambling text: `ch` itself inside `real` or when it is white space, else its glyph.
fn one(ch: char, index: usize, real: &Range<usize>, seed: u64) -> char {
    if ch.is_whitespace() || real.contains(&index) { ch } else { glyph(seed, index) }
}

/// `text` with the characters inside `real` (counted in characters) as they are and every other one a glyph.
#[must_use]
pub fn mixed(text: &str, real: Range<usize>, seed: u64) -> String {
    text.chars().enumerate().map(|(index, ch)| one(ch, index, &real, seed)).collect()
}

/// `job` with the characters inside `real` as they are and every other one a glyph, its sections kept on the same
/// characters; none when its text is longer than [`MOST`].
#[must_use]
pub fn job(job: &LayoutJob, real: &Range<usize>, seed: u64) -> Option<LayoutJob> {
    if job.text.len() > MOST {
        return None;
    }
    let mut out = job.clone();
    let mut text = String::with_capacity(job.text.len());
    let mut index = 0_usize;
    for section in &mut out.sections {
        let part = job.text.get(section.byte_range.start.0..section.byte_range.end.0).unwrap_or_default();
        let start = text.len();
        for ch in part.chars() {
            text.push(one(ch, index, real, seed));
            index = index.saturating_add(1);
        }
        section.byte_range = ByteIndex(start)..ByteIndex(text.len());
    }
    out.text = text;
    Some(out)
}

/// The characters of `galley`'s text; none past [`MOST`] bytes.
#[must_use]
pub fn count(galley: &Galley) -> Option<usize> {
    (galley.job.text.len() <= MOST).then(|| galley.job.text.chars().count())
}

/// `galley` laid out again through `painter` with the characters inside `real` as they are and the others glyphs; the
/// galley itself when it is too long to scramble.
#[must_use]
pub fn galley(painter: &Painter, galley: &Arc<Galley>, real: &Range<usize>, seed: u64) -> Arc<Galley> {
    job(&galley.job, real, seed).map_or_else(|| Arc::clone(galley), |job| painter.layout_job(job))
}

/// Which end of a text a scramble starts at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Resolving in: the characters read from the left as `share` grows.
    Left,
    /// Going out: the characters turn to glyphs from the right as `share` grows.
    Right,
}

/// The characters of a text of `count` that read at `share` (0 to 1) of a scramble from `from`.
#[must_use]
pub fn real(count: usize, share: f32, from: Side) -> Range<usize> {
    let share = if share.is_nan() { 1.0 } else { share.clamp(0.0, 1.0) };
    // A share of a count of characters on screen: the casts lose nothing that shows.
    let done = ((count as f32) * share).floor() as usize;
    match from {
        Side::Left => 0..done.min(count),
        Side::Right => 0..count.saturating_sub(done),
    }
}

/// Every text in `shape` scrambled through `painter` at `share` from `from`; a shape with no text stays.
pub fn shape(painter: &Painter, shape: &mut Shape, share: f32, from: Side) {
    match shape {
        Shape::Text(text) => {
            let Some(count) = count(&text.galley) else { return };
            let seed = count as u64;
            text.galley = galley(painter, &text.galley, &real(count, share, from), seed);
        }
        Shape::Vec(shapes) => {
            for inner in shapes {
                self::shape(painter, inner, share, from);
            }
        }
        _ => {}
    }
}

/// Whether a key or a press came in this frame: a movement that runs jumps to its end.
#[must_use]
pub fn interrupted(ctx: &Context) -> bool {
    ctx.input(|input| {
        input.pointer.any_pressed() || input.events.iter().any(|event| matches!(event, egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)))
    })
}

/// The state of a hover scramble: when it started, and whether the pointer was on the control in the last frame.
#[derive(Debug, Clone, Copy, Default)]
struct Hover {
    started: Option<f64>,
    on: bool,
}

/// How many characters of a label of `count` read while the pointer is on it (`hovered`), from the left; none when the
/// whole label reads. At full a scramble starts in the frame the pointer comes on and runs [`SCRAMBLE_CHAR`] a
/// character, [`MEDIUM`] at most; it does not run again until the pointer has left. A repaint is asked for while it runs.
pub fn hover(ctx: &Context, id: Id, hovered: bool, count: usize) -> Option<usize> {
    let key = id.with("scramble");
    let was: Hover = ctx.data(|data| data.get_temp(key)).unwrap_or_default();
    let now = ctx.input(|input| input.time);
    let mut started = if motion::level(ctx) == Level::Full && !interrupted(ctx) { was.started } else { None };
    if hovered && !was.on && motion::level(ctx) == Level::Full && count > 0 {
        started = Some(now);
    }
    let shown = started.and_then(|start| {
        // At most [`MEDIUM`] in all: a long label takes less than a scramble character each.
        let per = f64::from(SCRAMBLE_CHAR.min(MEDIUM / count.max(1) as f32));
        let age = (now - start).max(0.0);
        // A few hundred milliseconds over a few milliseconds: the cast loses nothing.
        let done = if per > 0.0 && age.is_finite() { (age / per + 1e-6).floor() as usize } else { count };
        (done < count).then_some(done)
    });
    if shown.is_some() {
        ctx.request_repaint();
    }
    ctx.data_mut(|data| data.insert_temp(key, Hover { started: shown.and(started), on: hovered }));
    shown
}

/// The state of a headline's scramble: when it started, the pass it was last drawn in, and whether it is over.
#[derive(Debug, Clone, Copy)]
struct Headline {
    started: f64,
    last: u64,
    over: bool,
}

/// Where the launch remembers that a headline has resolved once (reduced plays it once per launch).
fn played() -> Id {
    Id::unique("inboxcraft-headline-played")
}

/// The share of a headline `id` that reads, 0 to 1, from the left, resolving over [`SLOW`]: at full each time the page
/// is opened (a frame it was not drawn in starts it again), at reduced once per launch, at off never (1). A key or a
/// press ends it. A repaint is asked for while it runs.
pub fn headline(ctx: &Context, id: Id) -> f32 {
    let key = id.with("headline");
    let level = motion::level(ctx);
    let now = ctx.input(|input| input.time);
    let pass = ctx.cumulative_pass_nr();
    let was: Option<Headline> = ctx.data(|data| data.get_temp(key));
    let shown_before = was.filter(|was| was.last.saturating_add(1) >= pass);
    let mut state = match shown_before {
        Some(state) => state,
        None => {
            let once = level == Level::Reduced && ctx.data(|data| data.get_temp::<bool>(played())).unwrap_or(false);
            Headline { started: now, last: pass, over: level == Level::Off || once }
        }
    };
    if shown_before.is_none() && !state.over {
        ctx.data_mut(|data| data.insert_temp(played(), true));
    }
    state.last = pass;
    if interrupted(ctx) || level == Level::Off {
        state.over = true;
    }
    // A share of a fraction of a second: the cast loses nothing that shows.
    let share = if state.over { 1.0 } else { (((now - state.started) / f64::from(SLOW)) as f32).clamp(0.0, 1.0) };
    if share >= 1.0 || share.is_nan() {
        state.over = true;
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|data| data.insert_temp(key, state));
    if state.over { 1.0 } else { share }
}

#[cfg(test)]
#[path = "scramble_tests.rs"]
mod tests;
