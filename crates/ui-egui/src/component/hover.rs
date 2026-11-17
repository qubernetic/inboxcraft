// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The hover action group of a row: each action a boxless button, its 14 px icon and its key chip, 24 high, 4 apart,
//! at the right end of the row's first line. It shows for a hovered row and for the row the keyboard is on, at once:
//! the presentation's `.row:hover .acts` has no transition. Its buttons follow the row in the `Tab` order.
//!
//! An action at rest is its `muted` icon and its chip; under the pointer a 1 px `muted` edge and an `ink` icon; with
//! the keyboard focus the edge is `cyan_edge`. Held down (the sheet draws no pressed look) it takes the secondary
//! button's press: the `stage` fill and an `ink` edge. An action that cannot run is its icon in `dim` and its chip
//! dashed, its tooltip saying why. The group paints no ground of its own: the row's fill shows through, and the row
//! cuts its text short of the group.

use egui::{Id, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::key::{Chip, ChipState};
use super::{Hover, lit, soon, tooltip};

/// One action of the group.
#[derive(Debug, Clone, Copy)]
pub struct Action<'a> {
    /// Its icon.
    pub icon: IconId,
    /// Its name: the button's accessible name and tooltip.
    pub name: &'a str,
    /// The action it runs, whose key its chip and tooltip show.
    pub action: Option<&'a str>,
    /// Why it cannot run yet: it shows its icon in `dim` and its chip dashed, and its tooltip says so; `None` for one
    /// that runs.
    pub unavailable: Option<&'a str>,
}

/// What the group drew.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shown {
    /// The action pressed, by its place in the list.
    pub clicked: Option<usize>,
    /// The group's left edge, or the line's right edge when hidden.
    pub left: f32,
    /// Whether the group shows: 1 shown, 0 hidden (it comes and goes at once).
    pub openness: f32,
}

/// The most actions one group draws.
const MOST: usize = 16;

/// The room left and right inside an action, and between its icon and its chip, in points.
const PAD: f32 = 4.0;

/// One action laid out: its chip, if its action has a key, and its width.
struct Laid {
    chip: Option<Chip>,
    width: f32,
}

/// The actions of `list` laid out in `ui`: each one's chip and width.
fn lay(ui: &Ui, theme: &Theme, keymap: &Keymap, list: &[Action<'_>], soon: bool) -> Vec<Laid> {
    let icon = theme.mark_side();
    list.iter()
        .take(MOST)
        .map(|action| {
            let state = if soon || action.unavailable.is_some() { ChipState::Disabled } else { ChipState::Default };
            let chip = action.action.and_then(|id| keymap.chip(ui.ctx(), id)).map(|key| Chip::new(ui, theme, &key, state));
            let width = PAD + icon + chip.as_ref().map_or(0.0, |chip| PAD + chip.size().x) + PAD;
            Laid { chip, width }
        })
        .collect()
}

/// The width of the group of `list` in `ui`: what a row leaves free for it at the right end of its line.
#[must_use]
pub fn width(ui: &Ui, theme: &Theme, keymap: &Keymap, list: &[Action<'_>]) -> f32 {
    let laid = lay(ui, theme, keymap, list, soon::is_soon(ui));
    let gaps = laid.len().saturating_sub(1) as f32 * theme.spacing.s1;
    laid.iter().map(|action| action.width).sum::<f32>() + gaps
}

/// The group `id` of `list` at the right end of `line` (the row's first line: the group's right edge on the line's,
/// centred on it), when `shown`.
pub fn actions(ui: &mut Ui, theme: &Theme, keymap: &Keymap, id: Id, line: Rect, shown: bool, list: &[Action<'_>]) -> Shown {
    actions_with(ui, theme, keymap, id, line, shown, list, None)
}

/// [`actions`], with the action at a place drawn in a state whatever the pointer and the keyboard do (the gallery's
/// board).
#[expect(clippy::too_many_arguments, reason = "the group's arguments and the state a board forces")]
pub(crate) fn actions_with(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    id: Id,
    line: Rect,
    shown: bool,
    list: &[Action<'_>],
    forced: Option<(usize, super::Shown)>,
) -> Shown {
    if !shown || list.is_empty() {
        return Shown { clicked: None, left: line.right(), openness: 0.0 };
    }
    let p = &theme.palette;
    let soon = soon::is_soon(ui);
    let laid = lay(ui, theme, keymap, list, soon);
    let gap = theme.spacing.s1;
    let total = laid.iter().map(|action| action.width).sum::<f32>() + laid.len().saturating_sub(1) as f32 * gap;
    let high = theme.boxes().strip;
    let icon = theme.mark_side();
    let top = (line.center().y - high / 2.0).round();
    let mut x = (line.right() - total).round();
    let left = x;
    let mut clicked = None;
    for (index, (action, laid)) in list.iter().zip(laid).enumerate() {
        let rect = Rect::from_min_size(pos2(x, top), vec2(laid.width, high));
        x += laid.width + gap;
        let enabled = !soon && action.unavailable.is_none() && ui.is_enabled();
        if enabled && let Some(id) = action.action {
            soon::offer(ui, id);
        }
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let response = ui.interact(rect, id.with(index), sense);
        response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, action.name));
        let state = forced.filter(|(at, _)| *at == index).map(|(_, state)| state);
        let lit = lit(ui, theme, &response, enabled, state, Hover::AtOnce);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let (fill, edge, glyph) = if !enabled {
                (None, None, p.dim)
            } else if lit.pressed {
                (Some(p.stage), Some(p.ink), p.ink)
            } else if lit.focus {
                (None, Some(p.cyan_edge), p.ink)
            } else if lit.hover {
                (None, Some(p.muted), p.ink)
            } else {
                (None, None, p.muted)
            };
            if let Some(fill) = fill {
                painter.rect_filled(rect, 0.0, fill);
            }
            if let Some(edge) = edge {
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            }
            let at = Rect::from_min_size(pos2(rect.left() + PAD, rect.center().y - icon / 2.0), vec2(icon, icon));
            crate::iconset::paint(painter, at, action.icon, glyph);
            if let Some(chip) = laid.chip {
                let chip_top = (rect.center().y - chip.size().y / 2.0).round();
                chip.paint(painter, pos2(at.right() + PAD, chip_top));
            }
        }
        // Its key shows in the tooltip only when pressing it would run the action.
        let response: Response = tooltip::attach_note(response, theme, keymap, action.name, action.action.filter(|_| enabled), action.unavailable);
        if response.clicked() {
            clicked = Some(index);
        }
    }
    Shown { clicked, left, openness: 1.0 }
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod tests;
