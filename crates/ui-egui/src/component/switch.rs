// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The switch: a 28 × 16 box with a square 12 px knob 2 from its edge, no round shapes, the same at every text size.
//! Off: a `muted` edge and the `muted` knob at the left. On: the `cyan` fill, a `cyan_edge` edge and the `on_cyan`
//! knob at the right. Hovered: the `stage` fill, an `ink` edge and knob; pressed: the accent wash, the `cyan_edge` edge
//! and knob. Waiting (the change is on its way): a `warn` edge and the `warn` knob in the middle. Disabled (it cannot be
//! changed now): a dashed `muted` edge, the knob in `dim` at its place. Unavailable (the app cannot do this, or not
//! yet; coming soon too): the dashed edge and no knob. The row beside it carries the name and says why. The knob jumps:
//! the design gives it no transition.

use egui::{Color32, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::Theme;

use super::{Hover, Shown, lit, solid, soon};

/// The switch's width and height, in points.
const SIZE: [f32; 2] = [28.0, 16.0];

/// The knob's side, in points.
const KNOB: f32 = 12.0;

/// From the box's edge to the knob, in points.
const INSET: f32 = 2.0;

/// How a switch stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchState {
    /// Off: the knob at the left.
    Off,
    /// On: the knob at the right, on the accent.
    On,
    /// The change is on its way: the knob in the middle, in `warn`; it takes no press until the change has landed.
    Waiting,
    /// The app cannot do this, or not yet: no knob and no press.
    Unavailable,
}

/// A switch named `name` for a screen reader, on or off; its response is clicked when it was pressed.
pub fn switch(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    switch_in(ui, theme, name, if on { SwitchState::On } else { SwitchState::Off })
}

/// A switch named `name` standing as `state`; disabled (in a disabled `Ui`) it keeps its knob in `dim` at its place.
/// Its response is clicked when it was pressed.
pub fn switch_in(ui: &mut Ui, theme: &Theme, name: &str, state: SwitchState) -> Response {
    switch_with(ui, theme, name, state, None)
}

/// [`switch_in`] drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
pub(crate) fn switch_with(ui: &mut Ui, theme: &Theme, name: &str, state: SwitchState, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let state = if soon::is_soon(ui) { SwitchState::Unavailable } else { state };
    let enabled = ui.is_enabled() && state != SwitchState::Unavailable;
    let pressable = enabled && state != SwitchState::Waiting;
    let sense = if pressable { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(SIZE[0], SIZE[1]), sense);
    response.widget_info(|| WidgetInfo::selected(Role::Switch, enabled, state == SwitchState::On, name));
    let lit = lit(ui, theme, &response, pressable, shown, Hover::AtOnce);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = solid(ui);
    let none = Color32::TRANSPARENT;
    // The knob's place from the left edge, the box's fill and edge, the knob's colour.
    let place = match state {
        SwitchState::Off => Some(INSET),
        SwitchState::On => Some(SIZE[0] - INSET - KNOB),
        SwitchState::Waiting => Some((SIZE[0] - KNOB) / 2.0),
        SwitchState::Unavailable => None,
    };
    let (fill, edge, knob) = match state {
        SwitchState::Unavailable => (none, None, p.muted),
        _ if !enabled => (none, None, p.dim),
        SwitchState::Waiting => (none, Some(p.warn), p.warn),
        _ if lit.pressed => (p.cyan_muted, Some(p.cyan_edge), p.cyan_edge),
        _ if lit.hover => (p.stage, Some(p.ink), p.ink),
        SwitchState::On => (p.cyan, Some(p.cyan_edge), p.on_cyan),
        SwitchState::Off => (none, Some(p.muted), p.muted),
    };
    match edge {
        Some(colour) => {
            painter.rect(rect, 0.0, fill, Stroke::new(1.0, colour), StrokeKind::Inside);
        }
        None => paint::dashed(&painter, rect, p.muted),
    }
    if let Some(x) = place {
        painter.rect_filled(Rect::from_min_size(rect.min + vec2(x, INSET), vec2(KNOB, KNOB)), 0.0, knob);
    }
    if lit.focus {
        paint::focus_ring(&painter, theme, rect, Ring::Outside);
    }
    response
}

#[cfg(test)]
#[path = "switch_tests.rs"]
mod tests;
