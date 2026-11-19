// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The count tile of Home (`count-tile`): a count over its label, 14 down and 16 in, 2 apart. The count in `title`
//! (mono 22/30) on one line, in its tone's colour (`ink`, `cyan_text`, `warn`, `minus`) and `muted` when it is 0, written
//! as [`super::label::count`] writes one, so it caps at `9 999+`; the label in `label` (mono 11/16, capitals, 8 %
//! tracking) and `muted`, on at most two lines (the sheet's "nothing to read" takes two and the tile grows to 92), cut
//! with one `…` after them. The tile is as wide as its room and paints no ground of its own; under the pointer it
//! takes the `stage` fill and its label `ink`, with the keyboard on it a 1 px `cyan_edge` edge inside its box. It is a
//! button: it returns the press, Enter and Space included, and opens nothing itself.

use egui::{Align, Id, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::theme::Theme;

use super::{Hover, Shown, lit};

/// The room over and under the tile's text, and beside it, in points.
const PAD_Y: f32 = 14.0;
const PAD_X: f32 = 16.0;

/// The room between the count and its label, in points.
const GAP: f32 = 2.0;

/// The most lines the label takes.
const LABEL_LINES: usize = 2;

/// The colour a tile's count is drawn in, as its sheet names the states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// `ink` (`default`).
    Plain,
    /// `cyan_text` (`accent`, `overflow`).
    Accent,
    /// `warn`: what comes back today.
    Warn,
    /// `minus`: what was not sent.
    Minus,
}

/// What a count tile shows.
#[derive(Debug, Clone, Copy)]
pub struct CountTile<'a> {
    /// Its id.
    pub id: Id,
    /// The count; above 9 999 it reads `9 999+`.
    pub count: u64,
    /// What is counted: "To read". Drawn in capitals.
    pub label: &'a str,
    /// The count's colour while it is not 0.
    pub tone: Tone,
    /// The height of the row it stands in, in points (the tallest tile's [`height`]): the tile takes it when it is
    /// taller than its own, so a row of tiles shares one height. `None`: its own.
    pub row: Option<f32>,
}

/// The count as the tile writes it: `0` for none, else as [`super::label::count`] writes it.
#[must_use]
pub fn written(count: u64) -> String {
    if count == 0 { "0".to_owned() } else { super::label::count(count) }
}

/// What a screen reader hears for `tile`: its count and its label.
#[must_use]
pub fn spoken(tile: &CountTile<'_>) -> String {
    format!("{} {}", written(tile.count), super::plain(super::capped(tile.label, super::MOST).0))
}

/// The label's room across a tile `width` wide, in points.
fn room(width: f32) -> f32 {
    (width - PAD_X * 2.0).max(1.0)
}

/// The label of `tile` laid out for a tile `width` wide in `colour`: plain words, at most [`super::MOST`] characters of
/// them, on at most two lines, the last cut with `…`.
fn label_galley(ui: &Ui, theme: &Theme, label: &str, width: f32, colour: egui::Color32) -> std::sync::Arc<egui::Galley> {
    let (kept, cut) = super::capped(label, super::MOST);
    let words = super::plain(kept);
    let words = if cut { std::borrow::Cow::Owned(format!("{words}…")) } else { words };
    super::wrapped(ui, &words, theme.fonts.label, colour, room(width), LABEL_LINES, Align::LEFT)
}

/// The height of a tile `width` wide with `label`, at the theme's text scale: 76 with one line of label, 92 with two.
#[must_use]
pub fn height(ui: &Ui, theme: &Theme, label: &str, width: f32) -> f32 {
    let two = label_galley(ui, theme, label, width, theme.palette.muted).rows.len() > 1;
    let line = if two { 32.0 } else { 16.0 };
    Theme::size(theme, PAD_Y + 30.0 + GAP + line + PAD_Y)
}

/// The tile `tile` across `ui`'s width at its cursor; `shown` draws it hovered or focused whatever the pointer does (the
/// gallery's boards). Its response: clicked when it was pressed.
pub fn count_tile(ui: &mut Ui, theme: &Theme, tile: &CountTile<'_>, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(PAD_X * 2.0 + 1.0).round();
    let own = height(ui, theme, tile.label, width);
    let high = tile.row.filter(|row| row.is_finite()).map_or(own, |row| row.max(own)).round();
    let (rect, _) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    let response = ui.interact(rect, tile.id, Sense::click());
    let name = spoken(tile);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
    let lit = lit(ui, theme, &response, true, shown, Hover::AtOnce);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter_at(rect);
    if lit.hover || lit.pressed {
        painter.rect_filled(rect, 0.0, p.stage);
    }
    let ink = match (tile.count, tile.tone) {
        (0, _) => p.muted,
        (_, Tone::Plain) => p.ink,
        (_, Tone::Accent) => p.cyan_text,
        (_, Tone::Warn) => p.warn,
        (_, Tone::Minus) => p.minus,
    };
    let count = super::cut(ui, &written(tile.count), f.title, ink, room(width));
    let left = rect.left() + PAD_X;
    let top = rect.top() + PAD_Y;
    paint::run_at(&painter, pos2(left, top), count, ink);
    let words = if lit.hover || lit.pressed { p.ink } else { p.muted };
    let label = label_galley(ui, theme, tile.label, width, words);
    let label_top = (top + f.title.line + GAP).round();
    painter.galley(pos2(left, label_top), label, words);
    if lit.focus {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, p.cyan_edge), StrokeKind::Inside);
    }
    response
}

#[cfg(test)]
#[path = "count_tile_tests.rs"]
mod tests;
