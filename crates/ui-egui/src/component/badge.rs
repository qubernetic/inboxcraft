// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Badges: an unread count in `cyan_text` on a `cyan_muted` box, and a group's badge, its outline (Regular) icon in
//! the muted colour without a background; and a chip, a short name in the same colours as the count. All square.

use egui::{Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::iconset::IconId;
use crate::theme::Theme;

use super::galley;

/// The rectangle of a count badge for `count`, its right edge at `right`, centred on `centre_y`.
fn count_rect(ui: &Ui, theme: &Theme, count: u64, right: f32, centre_y: f32) -> (Rect, std::sync::Arc<egui::Galley>) {
    let text = galley(ui, &count.to_string(), theme.fonts.mono_s, theme.palette.cyan_text);
    let height = text.size().y + 2.0;
    let width = (text.size().x + theme.spacing.s2 * 1.5).max(height);
    (Rect::from_min_size(pos2(right - width, centre_y - height / 2.0), vec2(width, height)), text)
}

/// The width of a count badge for `count`.
#[must_use]
pub fn count_width(ui: &Ui, theme: &Theme, count: u64) -> f32 {
    count_rect(ui, theme, count, 0.0, 0.0).0.width()
}

/// Paints a count badge with `count` in it, its right edge at `right`, centred on `centre_y`; returns its left edge.
pub fn count(ui: &Ui, theme: &Theme, count: u64, right: f32, centre_y: f32) -> f32 {
    let p = &theme.palette;
    let (rect, text) = count_rect(ui, theme, count, right, centre_y);
    ui.painter().rect_filled(rect, 0.0, p.cyan_muted);
    ui.painter().galley(rect.center() - text.size() / 2.0, text, p.cyan_text);
    rect.left()
}

/// The ink of a group badge's icon in `theme`: the muted colour.
#[must_use]
pub fn group_ink(theme: &Theme) -> egui::Color32 {
    theme.palette.muted
}

/// A group's badge of `side` points: `icon` as an outline in the muted colour, without a background; a screen reader
/// reads `name`.
pub fn group(ui: &mut Ui, theme: &Theme, icon: IconId, name: &str, side: f32) -> Response {
    let side = if side.is_finite() { side.clamp(1.0, 512.0) } else { 1.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, name));
    if ui.is_rect_visible(rect) {
        crate::iconset::paint(ui.painter(), rect.shrink(side * 0.15), icon, group_ink(theme));
    }
    response
}

/// A chip: `name` in the caption font, `cyan_text` on a `cyan_muted` box; a screen reader reads `spoken`.
pub fn chip(ui: &mut Ui, theme: &Theme, name: &str, spoken: &str) -> Response {
    let s = &theme.spacing;
    let p = theme.palette;
    let ink = p.cyan_text;
    let text = galley(ui, name, theme.fonts.mono_s, ink);
    let size = vec2(text.size().x + s.s2 * 2.0, theme.boxes().chip);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, spoken));
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 0.0, p.cyan_muted);
        ui.painter().galley(rect.center() - text.size() / 2.0, text, ink);
    }
    response
}

#[cfg(test)]
#[path = "badge_tests.rs"]
mod tests;
