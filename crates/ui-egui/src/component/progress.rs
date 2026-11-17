// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The progress line of the status line (a sync, a send): a mark, a word, a bar of ten characters and a note, in mono
//! 11, 8 apart, 24 high. The bar is characters, not a drawing: `[=====>    ]`, a cell for each tenth of the work done.
//!
//! - Running: the mark is a ring of eight 2 px `warn` dots in a 10 px box; the word and the bar `ink`, the note `muted`.
//!   One dot of the ring is left out, and the gap goes round clockwise a dot every 125 ms: the line asks for a frame
//!   125 ms on (never at once) and for nothing else. The bar fills as the caller says the work is done (motion M9:
//!   the same at full and reduced motion, since it is information).
//! - Done: a `plus` dot, the word `ink`, a full `muted` bar, the note `muted`.
//! - Failed: a `minus` dot, the word and the bar `minus`, the bar broken with `×` where the work stopped, and the note
//!   is the small link to the log.
//!
//! A fraction from outside is clamped to 0 to 1 (not a number counts as 0).

use std::time::Duration;

use egui::{Rect, Response, Role, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::button::{LinkSize, link_sized};

/// How the work stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressState {
    /// Under way, this share of it done (0 to 1).
    Running(f32),
    /// Finished.
    Done,
    /// Stopped by a failure when this share of it was done (0 to 1).
    Failed(f32),
}

/// The cells of the bar.
const CELLS: usize = 10;

/// The room between the line's parts, in points.
const GAP: f32 = 8.0;

/// The side of the running mark's box, in points: a fixed mark, as large at every text scale.
const RING: f32 = 10.0;

/// The width of the dot of a finished line, in points: the sheet's `●` is 11 wide (its glyph comes from another face
/// there); Plex Mono's is narrower and stands in the middle of that room.
const DOT_MARK: f32 = 11.0;

/// The side of one dot of the running mark, in points.
const DOT: f32 = 2.0;

/// The dots of the running mark in its box, clockwise from the top.
const DOTS: [(f32, f32); 8] = [(4.0, 0.0), (7.0, 1.0), (8.0, 4.0), (7.0, 7.0), (4.0, 8.0), (1.0, 7.0), (0.0, 4.0), (1.0, 1.0)];

/// How long the running mark's gap stays on one dot.
const STEP: Duration = Duration::from_millis(125);

/// The cells done of `fraction`: 0 to 10, a fraction that is not a number counting as none.
fn done_cells(fraction: f32) -> usize {
    if fraction.is_nan() {
        return 0;
    }
    // The bounds are fixed numbers, never negative or NaN, and the value is a number here: `clamp` cannot panic.
    let share = fraction.clamp(0.0, 1.0);
    // 0 to 10 after the bounds above.
    (share * CELLS as f32).floor() as usize
}

/// The bar of `state`: ten cells in brackets.
pub(crate) fn bar(state: ProgressState) -> String {
    let (done, head) = match state {
        ProgressState::Done => (CELLS, None),
        ProgressState::Running(fraction) => match done_cells(fraction) {
            CELLS => (CELLS, None),
            done => (done, Some('>')),
        },
        // The break stands in the last cell at the most.
        ProgressState::Failed(fraction) => (done_cells(fraction).min(CELLS - 1), Some('×')),
    };
    let mut text = String::with_capacity(CELLS + 4);
    text.push('[');
    text.extend(std::iter::repeat_n('=', done));
    if let Some(head) = head {
        text.push(head);
        text.extend(std::iter::repeat_n(' ', CELLS - done - 1));
    }
    text.push(']');
    text
}

/// A progress line across what it needs of `ui`: `word`, the bar of `state` and `note` (in [`ProgressState::Failed`]
/// the link to the log, whose response this is).
pub fn progress(ui: &mut Ui, theme: &Theme, keymap: &Keymap, word: &str, state: ProgressState, note: &str) -> Option<Response> {
    progress_with(ui, theme, keymap, word, state, note, false)
}

