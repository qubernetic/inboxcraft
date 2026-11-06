// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Badges: an unread count in a pill of the badge colour, and a group's badge, its solid (Fill) icon on the preset's
//! gradient at full strength, white wherever white reaches 3:1 on every stop, otherwise the deep ink.

use egui::{Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::gradient::{self, Gradient, Stop};
use crate::iconset::{IconId, Style};
use crate::theme::{Theme, icon_ink_over};

use super::galley;

/// The rectangle of a count badge for `count`, its right edge at `right`, centred on `centre_y`.
fn count_rect(ui: &Ui, theme: &Theme, count: u64, right: f32, centre_y: f32) -> (Rect, std::sync::Arc<egui::Galley>) {
    let text = galley(ui, &count.to_string(), theme.fonts.caption, theme.palette.on_badge);
    let height = text.size().y + 2.0;
    let width = (text.size().x + theme.spacing.s * 1.5).max(height);
    (Rect::from_min_size(pos2(right - width, centre_y - height / 2.0), vec2(width, height)), text)
}

/// Paints a count badge with `count` in it, its right edge at `right`, centred on `centre_y`; returns its left edge.
pub fn count(ui: &Ui, theme: &Theme, count: u64, right: f32, centre_y: f32) -> f32 {
    let p = &theme.palette;
    let (rect, text) = count_rect(ui, theme, count, right, centre_y);
    ui.painter().rect_filled(rect, rect.height() / 2.0, p.badge);
    ui.painter().galley(rect.center() - text.size() / 2.0, text, p.on_badge);
    rect.left()
}

/// The ink of a group badge's icon in `theme`.
#[must_use]
pub fn group_ink(theme: &Theme) -> egui::Color32 {
    icon_ink_over(&theme.palette.gradient)
}

/// A group's badge of `side` points: `icon` on the preset's gradient; a screen reader reads `name`.
pub fn group(ui: &mut Ui, theme: &Theme, icon: IconId, name: &str, side: f32) -> Response {
    let side = if side.is_finite() { side.clamp(1.0, 512.0) } else { 1.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, name));
    if ui.is_rect_visible(rect) {
        let [first, middle, last] = theme.palette.gradient;
        let gradient = Gradient::Linear {
            from: pos2(0.0, 0.0),
            to: pos2(1.0, 1.0),
            stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
        };
        gradient::fill(ui.painter(), rect, side * 0.3, &gradient);
        theme.icons.paint(ui.painter(), rect.shrink(side * 0.2), icon, Style::Fill, group_ink(theme));
    }
    response
}

#[cfg(test)]
#[path = "badge_tests.rs"]
mod tests;
