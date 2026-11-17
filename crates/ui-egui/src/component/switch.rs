// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The switch: a round track with a knob that slides to the right when it is on, the track then on the selection.
//! The knob moves over the hover token. Coming soon it is the unavailable switch: a dashed edge and no knob.

use egui::{Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::motion;
use crate::theme::Theme;

use super::soon;

/// A switch named `name` for a screen reader, on or off; its response is clicked when it was pressed.
pub fn switch(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    let p = &theme.palette;
    let height = theme.fonts.body.line;
    let soon = soon::is_soon(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(height * 1.8, height), soon::sense(ui, Sense::click()));
    response.widget_info(|| WidgetInfo::selected(Role::Switch, ui.is_enabled() && !soon, on, name));
    if soon {
        if ui.is_rect_visible(rect) {
            paint::dashed(ui.painter(), rect, p.muted);
        }
        return response;
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let round = rect.height() / 2.0;
        painter.rect_filled(rect, round, if on { p.stage } else { p.term });
        painter.rect_stroke(rect, round, Stroke::new(1.0, p.line), StrokeKind::Inside);
        let at = motion::toggle(ui.ctx(), response.id.with("knob"), on, theme.motion.hover_fade);
        let x = rect.left() + round + (rect.width() - 2.0 * round) * at;
        painter.circle_filled(pos2(x, rect.center().y), round - 2.0, if on { p.cyan_text } else { p.muted });
        if response.has_focus() {
            painter.rect_stroke(rect.expand(2.0), round + 2.0, Stroke::new(2.0, p.cyan_edge), StrokeKind::Outside);
        }
    }
    response
}

#[cfg(test)]
#[path = "switch_tests.rs"]
mod tests;
