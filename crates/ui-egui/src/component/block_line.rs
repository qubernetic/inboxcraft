// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A line of a block (`block-line`), across its room:
//!
//! - Empty: what the block would hold says why it holds nothing ("No drafts. …"), in `caption` and `muted`, 12 over and
//!   under and 16 beside, wrapped onto at most three lines, the last cut with one `…`. Information: no Tab stop.
//! - More: the small link to the whole list ("all drafts (5) →"), 16 in, on a line 32 high. It is pressable (the
//!   pointer underlines it, the keyboard rings it) and returns the press; it opens nothing itself.

use egui::{Align, Rect, Role, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;

use super::Shown;
use super::button::{LinkSize, link_with};

/// The room beside a line's text or link, in points.
const PAD_X: f32 = 16.0;

/// The room over and under the empty line's text, in points.
const PAD_Y: f32 = 12.0;

/// The most lines the empty line's text takes.
const EMPTY_LINES: usize = 3;

/// The more line's height at 100 % text, in points.
pub const MORE: f32 = 32.0;

/// What a block line shows.
#[derive(Debug, Clone, Copy)]
pub enum BlockLine<'a> {
    /// Why the block holds nothing.
    Empty(&'a str),
    /// The link to the whole list, with its words.
    More(&'a str),
}

/// The empty line's `text` laid out across a line `width` wide: plain, at most [`super::MOST`] characters of it, on at
/// most three lines, the last cut with `…`.
fn empty_galley(ui: &Ui, theme: &Theme, text: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    super::wrapped(ui, &empty_words(text), theme.fonts.caption, theme.palette.muted, (width - PAD_X * 2.0).max(1.0), EMPTY_LINES, Align::LEFT)
}

/// The empty line's `text` as it is laid out and heard, in the written order: plain, at most [`super::MOST`]
/// characters of it, `…` after a cut.
fn empty_words(text: &str) -> std::borrow::Cow<'_, str> {
    let (kept, cut) = super::capped(text, super::MOST);
    let words = super::plain(kept);
    if cut { std::borrow::Cow::Owned(format!("{words}…")) } else { words }
}

/// The height of `line` across `width`, at the theme's text scale: 32 for the more line; for the empty line its text's
/// lines of 18 and 12 over and under (42 for one line, 60 for two).
#[must_use]
pub fn height(ui: &Ui, theme: &Theme, line: &BlockLine<'_>, width: f32) -> f32 {
    match line {
        BlockLine::Empty(text) => {
            let rows = empty_galley(ui, theme, text, width).rows.len().clamp(1, EMPTY_LINES);
            let lines = match rows {
                1 => 18.0,
                2 => 36.0,
                _ => 54.0,
            };
            Theme::size(theme, PAD_Y * 2.0 + lines)
        }
        BlockLine::More(_) => Theme::size(theme, MORE),
    }
}

/// The block line `line` across `ui`'s width at its cursor; `shown` draws the more link hovered or focused whatever the
/// pointer does (the gallery's boards). Whether the more link was pressed.
pub fn block_line(ui: &mut Ui, theme: &Theme, keymap: &Keymap, line: &BlockLine<'_>, shown: Option<Shown>) -> bool {
    let width = ui.available_width().max(PAD_X * 2.0 + 1.0).round();
    let high = height(ui, theme, line, width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    match line {
        BlockLine::Empty(text) => {
            let galley = empty_galley(ui, theme, text, width);
            // Heard in the written order: the galley holds right-to-left words in the order they are drawn.
            let spoken = empty_words(text).into_owned();
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &spoken));
            if ui.is_rect_visible(rect) {
                let top = (rect.center().y - galley.size().y / 2.0).round();
                ui.painter_at(rect).galley(pos2(rect.left() + PAD_X, top), galley, theme.palette.muted);
            }
            false
        }
        BlockLine::More(words) => {
            let link_high = theme.boxes().row_link;
            let place = Rect::from_min_max(
                pos2(rect.left() + PAD_X, (rect.center().y - link_high / 2.0).round()),
                pos2(rect.right() - PAD_X, (rect.center().y + link_high / 2.0).round()),
            );
            // Cut with one `…` to the line's room, so the link neither paints nor takes a press outside it.
            let words = super::fit_capped(ui, &super::plain(super::capped(words, super::MOST).0), theme.fonts.mono_s, place.width());
            if words.is_empty() {
                return false;
            }
            ui.scope_builder(UiBuilder::new().max_rect(place).layout(egui::Layout::left_to_right(Align::Center)), |ui| {
                link_with(ui, theme, keymap, &words, LinkSize::Small, true, shown).clicked()
            })
            .inner
        }
    }
}

#[cfg(test)]
#[path = "block_line_tests.rs"]
mod tests;
