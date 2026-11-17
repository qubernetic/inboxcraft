// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The icon button: an icon in a square as tall as a line of button text with a little room, a flat hover fill, and
//! always a tooltip with its name and key. Active (a pinned pin) it shows its icon in the accent on the
//! `stage`; disabled (an action without a command yet) its glyph is `dim`, it takes no click and still explains itself
//! in its tooltip, with a note that says why when it is unavailable. On the navigation rail it is drawn in one ink.
//! A text button that is unavailable fades and says why too. Everything is square.

use egui::{Color32, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::iconset::IconId;
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
    note: Option<&'a str>,
    ink: Option<Color32>,
    side: Option<f32>,
}

/// How far a disabled button is faded.
pub(crate) const DISABLED: f32 = 0.45;

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

    /// Its icon drawn in `ink` (the navigation rail's).
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
        let side = self.side.filter(|side| side.is_finite() && *side >= 1.0).unwrap_or_else(|| control_side(theme)).min(512.0);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(side, side), sense);
        response.widget_info(|| WidgetInfo::selected(Role::Button, self.enabled, self.active, self.name));
        let lit = self.enabled && (response.hovered() || response.has_focus());
        let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            if self.active {
                painter.rect_filled(rect, 0.0, p.stage);
            } else if hover > 0.0 {
                // The rail's buttons (drawn in its ink) stand on the `panel`; every other on a `surface`.
                painter.rect_filled(rect, 0.0, if self.ink.is_some() { p.grid } else { p.stage });
            }
            let colour = match (self.ink, self.active) {
                (Some(ink), _) => ink,
                (None, true) => p.cyan_text,
                (None, false) => p.ink,
            };
            let colour = if self.enabled { colour } else { p.dim };
            let icon = if self.side.is_some() { side * 0.5 } else { theme.icon_side() };
            crate::iconset::paint(painter, egui::Rect::from_center_size(rect.center(), vec2(icon, icon)), self.icon, colour);
            if response.has_focus() {
                painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(1.0, p.cyan_edge), StrokeKind::Inside);
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
    /// `on_cyan` on the `cyan` fill: the one thing to do.
    Primary,
    /// On the error colour: a hard step, such as blocking a sender.
    Danger,
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

    /// The primary button: on the cyan fill.
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

    /// The red button of a hard step: filled with the error colour.
    #[must_use]
    pub fn danger(mut self) -> Self {
        self.tone = Tone::Danger;
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
        let ink = match self.tone {
            Tone::Primary => p.on_cyan,
            Tone::Danger => p.bg,
            Tone::Quiet | Tone::Secondary => p.ink,
        };
        let galley = super::galley(ui, self.name, theme.fonts.button, ink);
        let height = control_side(theme);
        let icon_side = theme.fonts.button.size * 1.2;
        let lead = self.icon.map_or(0.0, |_| icon_side + s.s2);
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + lead + s.s3 * 2.0, height), sense);
        response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, self.name));
        let lit = enabled && (response.hovered() || response.has_focus());
        let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
        if ui.is_rect_visible(rect) {
            let mut faded = ui.painter().clone();
            faded.multiply_opacity(if enabled { 1.0 } else { DISABLED });
            let painter = &faded;
            let round = 0.0;
            match self.tone {
                Tone::Primary => {
                    painter.rect_filled(rect, round, p.cyan);
                }
                Tone::Danger => {
                    painter.rect_filled(rect, round, p.minus);
                }
                Tone::Secondary => {
                    painter.rect_stroke(rect.shrink(0.5), round, Stroke::new(1.0, p.line), StrokeKind::Inside);
                }
                Tone::Quiet => {}
            }
            if hover > 0.0 && matches!(self.tone, Tone::Quiet | Tone::Secondary) {
                painter.rect_filled(rect, round, p.stage);
            }
            let left = rect.center().x - (galley.size().x + lead) / 2.0;
            if let Some(icon) = self.icon {
                let at = egui::Rect::from_center_size(egui::pos2(left + icon_side / 2.0, rect.center().y), vec2(icon_side, icon_side));
                crate::iconset::paint(painter, at, icon, ink);
            }
            painter.galley(egui::pos2(left + lead, rect.center().y - galley.size().y / 2.0), galley, ink);
            if response.has_focus() {
                painter.rect_stroke(rect.shrink(1.0), round, Stroke::new(1.0, p.cyan_edge), StrokeKind::Inside);
            }
        }
        tooltip::attach_note(response, theme, keymap, self.name, self.action, self.note)
    }
}

/// A text button called `name` that runs `action`, whose key its tooltip shows: a quiet one with a hover fill, or the
/// `primary` one on the cyan fill.
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
    let galley = ui.painter().layout_no_wrap(name.to_owned(), theme.fonts.button.id(), p.cyan_text);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(2.0 * LINK_PAD, 2.0 * s.s1), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, p.stage);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, p.cyan_edge), StrokeKind::Outside);
        }
        ui.painter().galley(rect.min + vec2(LINK_PAD, s.s1), galley, p.cyan_text);
    }
    tooltip::attach(response, theme, keymap, name, None)
}
