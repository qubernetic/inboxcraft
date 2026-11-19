// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The key hint of the status line: a key chip ([`super::key`]) and 8 after it the word of its action in `mono_s`,
//! `muted` (read, never dim), as high as the chip. Hovered, the word is `ink` on a `grid` fill that stands 4 out at
//! the sides and 2 above and below; a click runs the action. It is information: the keyboard runs the action by its
//! key, so the hint takes no Tab stop. A word longer than the room the hint has is cut with one `…`.

use egui::{Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};

use super::Shown;
use super::key::{Chip, ChipState};
use crate::theme::Theme;

/// How far the hover fill stands out at the left and right, and above and below, in points (the sheet's `hover`:
/// its fill at −4, −2, 60 × 24 around a hint 52 × 20).
const HOVER_OUT: (f32, f32) = (4.0, 2.0);

/// What a screen reader hears of the hint of `key` and `word`.
#[must_use]
pub fn spoken(key: &str, word: &str) -> String {
    format!("{key} {word}")
}

/// The hint of `key` (already in words, [`super::key::key_words`]) and `word`, drawn in `forced` whatever the pointer
/// does (a board), else hovered while the pointer is over it. Its response and whether it was clicked. Its width hugs
/// the chip and the word, at most the room `ui` has; the word is cut to fit.
pub fn key_hint(ui: &mut Ui, theme: &Theme, key: &str, word: &str, forced: Option<Shown>) -> (Response, bool) {
    let p = &theme.palette;
    let font = theme.fonts.mono_s;
    // The sheet's 8 between the chip and the word.
    let gap = theme.spacing.s2;
    let chip = Chip::new(ui, theme, key, ChipState::Default);
    // At most a line's worth of characters is laid out, as plain words: a control or bidi character the caller left
    // in is a space or left out, never layout.
    let (word_kept, _) = super::capped(word, super::MOST);
    let word_kept = super::plain(word_kept);
    let word_kept = word_kept.as_ref();
    let room = if ui.available_width().is_finite() { ui.available_width() } else { f32::INFINITY };
    let word_room = (room - chip.size().x - gap).max(0.0);
    let whole = super::galley(ui, word_kept, font, p.muted);
    let wide = crate::draw::paint::run_box(whole.size().x);
    let size = vec2(chip.size().x + gap + wide.min(word_room), chip.size().y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let spoken = spoken(key, word_kept);
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), &spoken));
    let hover = match forced {
        Some(shown) => shown == Shown::Hover,
        None => ui.is_enabled() && response.hovered(),
    };
    let clicked = forced.is_none() && hover && ui.input(|input| input.pointer.primary_clicked());
    if ui.is_rect_visible(rect) {
        let painter = super::solid(ui);
        if hover {
            painter.rect_filled(rect.expand2(vec2(HOVER_OUT.0, HOVER_OUT.1)), 0.0, p.grid);
        }
        let at = rect.min;
        let chip_size = chip.size();
        chip.paint(&painter, at);
        let ink = if hover { p.ink } else { p.muted };
        let left = at.x + chip_size.x + gap;
        let text = if wide <= word_room { super::galley(ui, word_kept, font, ink) } else { super::cut(ui, word_kept, font, ink, word_room) };
        let top = (rect.center().y - font.line / 2.0).round();
        let clip = Rect::from_min_max(pos2(left, rect.top()), pos2(rect.right(), rect.bottom()));
        painter.with_clip_rect(clip.intersect(painter.clip_rect())).galley(pos2(left, top), text, ink);
    }
    (response, clicked)
}

#[cfg(test)]
#[path = "key_hint_tests.rs"]
mod tests;
