// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The icon button: an icon in a square as tall as a line of button text with a little room, its hover fill fading
//! in over the hover token, and always a tooltip with its name and key. Active (a pinned pin) it shows the Fill
//! style in the accent on the selection; disabled (an action without a command yet) it is muted, takes no click and
//! still explains itself in its tooltip. On a gradient (the navigation rail) it is drawn in one ink.

use egui::{Color32, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::gradient::{self, Gradient, Stop};
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::{Theme, ink_over};

use super::{control_side, tooltip};

/// An icon button.
#[derive(Debug, Clone, Copy)]
pub struct IconButton<'a> {
    icon: IconId,
    name: &'a str,
    action: Option<&'a str>,
    active: bool,
    enabled: bool,
    ink: Option<Color32>,
}

/// How far a disabled button is faded.
const DISABLED: f32 = 0.45;

/// How strong the ink is in the fill of an active button on a gradient.
const ON_INK_ACTIVE: f32 = 0.24;

/// How strong the ink is in the hover fill of a button on a gradient.
const ON_INK_HOVER: f32 = 0.14;

impl<'a> IconButton<'a> {
    /// A button showing `icon`, called `name` (its accessible name and the text of its tooltip).
    #[must_use]
    pub fn new(icon: IconId, name: &'a str) -> Self {
        Self { icon, name, action: None, active: false, enabled: true, ink: None }
    }

    /// The action it runs, whose key its tooltip shows.
    #[must_use]
    pub fn action(mut self, action: &'a str) -> Self {
        self.action = Some(action);
        self
    }

    /// Shown as active (pinned, flagged).
    #[must_use]
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Whether it can be pressed.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Drawn in `ink` on a gradient (the navigation rail): the icon, its focus ring and, at lower strength, its
    /// active and hover fills.
    #[must_use]
    pub fn ink(mut self, ink: Color32) -> Self {
        self.ink = Some(ink);
        self
    }

    /// Draws it into `ui`; its response is clicked when it was pressed.
    pub fn show(self, ui: &mut Ui, theme: &Theme, keymap: &Keymap) -> Response {
        let p = &theme.palette;
        let s = &theme.spacing;
        let side = control_side(ui, theme);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(side, side), sense);
        response.widget_info(|| WidgetInfo::selected(Role::Button, self.enabled, self.active, self.name));
        let lit = self.enabled && (response.hovered() || response.has_focus());
        let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let (selection, lit, focus) = match self.ink {
                Some(ink) => (ink.gamma_multiply(ON_INK_ACTIVE), ink.gamma_multiply(ON_INK_HOVER), ink),
                None => (p.selection, p.hover, p.focus),
            };
            if self.active {
                painter.rect_filled(rect, s.corner, selection);
            }
            if hover > 0.0 {
                painter.rect_filled(rect, s.corner, lit.gamma_multiply(hover));
            }
            let (style, colour) = match (self.ink, self.active) {
                (Some(ink), active) => (if active { Style::Fill } else { Style::Regular }, ink),
                (None, true) => (Style::Fill, p.accent),
                (None, false) => (Style::Regular, p.text),
            };
            let colour = if self.enabled {
                colour
            } else if let Some(ink) = self.ink {
                ink.gamma_multiply(DISABLED)
            } else {
                p.muted.gamma_multiply(DISABLED)
            };
            let icon = theme.fonts.body.size * 1.3;
            theme.icons.paint(painter, egui::Rect::from_center_size(rect.center(), vec2(icon, icon)), self.icon, style, colour);
            if response.has_focus() {
                painter.rect_stroke(rect.shrink(1.0), s.corner, Stroke::new(2.0, focus), StrokeKind::Inside);
            }
        }
        tooltip::attach(response, theme, keymap, self.name, self.action)
    }
}

/// A text button called `name` that runs `action`, whose key its tooltip shows: a quiet one with a hover fill, or the
/// `primary` one on the preset's gradient with white or deep ink, whichever reads on every stop.
pub fn text(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, primary: bool) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let ink = if primary { ink_over(&p.gradient) } else { p.text };
    let galley = super::galley(ui, name, theme.fonts.button, ink);
    let height = control_side(ui, theme);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + s.m * 2.0, height), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
    let lit = response.hovered() || response.has_focus();
    let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let round = rect.height() / 2.0;
        if primary {
            let [first, middle, last] = p.gradient;
            let fill = Gradient::Linear {
                from: egui::pos2(0.0, 0.5),
                to: egui::pos2(1.0, 0.5),
                stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
            };
            gradient::fill(painter, rect, round, &fill);
        }
        if hover > 0.0 {
            painter.rect_filled(rect, round, p.hover.gamma_multiply(hover));
        }
        painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
        if response.has_focus() {
            painter.rect_stroke(rect.shrink(1.0), round, Stroke::new(2.0, p.focus), StrokeKind::Inside);
        }
    }
    tooltip::attach(response, theme, keymap, name, action)
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod tests;
