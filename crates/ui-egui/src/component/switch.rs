// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The switch: a round track with a knob that slides to the right when it is on, the track then on the selection
//! gradient. The knob moves over the hover token.

use egui::{Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::gradient::{self, Gradient, Stop};
use crate::motion;
use crate::theme::Theme;

use super::line_height;

/// A switch named `name` for a screen reader, on or off; its response is clicked when it was pressed.
pub fn switch(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    let p = &theme.palette;
    let height = line_height(ui, theme.fonts.body);
    let (rect, response) = ui.allocate_exact_size(vec2(height * 1.8, height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Switch, true, on, name));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let round = rect.height() / 2.0;
        if on {
            let across = Gradient::Linear {
                from: pos2(0.0, 0.5),
                to: pos2(1.0, 0.5),
                stops: vec![Stop::new(0.0, p.selection), Stop::new(1.0, p.selection_end)],
            };
            gradient::fill(painter, rect, round, &across);
        } else {
            painter.rect_filled(rect, round, p.field);
        }
        painter.rect_stroke(rect, round, Stroke::new(1.0, p.field_border), StrokeKind::Inside);
        let at = motion::toggle(ui.ctx(), response.id.with("knob"), on, theme.motion.hover_fade);
        let x = rect.left() + round + (rect.width() - 2.0 * round) * at;
        painter.circle_filled(pos2(x, rect.center().y), round - 2.0, if on { p.accent } else { p.muted });
        if response.has_focus() {
            painter.rect_stroke(rect.expand(2.0), round + 2.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
        }
    }
    response
}

#[cfg(test)]
#[path = "switch_tests.rs"]
mod tests;