/// [`progress`]; `still` draws the running mark whole and standing (the sheet's frame, the gallery's board, and the
/// `off` level of motion once the app has one).
pub(crate) fn progress_with(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    word: &str,
    state: ProgressState,
    note: &str,
    still: bool,
) -> Option<Response> {
    let p = &theme.palette;
    let font = theme.fonts.mono_s;
    let (mark, word_ink, bar_ink) = match state {
        ProgressState::Running(_) => (None, p.ink, p.ink),
        ProgressState::Done => (Some(p.plus), p.ink, p.muted),
        ProgressState::Failed(_) => (Some(p.minus), p.minus, p.minus),
    };
    let dot = mark.map(|ink| super::galley(ui, "●", font, ink));
    let words = super::galley(ui, &super::plain(word), font, word_ink);
    let bar = super::galley(ui, &bar(state), font, bar_ink);
    let failed = matches!(state, ProgressState::Failed(_));
    let note_galley = (!failed).then(|| super::galley(ui, &super::plain(note), font, p.muted));
    let link_width = if failed { paint::run_box(super::galley(ui, note, font, p.cyan_text).size().x) } else { 0.0 };
    // Each run in a text box hugging it, as the design lays them out ([`paint::run_box`]).
    let mark_width = dot.as_ref().map_or(RING, |dot| paint::run_box(dot.size().x).max(DOT_MARK));
    let (words_width, bar_width) = (paint::run_box(words.size().x), paint::run_box(bar.size().x));
    let note_width = note_galley.as_ref().map_or(link_width, |note| paint::run_box(note.size().x));
    let width = mark_width + GAP + words_width + GAP + bar_width + GAP + note_width;
    let high = theme.boxes().strip;
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let response = ui.interact(rect, ui.scope_id().with("progress").with(word), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, word));
    let text_top = (rect.center().y - font.line / 2.0).round();
    let mut x = rect.left();
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        match dot {
            Some(dot) => {
                // The dot's run centred in the sheet's 11 point box.
                let at = Rect::from_min_size(pos2(x, text_top), vec2(mark_width, font.line));
                paint::run_centred(painter, at, dot, p.warn);
            }
            None => {
                let ring = pos2(x, (rect.center().y - RING / 2.0).round());
                // The gap goes round while it runs; still, every dot shows.
                let gap = (!still).then(|| {
                    let now = ui.input(|input| input.time);
                    let steps = (now / STEP.as_secs_f64()).floor();
                    // A time that is not a number keeps the gap at the top.
                    if steps.is_finite() { (steps.rem_euclid(DOTS.len() as f64)) as usize } else { 0 }
                });
                for (index, (dx, dy)) in DOTS.iter().enumerate() {
                    if gap != Some(index) {
                        painter.rect_filled(Rect::from_min_size(ring + vec2(*dx, *dy), vec2(DOT, DOT)), 0.0, p.warn);
                    }
                }
                if !still {
                    ui.ctx().request_repaint_after(STEP);
                }
            }
        }
        x += mark_width + GAP;
        painter.galley(pos2(x, text_top), words, word_ink);
        x += words_width + GAP;
        painter.galley(pos2(x, text_top), bar, bar_ink);
        x += bar_width + GAP;
        if let Some(note) = note_galley {
            painter.galley(pos2(x, text_top), note, p.muted);
        }
    } else {
        x += mark_width + GAP + words_width + GAP + bar_width + GAP;
    }
    if !failed {
        return None;
    }
    let at = Rect::from_min_size(pos2(x, text_top), vec2(link_width, font.line));
    let mut child = ui.new_child(UiBuilder::new().max_rect(at).layout(egui::Layout::left_to_right(egui::Align::Center)));
    Some(link_sized(&mut child, theme, keymap, note, LinkSize::Small))
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
