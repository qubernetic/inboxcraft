// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A place of the index (`nav-row`): 232 × 34 on the `panel`, its number (`[ 01 ]`) in `label` (`dim`, `cyan_text`
//! while the place is shown or open), its name in `label` (`muted`; `ink` hovered, shown or open) 52 right of it and,
//! when there is one, its count at the right in `mono_s` (`muted`; `cyan_text` shown or open), written by
//! [`super::label::count`] (`9 999+` at most). The place shown takes the `stage` fill and the 2 px `cyan_edge` bar;
//! hovered, the row takes `grid`; with the keyboard on it, a 1 px `cyan_edge` edge just inside it. Narrow (56 wide) it
//! shows the place's icon, 16 px, in the middle (`muted`; `ink` shown or open).
//!
//! The rows of the index share [`base`]: the fill, the bar, the hover's fade and the focus edge, so the mailbox row and
//! the account row stand exactly as a place does.

use egui::{Color32, Id, Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::iconset::{self, IconId};
use crate::motion;
use crate::theme::Theme;

use super::Shown;

/// The width of the shown row's bar at its left edge, in points.
pub(crate) const BAR: f32 = 2.0;

/// A row's right padding, in points: the count's right edge stands this far in.
pub(crate) const RIGHT: f32 = 14.0;

/// The side of a row's icon, in points.
pub(crate) const ICON: f32 = 16.0;

/// A place's number's left edge, from the row's left, in points.
const INDEX_LEFT: f32 = 16.0;

/// A place's name's left edge, from the row's left, in points.
const LABEL_LEFT: f32 = 68.0;

/// The room between a name and the count after it, in points.
const COUNT_GAP: f32 = 8.0;

/// How high a place's row is at 100 % text, in points.
pub const HEIGHT: f32 = 34.0;

/// How a row of the index stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Neither shown nor open.
    Rest,
    /// The place or mailbox shown: the `stage` fill and the bar.
    Active,
    /// Open, without being the one shown (MAIL while a mailbox of it is shown): its number lit, no fill.
    Open,
}

/// A row of the index laid down by [`base`]: where it is, its response, whether the pointer is over it and whether the
/// keyboard is on it.
#[derive(Debug, Clone)]
pub(crate) struct Base {
    pub rect: Rect,
    pub response: Response,
    pub hovered: bool,
    pub focused: bool,
}

/// Lays down a row of the index `id` across `ui`'s width, `height` points high at 100 % text, `active` (the `stage`
/// fill and the bar) or not; `pressable` false senses hover only (a row coming soon). Paints its fill and bar; its
/// focus edge comes with [`Base::finish`], after the caller's content. A row out of sight paints nothing.
#[must_use = "draw the row's content, then call `Base::finish`: it paints the focus edge"]
pub(crate) fn base(ui: &mut Ui, theme: &Theme, id: Id, height: f32, active: bool, pressable: bool, shown: Option<Shown>) -> Base {
    let p = &theme.palette;
    let high = Theme::size(theme, height);
    let (_, rect) = ui.allocate_space(vec2(ui.available_width().max(0.0), high));
    let sense = if pressable { super::soon::sense(ui, Sense::click()) } else { Sense::hover() };
    let response = ui.interact(rect, id, sense);
    // Rows touch: a row owns its top edge and not its bottom one.
    let below_top = || ui.ctx().pointer_latest_pos().is_some_and(|at| at.y < rect.bottom());
    let hovered = shown.map_or_else(|| response.contains_pointer() && below_top(), |shown| shown == Shown::Hover);
    let focused = shown.map_or_else(|| response.has_focus(), |shown| shown == Shown::Focus);
    if !ui.is_rect_visible(rect) {
        return Base { rect, response, hovered, focused };
    }
    let lit = hovered && sense.senses_click();
    let fade = if shown.is_some() { f32::from(u8::from(lit)) } else { motion::fade(ui.ctx(), id.with("hover"), lit, theme.motion.fast) };
    let painter = ui.painter();
    if active {
        // The ground and the bar on whole pixels at fractional scales.
        painter.rect_filled(paint::on_pixels(painter, rect), 0.0, p.stage);
        painter.rect_filled(paint::on_pixels(painter, Rect::from_min_size(rect.min, vec2(BAR, rect.height()))), 0.0, p.cyan_edge);
    } else if fade >= 1.0 {
        painter.rect_filled(paint::on_pixels(painter, rect), 0.0, p.grid);
    } else if fade > 0.0 {
        // Between the two token colours, never the hover colour faded by alpha.
        painter.rect_filled(paint::on_pixels(painter, rect), 0.0, p.panel.lerp_to_gamma(p.grid, fade));
    }
    Base { rect, response, hovered: lit, focused }
}

