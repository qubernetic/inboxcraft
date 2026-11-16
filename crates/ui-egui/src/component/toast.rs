// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The toast: a short message on a float that rises from the bottom edge over the toast motion token, with Undo as an
//! accent word and its key chip for a reversible action. The Undo key works while the toast shows.

use egui::{Align2, Area, Context, Id, Order, Pos2, Shape, Ui, vec2};

use crate::glass;
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::Theme;
use crate::tr;

use super::{button, galley, margin, tooltip};

/// What a toast says.
#[derive(Debug, Clone, Copy)]
pub struct Toast<'a> {
    /// The message.
    pub text: &'a str,
    /// The undo action of a reversible action: an Undo button and its key.
    pub undo: Option<&'a str>,
}

/// Draws the toast's float and content into `ui`; whether Undo was pressed (the button or its key).
pub fn panel(ui: &mut Ui, theme: &Theme, keymap: &Keymap, toast: &Toast<'_>) -> bool {
    let s = &theme.spacing;
    let p = &theme.palette;
    let slot = ui.painter().add(Shape::Noop);
    let mut undone = false;
    // Undo's own room makes the line taller, so the float's margin gives that back.
    let air = if toast.undo.is_some() { s.xs } else { s.s };
    let inner = egui::Frame::new().inner_margin(margin(s.l, air)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            ui.label(galley(ui, toast.text, theme.fonts.body, p.text));
            if let Some(action) = toast.undo {
                let undo = button::link(ui, theme, keymap, tr!("Undo"));
                undone = undo.clicked() || keymap.pressed(ui.ctx(), action);
                if let Some(chip) = keymap.chip(ui.ctx(), action) {
                    tooltip::key_chip(ui, theme, &chip);
                }
            }
        });
    });
    let shapes = glass::float_shapes(ui.painter(), theme, inner.response.rect);
    ui.painter().set(slot, Shape::Vec(shapes));
    undone
}

/// Shows the toast `id` with its bottom edge centred on `bottom`, rising into place; whether Undo was pressed.
pub fn show(ctx: &Context, theme: &Theme, keymap: &Keymap, id: Id, toast: &Toast<'_>, bottom: Pos2) -> bool {
    let risen = motion::appear(ctx, id, theme.motion.toast);
    let at = bottom + vec2(0.0, theme.spacing.xl * (1.0 - risen));
    Area::new(id)
        .order(Order::Foreground)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(at)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(risen);
            panel(ui, theme, keymap, toast)
        })
        .inner
}

#[cfg(test)]
#[path = "toast_tests.rs"]
mod tests;
