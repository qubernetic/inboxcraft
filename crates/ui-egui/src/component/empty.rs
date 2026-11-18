// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The empty state of a pane: a label in capitals, a title in mono 22/30 and a line of help, all `muted`, in a block
//! 440 wide, 40 above and below, 24 left and right, 12 between the lines, in the middle of the pane. A bare one has its
//! lines centred; one with an action (a primary text button 8 under the help) has them on the left, as the action's
//! sheet draws them. No fill, no edge, nothing moves.

use std::sync::Arc;

use egui::{Align, Galley, Rect, Response, Role, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::button::TextButton;
use super::control_side;

/// The block's width, in points.
const WIDTH: f32 = 440.0;

/// The room above and below the lines, in points.
const PAD_Y: f32 = 40.0;

/// The room left and right of the lines, in points.
const PAD_X: f32 = 24.0;

/// The room between two lines, in points.
const GAP: f32 = 12.0;

/// The room over the action, under the gap after the help, in points.
const ACTION_TOP: f32 = 8.0;

/// The most lines the title and the help each take.
const ROWS: usize = 4;

/// The empty state of the pane `ui` stands in: `label`, `title` (a line break where the title has one) and `help`, its
/// lines centred, in the middle of the pane.
pub fn empty_state(ui: &mut Ui, theme: &Theme, label: &str, title: &str, help: &str) {
    block(ui, theme, label, title, help, None);
}

/// [`empty_state`] with `action` under the help, its lines on the left as its sheet draws them; the action's response.
pub fn empty_state_with(ui: &mut Ui, theme: &Theme, label: &str, title: &str, help: &str, action: TextButton<'_>, keymap: &Keymap) -> Response {
    let response = block(ui, theme, label, title, help, Some((action, keymap)));
    // `block` always draws a given action; a pane with no room for it still answers with the block's own response.
    response.unwrap_or_else(|| ui.response())
}

/// The block in the middle of `ui`'s room: the action's response when there is one.
fn block(ui: &mut Ui, theme: &Theme, label: &str, title: &str, help: &str, action: Option<(TextButton<'_>, &Keymap)>) -> Option<Response> {
    let p = &theme.palette;
    let f = &theme.fonts;
    let centred = action.is_none();
    let align = if centred { Align::Center } else { Align::LEFT };
    let width = WIDTH.min(ui.available_width()).max(PAD_X * 2.0 + 1.0).floor();
    let inner = width - PAD_X * 2.0;
    let lines: [Arc<Galley>; 3] = [
        super::wrapped(ui, label, f.label, p.muted, inner, 1, align),
        super::wrapped(ui, title, f.title, p.muted, inner, ROWS, align),
        super::wrapped(ui, help, f.caption, p.muted, inner, ROWS, align),
    ];
    let text_high: f32 = lines.iter().map(|line| line.size().y).sum::<f32>() + GAP * 2.0;
    let action_high = if action.is_some() { GAP + ACTION_TOP + control_side(theme) } else { 0.0 };
    let high = (PAD_Y + text_high + action_high + PAD_Y).ceil();
    let room = ui.available_rect_before_wrap();
    let pane_high = if room.height().is_finite() { room.height().max(high) } else { high };
    let (_, pane) = ui.allocate_space(vec2(room.width().max(width), pane_high));
    let min = (pane.center() - vec2(width, high) / 2.0).round();
    let rect = Rect::from_min_size(min, vec2(width, high));
    let response = ui.interact(rect, ui.scope_id().with("empty-state").with(title), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, title));
    let mut top = rect.top() + PAD_Y;
    if ui.is_rect_visible(rect) {
        for line in lines {
            let high = line.size().y;
            // Each row at the text box's left, or in its middle, as the design places it.
            paint::rows_at(ui.painter(), pos2(rect.left() + PAD_X, top), inner, line, p.muted);
            top += high + GAP;
        }
    } else {
        top += text_high + GAP;
    }
    let (button, keymap) = action?;
    let at = Rect::from_min_size(pos2(rect.left() + PAD_X, top + ACTION_TOP), vec2(inner, control_side(theme)));
    let mut child = ui.new_child(UiBuilder::new().max_rect(at).layout(egui::Layout::left_to_right(Align::Center)));
    Some(button.show(&mut child, theme, keymap))
}

#[cfg(test)]
#[path = "empty_tests.rs"]
mod tests;
