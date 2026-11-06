// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The hover action group: icon buttons on a float at the right end of a row, fading in over the hover token while
//! the row is hovered or selected by keyboard, and out again after. The row's text fades out before the group
//! ([`crate::glass::faded`] up to [`Shown::left`]), so no letter shows through it or touches its edge.

use egui::{Align, Id, Layout, Rect, Shape, Ui, UiBuilder, vec2};

use crate::glass::{self, Under};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::Theme;

use super::button::IconButton;
use super::control_side;

/// One action of the group.
#[derive(Debug, Clone, Copy)]
pub struct Action<'a> {
    /// Its icon.
    pub icon: IconId,
    /// Its name: the button's accessible name and tooltip.
    pub name: &'a str,
    /// The action it runs, whose key the tooltip shows.
    pub action: Option<&'a str>,
    /// Why it cannot run yet: it shows faded and its tooltip says so; `None` for one that runs.
    pub unavailable: Option<&'a str>,
}

/// What the group drew.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shown {
    /// The action pressed, by its place in the list.
    pub clicked: Option<usize>,
    /// Where the row's text must have faded out by: the group's left edge, or the row's right edge when hidden.
    pub left: f32,
    /// How far the group has faded in, 0 to 1.
    pub openness: f32,
}

/// The rectangle of a group of `count` buttons at the right end of `row`.
fn place(ui: &Ui, theme: &Theme, row: Rect, count: usize) -> Rect {
    let s = &theme.spacing;
    let side = control_side(ui, theme);
    let count = u16::try_from(count.min(16)).unwrap_or(16);
    let width = f32::from(count) * side + f32::from(count.saturating_sub(1)) * s.xs + s.xs * 2.0;
    let height = side + s.xs * 2.0;
    Rect::from_min_size(egui::pos2(row.right() - s.s - width, row.center().y - height / 2.0), vec2(width, height))
}

/// The group `id` of `actions` over the right end of `row`, shown or hidden.
pub fn actions(ui: &mut Ui, theme: &Theme, keymap: &Keymap, id: Id, row: Rect, shown: bool, actions: &[Action<'_>]) -> Shown {
    let openness = motion::toggle(ui.ctx(), id.with("shown"), shown, theme.motion.hover_fade);
    if openness <= 0.0 || actions.is_empty() {
        return Shown { clicked: None, left: row.right(), openness: 0.0 };
    }
    let group = place(ui, theme, row, actions.len());
    let mut child = ui.new_child(UiBuilder::new().id_salt(id).max_rect(group).layout(Layout::left_to_right(Align::Center)));
    child.multiply_opacity(openness);
    let slot = child.painter().add(Shape::Noop);
    let shapes = glass::float_shapes(child.painter(), theme, group, Under::Fading);
    child.painter().set(slot, Shape::Vec(shapes));
    child.add_space(theme.spacing.xs);
    child.spacing_mut().item_spacing.x = theme.spacing.xs;
    let mut clicked = None;
    for (index, action) in actions.iter().take(16).enumerate() {
        let mut button = IconButton::new(action.icon, action.name);
        if let Some(id) = action.action {
            button = button.action(id);
        }
        if let Some(note) = action.unavailable {
            button = button.unavailable(note);
        }
        if button.show(&mut child, theme, keymap).clicked() {
            clicked = Some(index);
        }
    }
    Shown { clicked, left: group.left(), openness }
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod tests;
