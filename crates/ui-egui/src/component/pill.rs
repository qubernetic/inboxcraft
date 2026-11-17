// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pill: a square chip with a word in it, for filters and choices. Selected it is the `stage` fill with a
//! SemiBold label; otherwise it has no fill and a `muted` edge. Hovered it takes the `grid` fill.

use egui::{Painter, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::motion;
use crate::theme::Theme;

use super::galley;

/// A selected pill or segment over `rect`: the `stage` fill.
pub(crate) fn selected_fill(painter: &Painter, theme: &Theme, rect: Rect) {
    painter.rect_filled(rect, 0.0, theme.palette.stage);
}

/// A pill with `text` in it, selected or not; its response is clicked when it was pressed.
pub fn pill(ui: &mut Ui, theme: &Theme, text: &str, selected: bool) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let font = if selected { theme.fonts.button.semibold() } else { theme.fonts.button };
    let text_galley = galley(ui, text, font, p.ink);
    let height = theme.fonts.button.line + s.s2;
    let size = vec2(text_galley.size().x + s.s3 * 2.0, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, text));
    let lit = response.hovered() || response.has_focus();
    let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let round = 0.0;
        if selected {
            selected_fill(painter, theme, rect);
        } else {
            if hover > 0.0 {
                painter.rect_filled(rect, round, p.stage);
            }
            painter.rect_stroke(rect, round, Stroke::new(1.0, p.muted), StrokeKind::Inside);
        }
        let at = rect.center() - text_galley.size() / 2.0;
        painter.galley(pos2(at.x, at.y), text_galley, p.ink);
        if response.has_focus() {
            painter.rect_stroke(
                Rect::from_center_size(rect.center(), rect.size() - vec2(2.0, 2.0)),
                round,
                Stroke::new(1.0, p.cyan_edge),
                StrokeKind::Inside,
            );
        }
    }
    response
}

#[cfg(test)]
#[path = "pill_tests.rs"]
mod tests;
