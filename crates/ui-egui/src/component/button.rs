// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The icon button: an icon in a square as tall as a line of button text with a little room, its hover fill fading
//! in over the hover token, and always a tooltip with its name and key. Active (a pinned pin) it shows the Fill
//! style in the accent on the selection; disabled (an action without a command yet) it is muted, takes no click and
//! still explains itself in its tooltip, with a note that says why when it is unavailable. On a gradient (the
//! navigation rail) it is drawn in one ink. A text button that is unavailable fades the same way and says why too.

use egui::{Color32, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::gradient::{self, Gradient, Stop};
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::{Theme, text_gradient};

use super::{control_side, tooltip};

/// An icon button.
#[derive(Debug, Clone, Copy)]
pub struct IconButton<'a> {
    icon: IconId,
    name: &'a str,
    action: Option<&'a str>,
    active: bool,
    enabled: bool,
    note: Option<&'a str>,
    ink: Option<Color32>,
    side: Option<f32>,
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
        Self { icon, name, action: None, active: false, enabled: true, note: None, ink: None, side: None }
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

    /// Disabled, its tooltip saying why under its name: `note`.
    #[must_use]
    pub fn unavailable(mut self, note: &'a str) -> Self {
        self.enabled = false;
        self.note = Some(note);
        self
    }

    /// Drawn in `ink` on a gradient (the navigation rail): the icon, its focus ring and, at lower strength, its
    /// active and hover fills.
    #[must_use]
    pub fn ink(mut self, ink: Color32) -> Self {
        self.ink = Some(ink);
        self
    }

    /// A square of `side` points (the rail's larger tiles) instead of a control's side, the icon half as wide.
    #[must_use]
    pub fn side(mut self, side: f32) -> Self {
        self.side = Some(side);
        self
    }

    /// Draws it into `ui`; its response is clicked when it was pressed.
    pub fn show(self, ui: &mut Ui, theme: &Theme, keymap: &Keymap) -> Response {
        let p = &theme.palette;
        let s = &theme.spacing;
        let side = self.side.filter(|side| side.is_finite() && *side >= 1.0).unwrap_or_else(|| control_side(ui, theme)).min(512.0);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(side, side), sense);
        response.widget_info(|| WidgetInfo::selected(Role::Button, self.enabled, self.active, self.name));
        let lit = self.enabled && (response.hovered() || response.has_focus());
        let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let corner = if self.side.is_some() { side * 0.25 } else { s.corner };
            let (selection, lit, focus) = match self.ink {
                Some(ink) => (ink.gamma_multiply(ON_INK_ACTIVE), ink.gamma_multiply(ON_INK_HOVER), ink),
                None => (p.selection, p.hover, p.focus),
            };
            if self.active {
                painter.rect_filled(rect, corner, selection);
            }
            if hover > 0.0 {
                painter.rect_filled(rect, corner, lit.gamma_multiply(hover));
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
            let icon = if self.side.is_some() { side * 0.5 } else { theme.fonts.body.size * 1.3 };
            theme.icons.paint(painter, egui::Rect::from_center_size(rect.center(), vec2(icon, icon)), self.icon, style, colour);
            if response.has_focus() {
                painter.rect_stroke(rect.shrink(1.0), corner, Stroke::new(2.0, focus), StrokeKind::Inside);
            }
        }
        tooltip::attach_note(response, theme, keymap, self.name, self.action, self.note)
    }
}

/// How a text button stands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Text only, with a hover fill (Done all, Clear).
    Quiet,
    /// Outlined in the field border (Reply all, Cancel).
    Secondary,
    /// On the preset's text gradient, in white or the deep ink ([`text_gradient`]): the one thing to do.
    Primary,
}

/// A text button: its name, an optional icon before it, and its tone.
#[derive(Debug, Clone, Copy)]
pub struct TextButton<'a> {
    name: &'a str,
    icon: Option<IconId>,
    tone: Tone,
    action: Option<&'a str>,
    note: Option<&'a str>,
}

impl<'a> TextButton<'a> {
    /// A quiet button called `name`.
    #[must_use]
    pub fn new(name: &'a str) -> Self {
        Self { name, icon: None, tone: Tone::Quiet, action: None, note: None }
    }

    /// The primary button: on the text gradient.
    #[must_use]
    pub fn primary(mut self) -> Self {
        self.tone = Tone::Primary;
        self
    }

    /// A secondary button: outlined.
    #[must_use]
    pub fn secondary(mut self) -> Self {
        self.tone = Tone::Secondary;
        self
    }

