// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Text fields: the search pill (a round field with a magnifier before the text) and the margin of a form field, so
//! every field is as tall as a button with room around its text.

use egui::{Align2, Id, Margin, Rect, Response, Sense, Stroke, StrokeKind, TextEdit, Ui, pos2, vec2};

use crate::iconset::{IconId, Style};
use crate::theme::Theme;

use super::control_side;

/// The margin inside a form field: the field is as tall as a button with a little more room.
#[must_use]
pub fn margin(theme: &Theme) -> Margin {
    let s = &theme.spacing;
    // A few points each; the clamp keeps the casts inside i8.
    Margin::symmetric(s.s.clamp(0.0, 127.0) as i8, (s.xs + 2.0).clamp(0.0, 127.0) as i8)
}

/// The search pill `width` points wide: the field colour with its border (the focus ring while it has the focus), a
/// magnifier and the text, `hint` while it is empty. Its response is the text field's.
pub fn search(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, hint: &str, width: f32) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = control_side(ui, theme) + s.xs;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let round = height / 2.0;
    let focused = ui.memory(|memory| memory.has_focus(id));
    let (edge, ink) = if focused { (Stroke::new(2.0, p.focus), p.text) } else { (Stroke::new(1.0, p.field_border), p.muted) };
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, round, p.field);
        ui.painter().rect_stroke(rect, round, edge, StrokeKind::Inside);
    }
    let icon = theme.fonts.body.size * 1.15;
    let at = Rect::from_center_size(pos2(rect.left() + s.m + icon / 2.0, rect.center().y), vec2(icon, icon));
    theme.icons.paint(ui.painter(), at, IconId::Search, Style::Regular, ink);
    let inner = Rect::from_min_max(pos2(at.right() + s.s, rect.top()), pos2(rect.right() - s.m, rect.bottom()));
    let edit = TextEdit::singleline(text)
        .id(id)
        .hint_text(hint)
        .frame(egui::Frame::NONE)
        .margin(Margin::ZERO)
        .align(Align2::LEFT_CENTER)
        .desired_width(inner.width());
    ui.put(inner, edit)
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
