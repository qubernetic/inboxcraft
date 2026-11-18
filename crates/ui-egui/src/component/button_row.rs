// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A row of text buttons in a room: all on one line where they fit; else as many on a line as fit, in order, a gap
//! apart, so none is cut at the room's edge. Measured once a frame with [`TextButton::width`], then drawn on the lines
//! the measure gave. A button wider than the room alone stands on its own line (the button keeps its words whole).

use egui::{Align, Layout, Rect, Ui, UiBuilder, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;

use super::button::TextButton;

/// How a row of buttons stands in a room: the line of each, and how high the lines are.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonLines {
    lines: Vec<usize>,
    /// How high the lines stand, with the gaps between them, in points.
    pub height: f32,
}

/// The lines `buttons` stand on in `room` points, `gap` apart.
pub fn button_lines(ui: &Ui, theme: &Theme, keymap: &Keymap, buttons: &[TextButton<'_>], room: f32, gap: f32) -> ButtonLines {
    let mut lines = Vec::with_capacity(buttons.len());
    let (mut line, mut used) = (0_usize, 0.0_f32);
    for button in buttons {
        let wide = button.width(ui, theme, keymap);
        if used > 0.0 && used + gap + wide > room {
            line = line.saturating_add(1);
            used = 0.0;
        }
        used += if used > 0.0 { gap + wide } else { wide };
        lines.push(line);
    }
    let count = if buttons.is_empty() { 0.0 } else { line as f32 + 1.0 };
    let side = super::control_side(theme);
    ButtonLines { lines, height: count * side + (count - 1.0).max(0.0) * gap }
}

/// Draws `buttons` on the lines `placed` gave them, from the top left of `room` and across its width, `gap` apart; the
/// index of the one pressed.
pub fn show_lines(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    buttons: Vec<TextButton<'_>>,
    placed: &ButtonLines,
    room: Rect,
    gap: f32,
) -> Option<usize> {
    let (at, width) = (room.min, room.width());
    let side = super::control_side(theme);
    let mut pressed = None;
    let mut line_ui: Option<(usize, Ui)> = None;
    for (index, (button, line)) in buttons.into_iter().zip(placed.lines.iter().copied()).enumerate() {
        if line_ui.as_ref().is_none_or(|(on, _)| *on != line) {
            let top = at.y + line as f32 * (side + gap);
            let place = Rect::from_min_size(pos2(at.x, top), vec2(width.max(1.0), side));
            let mut child = ui.new_child(UiBuilder::new().max_rect(place).layout(Layout::left_to_right(Align::Center)));
            child.spacing_mut().item_spacing.x = gap;
            line_ui = Some((line, child));
        }
        if let Some((_, child)) = line_ui.as_mut()
            && button.show(child, theme, keymap).clicked()
        {
            pressed = Some(index);
        }
    }
    pressed
}

#[cfg(test)]
#[path = "button_row_tests.rs"]
mod tests;
