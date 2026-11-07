// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pill: a fully rounded chip with a word in it, for filters and choices. Selected it carries the selection
//! gradient; otherwise the panel colour with a control edge. Its hover fill fades in over the hover token.

use egui::{Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::gradient::{self, Gradient, Stop};
use crate::motion;
use crate::theme::Theme;

use super::{galley, line_height};

/// A pill with `text` in it, selected or not; its response is clicked when it was pressed.
pub fn pill(ui: &mut Ui, theme: &Theme, text: &str, selected: bool) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let text_galley = galley(ui, text, theme.fonts.button, p.text);
    let height = line_height(ui, theme.fonts.button) + s.s;
    let size = vec2(text_galley.size().x + s.m * 2.0, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, text));
    let lit = response.hovered() || response.has_focus();
    let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let round = height / 2.0;
        if selected {
            let gradient = Gradient::Linear {
                from: pos2(0.0, 0.5),
                to: pos2(1.0, 0.5),
                stops: vec![Stop::new(0.0, p.selection), Stop::new(1.0, p.selection_end)],
            };
            gradient::fill(painter, rect, round, &gradient);
        } else {
            painter.rect_filled(rect, round, p.panel);
            painter.rect_stroke(rect, round, Stroke::new(1.0, p.field_border), StrokeKind::Inside);
        }
        if hover > 0.0 {
            painter.rect_filled(rect, round, p.hover.gamma_multiply(hover));
        }
        let at = rect.center() - text_galley.size() / 2.0;
        painter.galley(pos2(at.x, at.y), text_galley, p.text);
        if response.has_focus() {
            painter.rect_stroke(
                Rect::from_center_size(rect.center(), rect.size() - vec2(2.0, 2.0)),
                round,
                Stroke::new(2.0, p.focus),
                StrokeKind::Inside,
            );
        }
    }
    response
}

#[cfg(test)]
#[path = "pill_tests.rs"]
mod tests;
