// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Check boxes and radio buttons: a box or a ring before its name. A ticked box is filled in the accent with the tick
//! on it, a chosen ring carries a dot of the accent; off they are the field's colour with a quiet edge. The whole row
//! answers a press.

use egui::{Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::motion;
use crate::theme::Theme;

use super::{cut, line_height, tick};

/// The side of the box or the ring, in points.
const BOX: f32 = 18.0;

/// The corner of the box, in points.
const CORNER: f32 = 5.0;

/// The side of the tick, in points.
const TICK: f32 = 12.0;

/// The diameter of a chosen ring's dot, in points.
const DOT: f32 = 8.0;

/// Between the box or ring and its name, in points.
const GAP: f32 = 8.0;

/// The box or ring and `name` after it, in the room there is; the response is clicked when it was pressed.
fn row(ui: &mut Ui, theme: &Theme, role: Role, name: &str, on: bool) -> (Response, egui::Rect) {
    let p = &theme.palette;
    let high = line_height(ui, theme.fonts.body).max(BOX);
    let text = cut(ui, name, theme.fonts.body, p.text, f32::INFINITY);
    let wanted = BOX + GAP + text.size().x;
    let wide = wanted.min(ui.available_width().max(BOX));
    let (rect, response) = ui.allocate_exact_size(vec2(wide, high), Sense::click());
    response.widget_info(|| WidgetInfo::selected(role, ui.is_enabled(), on, name));
    let mark = egui::Rect::from_min_size(pos2(rect.left(), rect.center().y - BOX / 2.0), vec2(BOX, BOX));
    if ui.is_rect_visible(rect) {
        let room = rect.right() - mark.right() - GAP;
        let text = if wide < wanted { cut(ui, name, theme.fonts.body, p.text, room) } else { text };
        ui.painter().galley(pos2(mark.right() + GAP, rect.center().y - text.size().y / 2.0), text, p.text);
    }
    (response, mark)
}

/// Lays the hover fade over `mark` as `round` as it should be.
fn hover(ui: &Ui, theme: &Theme, response: &Response, mark: egui::Rect, round: f32) {
    let shown = motion::toggle(ui.ctx(), response.id.with("hover"), response.hovered(), theme.motion.hover_fade);
    if shown > 0.0 {
        ui.painter().rect_filled(mark, round, theme.palette.hover.gamma_multiply(shown));
    }
}

/// A check box named `name`; clicked when it was pressed.
pub fn checkbox(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    let (response, mark) = row(ui, theme, Role::CheckBox, name, on);
    if ui.is_rect_visible(mark) {
        let p = &theme.palette;
        let painter = ui.painter();
        if on {
            painter.rect_filled(mark, CORNER, p.accent);
            tick(painter, egui::Rect::from_center_size(mark.center(), vec2(TICK, TICK)), p.on_accent);
        } else {
            painter.rect_filled(mark, CORNER, p.field);
            painter.rect_stroke(mark, CORNER, Stroke::new(1.0, p.field_border), StrokeKind::Inside);
        }
        hover(ui, theme, &response, mark, CORNER);
        if response.has_focus() {
            painter.rect_stroke(mark.expand(2.0), CORNER + 2.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
        }
    }
    response
}

/// A radio button named `name`, the chosen one of its row or not; clicked when it was pressed.
pub fn radio(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    let (response, mark) = row(ui, theme, Role::RadioButton, name, on);
    if ui.is_rect_visible(mark) {
        let p = &theme.palette;
        let painter = ui.painter();
        let round = BOX / 2.0;
        painter.rect_filled(mark, round, p.field);
        let edge = if on { Stroke::new(2.0, p.accent) } else { Stroke::new(1.0, p.field_border) };
        painter.rect_stroke(mark, round, edge, StrokeKind::Inside);
        if on {
            painter.circle_filled(mark.center(), DOT / 2.0, p.accent);
        }
        hover(ui, theme, &response, mark, round);
        if response.has_focus() {
            painter.rect_stroke(mark.expand(2.0), round + 2.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
        }
    }
    response
}

#[cfg(test)]
#[path = "checkbox_tests.rs"]
mod tests;
