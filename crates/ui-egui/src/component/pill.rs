// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pill: a filter that is on or off, 24 high, its word in mono 11, 8 beside it, square corners, cut with an
//! ellipsis at 240. Off: a 1 px `line` edge and a `muted` word; hovered, the `stage` fill, a `muted` edge and an `ink`
//! word. Selected: the accent wash, the `cyan_edge` edge and a `cyan_text` word. Pressed: as a pressed secondary button
//! (the `stage` fill, an `ink` edge and word), or, selected, the accent wash with the `cyan_edge` edge as every chosen
//! control pressed, its word turning `ink`. Focus is the ring
//! 2 px clear; disabled a dashed `line` edge with the word `muted`. Nothing moves (the design gives it no transition).

use egui::{Color32, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::Theme;

use super::{Hover, Shown, cut, lit, solid, soon};

/// The room left and right of the word, in points.
const PAD: f32 = 8.0;

/// The widest the word gets before it ends with an ellipsis, in points.
const WIDEST: f32 = 240.0;

/// A pill with `text` in it, selected or not; its response is clicked when it was pressed.
pub fn pill(ui: &mut Ui, theme: &Theme, text: &str, selected: bool) -> Response {
    pill_with(ui, theme, text, selected, None)
}

/// [`pill`] drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
pub(crate) fn pill_with(ui: &mut Ui, theme: &Theme, text: &str, selected: bool, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let enabled = ui.is_enabled() && !soon::is_soon(ui);
    let font = theme.fonts.mono_s;
    let word = cut(ui, text, font, Color32::PLACEHOLDER, WIDEST);
    // A cut word takes all its room.
    let cut_short = super::galley(ui, text, font, Color32::PLACEHOLDER).size().x > WIDEST;
    let wide = if cut_short { WIDEST } else { word.size().x.ceil() };
    let size = vec2(wide + PAD * 2.0, theme.boxes().pill);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, selected, text));
    let lit = lit(ui, theme, &response, enabled, shown, Hover::AtOnce);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = solid(ui);
    let none = Color32::TRANSPARENT;
    let (fill, edge, ink) = match (enabled, selected, lit.pressed, lit.hover) {
        (false, ..) => (none, None, p.muted),
        (true, true, true, _) => (p.cyan_muted, Some(p.cyan_edge), p.ink),
        (true, true, false, _) => (p.cyan_muted, Some(p.cyan_edge), p.cyan_text),
        (true, false, true, _) => (p.stage, Some(p.ink), p.ink),
        (true, false, false, true) => (p.stage, Some(p.muted), p.ink),
        (true, false, false, false) => (none, Some(p.line), p.muted),
    };
    match edge {
        Some(colour) => {
            painter.rect(rect, 0.0, fill, Stroke::new(1.0, colour), StrokeKind::Inside);
        }
        None => paint::dashed(&painter, rect, p.line),
    }
    painter.galley(pos2(rect.left() + PAD, rect.center().y - word.size().y / 2.0), word, ink);
    if lit.focus {
        paint::focus_ring(&painter, theme, rect, Ring::Outside);
    }
    response
}

#[cfg(test)]
#[path = "pill_tests.rs"]
mod tests;
