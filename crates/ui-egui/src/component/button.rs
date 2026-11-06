// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The icon button: an icon in a square as tall as a line of button text with a little room, its hover fill fading
//! in over the hover token, and always a tooltip with its name and key. Active (a pinned pin) it shows the Fill
//! style in the accent on the selection; disabled (an action without a command yet) it is muted, takes no click and
//! still explains itself in its tooltip.

use egui::{Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::Theme;

use super::{control_side, tooltip};

/// An icon button.
#[derive(Debug, Clone, Copy)]
pub struct IconButton<'a> {
    icon: IconId,
    name: &'a str,
    action: Option<&'a str>,
    active: bool,
    enabled: bool,
}

/// How far a disabled button is faded.
const DISABLED: f32 = 0.45;

impl<'a> IconButton<'a> {
    /// A button showing `icon`, called `name` (its accessible name and the text of its tooltip).
    #[must_use]
    pub fn new(icon: IconId, name: &'a str) -> Self {
        Self { icon, name, action: None, active: false, enabled: true }
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
            if self.active {
                painter.rect_filled(rect, s.corner, p.selection);
            }
            if hover > 0.0 {
                painter.rect_filled(rect, s.corner, p.hover.gamma_multiply(hover));
            }
            let (style, colour) = if self.active { (Style::Fill, p.accent) } else { (Style::Regular, p.text) };
            let colour = if self.enabled { colour } else { p.muted.gamma_multiply(DISABLED) };
            let icon = theme.fonts.body.size * 1.3;
            theme.icons.paint(painter, egui::Rect::from_center_size(rect.center(), vec2(icon, icon)), self.icon, style, colour);
            if response.has_focus() {
                painter.rect_stroke(rect.shrink(1.0), s.corner, Stroke::new(2.0, p.focus), StrokeKind::Inside);
            }
        }
        tooltip::attach(response, theme, keymap, self.name, self.action)
    }
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod tests;
