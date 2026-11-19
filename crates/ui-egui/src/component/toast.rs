// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The toast: a short message on a float that rises from the bottom edge over the toast motion token, with Undo as an
//! accent word and its key chip for a reversible action. The Undo key works while the toast shows.

use egui::{Align2, Area, Context, Id, Order, Pos2, Shape, Ui, vec2};

use crate::draw::paint::{self, FloatEdge};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::Theme;
use crate::tr;

use super::{button, galley, key, margin, soon};

/// What a toast says.
#[derive(Debug, Clone, Copy)]
pub struct Toast<'a> {
    /// The message.
    pub text: &'a str,
    /// The undo action of a reversible action: an Undo button and its key.
    pub undo: Option<&'a str>,
    /// Undo rests: drawn unavailable, its button and its key do nothing (the toast still says what happened).
    pub rests: bool,
}

/// Where a toast stands: the middle of its foot, the widest its float may be, and the lowest its foot may go while it
/// rises (what lies under that line, a composer's foot, stays clear of it).
#[derive(Debug, Clone, Copy)]
pub struct Place {
    /// The middle of the toast's foot, risen.
    pub bottom: Pos2,
    /// The widest the float may be: its words are cut with one `…` to stay inside.
    pub widest: f32,
    /// The lowest the toast's foot may stand while it rises.
    pub lowest: f32,
}

/// How wide what follows the words stands: the gap, Undo and its key chip; nothing without Undo.
fn undo_wide(ui: &Ui, theme: &Theme, keymap: &Keymap, toast: &Toast<'_>) -> f32 {
    let s = &theme.spacing;
    let Some(action) = toast.undo else { return 0.0 };
    let chip = keymap.chip(ui.ctx(), action).map_or(0.0, |chip| s.s2 + key::Chip::new(ui, theme, &chip, key::ChipState::Default).size().x);
    s.s2 + button::link_width(ui, theme, tr!("Undo")) + chip
}

/// How wide the toast's float is with its words whole.
#[must_use]
pub fn width(ui: &Ui, theme: &Theme, keymap: &Keymap, toast: &Toast<'_>) -> f32 {
    let words = galley(ui, toast.text, theme.fonts.body, theme.palette.ink).size().x;
    words + undo_wide(ui, theme, keymap, toast) + theme.spacing.s4 * 2.0
}

/// Draws the toast's float and content into `ui`, no wider than `widest` (its words cut with one `…` to fit); whether
/// Undo was pressed (the button or its key).
pub fn panel(ui: &mut Ui, theme: &Theme, keymap: &Keymap, toast: &Toast<'_>, widest: f32) -> bool {
    let s = &theme.spacing;
    let p = &theme.palette;
    let slot = ui.painter().add(Shape::Noop);
    let mut undone = false;
    // Undo's own room makes the line taller, so the float's margin gives that back.
    let air = if toast.undo.is_some() { s.s1 } else { s.s2 };
    let room = widest - s.s4 * 2.0 - undo_wide(ui, theme, keymap, toast);
    let words = if room.is_finite() {
        super::cut(ui, toast.text, theme.fonts.body, p.ink, room.max(1.0))
    } else {
        galley(ui, toast.text, theme.fonts.body, p.ink)
    };
    let inner = egui::Frame::new().inner_margin(margin(s.s4, air)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s2;
            ui.label(words);
            if let Some(action) = toast.undo {
                let undo = button::link_with(ui, theme, keymap, tr!("Undo"), button::LinkSize::Line, !toast.rests, None);
                undone = !toast.rests && (undo.clicked() || (!soon::is_soon(ui) && keymap.pressed(ui.ctx(), action)));
                if let Some(chip) = keymap.chip(ui.ctx(), action) {
                    key::key_chip(ui, theme, &chip, key::ChipState::Default);
                }
            }
        });
    });
    let shapes = paint::float_shapes(theme, inner.response.rect, FloatEdge::Quiet);
    ui.painter().set(slot, Shape::Vec(shapes));
    undone
}

/// Shows the toast `id` at its `place`, rising into it; whether Undo was pressed.
pub fn show(ctx: &Context, theme: &Theme, keymap: &Keymap, id: Id, toast: &Toast<'_>, place: Place) -> bool {
    let risen = motion::appear(ctx, id, theme.motion.toast);
    let below = (theme.spacing.s6 * (1.0 - risen)).min((place.lowest - place.bottom.y).max(0.0));
    let at = place.bottom + vec2(0.0, below);
    Area::new(id)
        .order(Order::Foreground)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(at)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(risen);
            panel(ui, theme, keymap, toast, place.widest)
        })
        .inner
}

#[cfg(test)]
#[path = "toast_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "toast_cover_tests.rs"]
mod cover_tests;
