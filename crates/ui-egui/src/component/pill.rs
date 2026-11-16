// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pill: a fully rounded chip with a word in it, for filters and choices. Selected it is one even tone with an
//! edge in the full gradient and a SemiBold label; otherwise it has no fill and a quiet edge. Its hover fill fades in
//! over the hover token.

use egui::{Painter, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::gradient::{self, Gradient, Stop};
use crate::motion;
use crate::theme::{Face, Font, Theme, Weight};

use super::{galley, line_height};

/// The width of the gradient edge of a selected pill or segment.
const EDGE: f32 = 1.5;

/// A selected pill or segment over `rect`: the selection inside an edge in the full gradient.
pub(crate) fn selected_fill(painter: &Painter, theme: &Theme, rect: Rect, round: f32) {
    let p = &theme.palette;
    let [first, middle, last] = p.gradient;
    let edge = Gradient::Linear {
        from: pos2(0.0, 0.5),
        to: pos2(1.0, 0.5),
        stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
    };
    gradient::fill(painter, rect, round, &edge);
    painter.rect_filled(rect.shrink(EDGE), (round - EDGE).max(0.0), p.selection);
}

/// A pill with `text` in it, selected or not; its response is clicked when it was pressed.
pub fn pill(ui: &mut Ui, theme: &Theme, text: &str, selected: bool) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let font = if selected { Font { face: Face::Sans(Weight::SemiBold), ..theme.fonts.button } } else { theme.fonts.button };
    let text_galley = galley(ui, text, font, p.text);
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
            selected_fill(painter, theme, rect, round);
        } else {
            painter.rect_stroke(rect, round, Stroke::new(1.0, p.control_edge), StrokeKind::Inside);
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
