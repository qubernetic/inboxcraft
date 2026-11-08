// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tooltip: a rounded bubble with the action's name and, when the action has a key, the key as a chip. The key
//! is read from the [`Keymap`] each time the bubble is drawn, so a changed key shows at once. It appears after
//! egui's short hover delay and fades in over the tooltip motion token; the bubble is a float over sharp text, since
//! a tooltip may sit over the reader. A control that cannot be used yet adds a note under the name that says why.

use egui::{Frame, Response, Shape, Tooltip, Ui};

use crate::glass;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::galley;

/// The chip of a key (`Ctrl+F`): small text in a rounded frame.
pub fn key_chip(ui: &mut Ui, theme: &Theme, key: &str) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    Frame::new()
        .fill(p.panel)
        .stroke(egui::Stroke::new(1.0, p.field_border))
        .corner_radius(super::corner(s.corner))
        .inner_margin(super::margin(s.xs * 1.5, 1.0))
        .show(ui, |ui| ui.label(egui::RichText::new(key).font(theme.fonts.caption.id()).color(p.text)))
        .inner
}

/// The bubble's content: the name of the action and the chip of its key from `keymap`, on a float drawn under them.
pub fn bubble(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>) {
    noted(ui, theme, keymap, name, action, None);
}

/// The bubble's content with `note` in muted caption text under the name, when there is one.
fn noted(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, note: Option<&str>) {
    let s = &theme.spacing;
    let slot = ui.painter().add(Shape::Noop);
    let chip = action.and_then(|action| keymap.chip(ui.ctx(), action));
    let inner = Frame::new().inner_margin(super::margin(s.m, s.xs * 1.5)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            let text = galley(ui, name, theme.fonts.body, theme.palette.text);
            ui.label(text);
            if let Some(chip) = chip {
                key_chip(ui, theme, &chip);
            }
        });
        if let Some(note) = note {
            ui.label(galley(ui, note, theme.fonts.caption, theme.palette.muted));
        }
    });
    let shapes = glass::float_shapes(ui.painter(), theme, inner.response.rect);
    ui.painter().set(slot, Shape::Vec(shapes));
}

/// Gives `response` a tooltip with `name` and the key of `action`, shown while it is hovered, enabled or not.
pub fn attach(response: Response, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>) -> Response {
    attach_note(response, theme, keymap, name, action, None)
}

/// Gives `response` a tooltip with `name`, the key of `action` and `note` under them, shown while it is hovered.
pub fn attach_note(response: Response, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, note: Option<&str>) -> Response {
    let mut tooltip = Tooltip::for_widget(&response);
    let open = Tooltip::should_show_tooltip(&response, false);
    tooltip.popup = tooltip.popup.open(open).frame(Frame::NONE);
    tooltip.show(|ui| noted(ui, theme, keymap, name, action, note));
    response
}

#[cfg(test)]
#[path = "tooltip_tests.rs"]
mod tests;