    /// `icon` before the name, in the Regular style.
    #[must_use]
    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The action it runs, whose key its tooltip shows.
    #[must_use]
    pub fn action(mut self, action: &'a str) -> Self {
        self.action = Some(action);
        self
    }

    /// Cannot be pressed yet: faded, its tooltip saying why (`note`).
    #[must_use]
    pub fn unavailable(mut self, note: &'a str) -> Self {
        self.note = Some(note);
        self
    }

    /// Draws it into `ui`; its response is clicked when it was pressed.
    pub fn show(self, ui: &mut Ui, theme: &Theme, keymap: &Keymap) -> Response {
        let p = &theme.palette;
        let s = &theme.spacing;
        let enabled = self.note.is_none();
        let (stops, on_gradient) = text_gradient(p.gradient);
        let ink = match self.tone {
            Tone::Primary => on_gradient,
            Tone::Quiet | Tone::Secondary => p.text,
        };
        let galley = super::galley(ui, self.name, theme.fonts.button, ink);
        let height = control_side(ui, theme);
        let icon_side = theme.fonts.button.size * 1.2;
        let lead = self.icon.map_or(0.0, |_| icon_side + s.s);
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + lead + s.m * 2.0, height), sense);
        response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, self.name));
        let lit = enabled && (response.hovered() || response.has_focus());
        let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
        if ui.is_rect_visible(rect) {
            let mut faded = ui.painter().clone();
            faded.multiply_opacity(if enabled { 1.0 } else { DISABLED });
            let painter = &faded;
            let round = rect.height() / 2.0;
            match self.tone {
                Tone::Primary => {
                    let [first, middle, last] = stops;
                    let fill = Gradient::Linear {
                        from: egui::pos2(0.0, 0.5),
                        to: egui::pos2(1.0, 0.5),
                        stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
                    };
                    gradient::fill(painter, rect, round, &fill);
                }
                Tone::Secondary => {
                    painter.rect_stroke(rect.shrink(0.5), round, Stroke::new(1.0, p.field_border), StrokeKind::Inside);
                }
                Tone::Quiet => {}
            }
            if hover > 0.0 {
                painter.rect_filled(rect, round, p.hover.gamma_multiply(hover));
            }
            let left = rect.center().x - (galley.size().x + lead) / 2.0;
            if let Some(icon) = self.icon {
                let at = egui::Rect::from_center_size(egui::pos2(left + icon_side / 2.0, rect.center().y), vec2(icon_side, icon_side));
                theme.icons.paint(painter, at, icon, Style::Regular, ink);
            }
            painter.galley(egui::pos2(left + lead, rect.center().y - galley.size().y / 2.0), galley, ink);
            if response.has_focus() {
                painter.rect_stroke(rect.shrink(1.0), round, Stroke::new(2.0, p.focus), StrokeKind::Inside);
            }
        }
        tooltip::attach_note(response, theme, keymap, self.name, self.action, self.note)
    }
}

/// A text button called `name` that runs `action`, whose key its tooltip shows: a quiet one with a hover fill, or the
/// `primary` one on the preset's text gradient.
pub fn text(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, primary: bool) -> Response {
    let button = TextButton::new(name);
    let button = if primary { button.primary() } else { button };
    match action {
        Some(action) => button.action(action),
        None => button,
    }
    .show(ui, theme, keymap)
}

/// A text button like [`text`] that cannot be pressed yet: faded, and its tooltip says why (`note`).
pub fn unavailable(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, primary: bool, note: &str) -> Response {
    let button = TextButton::new(name).unavailable(note);
    if primary { button.primary() } else { button }.show(ui, theme, keymap)
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod tests;

/// The room a [`link`] leaves left and right of its text, in points.
pub const LINK_PAD: f32 = 4.0;

/// A link called `name`: the text in the accent colour on nothing, a hover fill under the pointer and a ring with the
/// keyboard focus, with [`LINK_PAD`] points of room beside the text.
pub fn link(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str) -> Response {
    let s = &theme.spacing;
    let p = &theme.palette;
    let galley = ui.painter().layout_no_wrap(name.to_owned(), theme.fonts.button.id(), p.accent);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(2.0 * LINK_PAD, 2.0 * s.xs), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, s.corner, p.hover);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, s.corner, Stroke::new(2.0, p.focus), StrokeKind::Outside);
        }
        ui.painter().galley(rect.min + vec2(LINK_PAD, s.xs), galley, p.accent);
    }
    tooltip::attach(response, theme, keymap, name, None)
}
