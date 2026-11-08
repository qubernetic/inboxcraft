// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The popup: a float with menu rows (an icon, a name and the key chip) and choice rows (one of a few, the chosen one
//! marked), opening over the card motion token.

use egui::{Align2, Area, Context, Id, Order, Pos2, Rect, Response, Role, Sense, Shape, Ui, WidgetInfo, pos2, vec2};

use crate::glass;
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::Theme;

use super::{control_side, cut, galley, margin};

/// Draws a float of `width` points into `ui` with `add` drawing its content.
pub fn panel<R>(ui: &mut Ui, theme: &Theme, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let slot = ui.painter().add(Shape::Noop);
    let width = if width.is_finite() { width.clamp(1.0, 2048.0) } else { 1.0 };
    let inner = egui::Frame::new().inner_margin(margin(theme.spacing.xs, theme.spacing.xs)).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width((width - theme.spacing.xs * 2.0).max(1.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .inner
    });
    let shapes = glass::float_shapes(ui.painter(), theme, inner.response.rect);
    ui.painter().set(slot, Shape::Vec(shapes));
    inner.inner
}

/// Shows the popup `id` with its top left corner at `anchor`, opening into place; what `add` gave.
pub fn show<R>(ctx: &Context, theme: &Theme, id: Id, anchor: Pos2, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let opened = motion::appear(ctx, id, theme.motion.card);
    let at = anchor - vec2(0.0, theme.spacing.xs * (1.0 - opened));
    Area::new(id)
        .order(Order::Foreground)
        .pivot(Align2::LEFT_TOP)
        .fixed_pos(at)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(opened);
            panel(ui, theme, width, add)
        })
        .inner
}

/// A menu row: `icon`, `name` and the key of `action` from `keymap` at its right end; clicked when chosen.
pub fn item(ui: &mut Ui, theme: &Theme, keymap: &Keymap, icon: IconId, name: &str, action: Option<&str>) -> Response {
    row(ui, theme, keymap, Some(icon), name, action, None)
}

/// A menu row without an icon, its name in line with the names of the rows that have one.
pub fn entry(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>) -> Response {
    row(ui, theme, keymap, None, name, action, None)
}

/// A choice row: `name`, marked with the Done icon when it is the one `chosen`; clicked when picked.
pub fn choice(ui: &mut Ui, theme: &Theme, name: &str, chosen: bool) -> Response {
    row(ui, theme, &Keymap::empty(), chosen.then_some(IconId::Done), name, None, Some(chosen))
}

fn row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, icon: Option<IconId>, name: &str, action: Option<&str>, chosen: Option<bool>) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = control_side(ui, theme);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width().max(height), height), Sense::click());
    response.widget_info(|| match chosen {
        Some(chosen) => WidgetInfo::selected(Role::MenuItemRadio, true, chosen, name),
        None => WidgetInfo::labeled(Role::MenuItem, true, name),
    });
    let lit = response.hovered() || response.has_focus();
    let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if hover > 0.0 {
            painter.rect_filled(rect, s.corner, p.hover.gamma_multiply(hover));
        }
        let side = theme.fonts.body.size * 1.2;
        let icon_rect = Rect::from_center_size(pos2(rect.left() + s.s + side / 2.0, rect.center().y), vec2(side, side));
        if let Some(icon) = icon {
            let colour = if chosen.is_some() { p.accent } else { p.text };
            theme.icons.paint(painter, icon_rect, icon, Style::Regular, colour);
        }
        let mut right = rect.right() - s.s;
        if let Some(chip) = action.and_then(|action| keymap.chip(ui.ctx(), action)) {
            let chip = galley(ui, &chip, theme.fonts.caption, p.muted);
            right -= chip.size().x;
            painter.galley(pos2(right, rect.center().y - chip.size().y / 2.0), chip, p.muted);
            right -= s.l;
        }
        let left = icon_rect.right() + s.s;
        let text = cut(ui, name, theme.fonts.body, p.text, right - left);
        painter.galley(pos2(left, rect.center().y - text.size().y / 2.0), text, p.text);
    }
    response
}

#[cfg(test)]
#[path = "popup_tests.rs"]
mod tests;
