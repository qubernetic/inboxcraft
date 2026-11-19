// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's own controls at the right end of the command line: minimise, maximise and close, three cells 36 wide
//! and 32 high side by side, each a 16 px Phosphor glyph in `ink` in its middle, on the command line's panel. Hovered,
//! a cell fills with `grid`; close fills with `minus` and its glyph turns `bg`. A maximised window shows restore (two
//! squares) in the middle. With another window in front the glyphs are `muted`, never dim: they still work.
//!
//! Each cell is a button: Tab and Shift Tab reach them in the drawn order, Enter and Space press them, the focus ring
//! stands inside the cell. The set returns which was pressed; it minimises, maximises and closes nothing itself.

use egui::{Id, Rect, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use super::Shown;
use crate::draw::paint::{Ring, focus_ring};
use crate::theme::Theme;
use crate::tr;

/// A cell's width, in points; it stays at every text size.
pub const CELL: f32 = 36.0;

/// The glyphs: Phosphor's minus, square, copy (restore) and x.
const MINUS: char = '\u{e32a}';
const SQUARE: char = '\u{e45e}';
const RESTORE: char = '\u{e1ca}';
const CROSS: char = '\u{e4f6}';

/// One of the three controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Minimise the window.
    Minimise,
    /// Maximise the window, or restore it when it is maximised.
    Maximise,
    /// Close the window.
    Close,
}

impl Control {
    /// The three, in the drawn order.
    pub const ALL: [Self; 3] = [Self::Minimise, Self::Maximise, Self::Close];
}

/// How the window stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// It fills the screen: the middle control restores it.
    pub maximised: bool,
    /// It is in front: the glyphs are `ink`, else `muted`.
    pub focused: bool,
}

/// The height of the set at `theme`'s text size: a button's, 32 at 100 %.
#[must_use]
pub fn height(theme: &Theme) -> f32 {
    theme.boxes().button
}

/// What a screen reader hears of `control` for a window that is `maximised` or not.
#[must_use]
pub fn spoken(control: Control, maximised: bool) -> &'static str {
    match control {
        Control::Minimise => tr!("Minimise"),
        Control::Maximise if maximised => tr!("Restore"),
        Control::Maximise => tr!("Maximise"),
        Control::Close => tr!("Close the window"),
    }
}

/// The three controls for `window`, under `id`, `forced` drawing one of them in a state whatever the pointer and the
/// keyboard do (a board). Which was pressed, by the pointer or by Enter or Space.
pub fn window_controls(ui: &mut Ui, theme: &Theme, id: Id, window: Window, forced: Option<(Control, Shown)>) -> Option<Control> {
    drawn(ui, theme, id, window, forced, true)
}

/// The three controls for `window` under `id`, Tab stops only while `keyboard` (a surface in front that keeps the
/// keyboard leaves them to the pointer). Which was pressed.
pub fn window_controls_keyed(ui: &mut Ui, theme: &Theme, id: Id, window: Window, keyboard: bool) -> Option<Control> {
    drawn(ui, theme, id, window, None, keyboard)
}

fn drawn(ui: &mut Ui, theme: &Theme, id: Id, window: Window, forced: Option<(Control, Shown)>, keyboard: bool) -> Option<Control> {
    let p = &theme.palette;
    let high = height(theme);
    let (whole, _) = ui.allocate_exact_size(vec2(CELL * 3.0, high), Sense::hover());
    let side = theme.icon_side();
    let mut pressed = None;
    for (at, control) in (0_u8..).zip(Control::ALL) {
        let cell = Rect::from_min_size(pos2(whole.left() + CELL * f32::from(at), whole.top()), vec2(CELL, high));
        let response = ui.interact(cell, id.with(at), if keyboard { Sense::click() } else { Sense::CLICK });
        let words = spoken(control, window.maximised);
        response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), words));
        if response.clicked() {
            pressed = Some(control);
        }
        let shown = forced.filter(|(which, _)| *which == control).map(|(_, shown)| shown);
        let lit = super::lit(ui, theme, &response, ui.is_enabled(), shown.or(forced.map(|_| Shown::Rest)), super::Hover::AtOnce);
        if !ui.is_rect_visible(cell) {
            continue;
        }
        let painter = super::solid(ui);
        let close = control == Control::Close;
        let (fill, ink) = match (lit.hover, close) {
            (true, true) => (Some(p.minus), p.bg),
            (true, false) => (Some(p.grid), p.ink),
            (false, _) => (None, if window.focused { p.ink } else { p.muted }),
        };
        if let Some(fill) = fill {
            painter.rect_filled(cell, 0.0, fill);
        }
        let glyph = match control {
            Control::Minimise => MINUS,
            Control::Maximise if window.maximised => RESTORE,
            Control::Maximise => SQUARE,
            Control::Close => CROSS,
        };
        let icon = Rect::from_center_size(cell.center(), vec2(side, side));
        crate::iconset::paint_glyph(&painter, icon, glyph, ink);
        if lit.focus {
            focus_ring(&painter, theme, cell, Ring::Inside);
        }
    }
    pressed
}

#[cfg(test)]
#[path = "window_controls_tests.rs"]
mod tests;