impl Base {
    /// Paints the row's focus edge when the keyboard is on it: 1 px of `cyan_edge` just inside the row, on whole
    /// pixels, over what the caller drew.
    pub(crate) fn finish(&self, ui: &Ui, theme: &Theme) {
        if self.focused && ui.is_rect_visible(self.rect) && self.rect.width() >= 2.0 && self.rect.height() >= 2.0 {
            // A ring 2 inside the rect grown by 2: on the row's own edge.
            paint::focus_ring(ui.painter(), theme, self.rect.expand(2.0), Ring::Inside);
        }
    }

    /// The top of a run `line` high in the middle of the row, on a whole point.
    pub(crate) fn middle(&self, line: f32) -> f32 {
        self.rect.top() + ((self.rect.height() - line) / 2.0).round()
    }
}

/// Paints `icon` 16 px in `colour` with its box's left edge at `left`, in the middle of `row`.
pub(crate) fn paint_icon(ui: &Ui, row: &Base, glyph: char, left: f32, colour: Color32) {
    let at = Rect::from_min_size(pos2(left, row.middle(ICON)), vec2(ICON, ICON));
    iconset::paint_glyph(ui.painter(), at, glyph, colour);
}

/// What a place's row shows.
#[derive(Debug, Clone, Copy)]
pub struct NavRow<'a> {
    /// Its id: its focus and its response.
    pub id: Id,
    /// What a screen reader hears for it.
    pub spoken: &'a str,
    /// Its number, `[ 01 ]`.
    pub index: &'a str,
    /// Its name; set in capitals.
    pub label: &'a str,
    /// Its icon, shown when narrow.
    pub icon: IconId,
    /// How many wait in it; 0 shows none.
    pub count: u64,
    /// Shown, open or neither.
    pub standing: Standing,
}

/// The row `row` across `ui`'s width, 56 wide when `narrow`; `shown` draws it hovered or focused whatever the pointer
/// does (the gallery's board). Inside [`super::soon::soon`] it takes no press and no focus.
pub fn nav_row(ui: &mut Ui, theme: &Theme, row: &NavRow<'_>, narrow: bool, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let drawn = base(ui, theme, row.id, HEIGHT, row.standing == Standing::Active, true, shown);
    let enabled = !super::soon::is_soon(ui);
    let spoken = row.spoken;
    let active = row.standing == Standing::Active;
    drawn.response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, active, spoken));
    if !ui.is_rect_visible(drawn.rect) {
        return drawn.response;
    }
    let lit = row.standing != Standing::Rest;
    let rect = drawn.rect;
    if narrow {
        let ink = if lit { p.ink } else { p.muted };
        paint_icon(ui, &drawn, iconset::glyph(row.icon), rect.center().x - ICON / 2.0, super::soon::ink(ui, theme, ink));
        drawn.finish(ui, theme);
        return drawn.response;
    }
    let top = drawn.middle(f.label.line);
    let number = if lit { p.cyan_text } else { p.dim };
    let numbered = super::galley(ui, row.index, f.label, number);
    // At a larger text the number grows past the label's place: the label keeps the frame's gap after it.
    let after = rect.left() + INDEX_LEFT + numbered.size().x + COUNT_GAP;
    paint::run_at(ui.painter(), pos2(rect.left() + INDEX_LEFT, top), numbered, number);
    let right = rect.right() - RIGHT;
    let count = super::label::count(row.count);
    let end = if count.is_empty() {
        right
    } else {
        let ink = if lit { p.cyan_text } else { p.muted };
        let galley = super::galley(ui, &count, f.mono_s, ink);
        paint::run_right(ui.painter(), right, drawn.middle(f.mono_s.line), galley, ink) - COUNT_GAP
    };
    let ink = if lit || drawn.hovered { p.ink } else { p.muted };
    let ink = super::soon::ink(ui, theme, ink);
    let left = (rect.left() + LABEL_LEFT).max(after.ceil());
    let label = super::fitted(ui, row.label, f.label, ink, (end - left).max(0.0));
    paint::run_at(ui.painter(), pos2(left, top), label, ink);
    drawn.finish(ui, theme);
    drawn.response
}

#[cfg(test)]
#[path = "nav_row_tests.rs"]
pub(crate) mod tests;
