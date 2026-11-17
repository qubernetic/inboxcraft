// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The buttons of the design, each drawn as its sheet says in every state, square and at full strength:
//!
//! - [`IconButton`]: a 16 px glyph in a 32 × 32 square, no box. `muted` at rest; hovered the `stage` fill and an `ink`
//!   glyph; pressed a `muted` edge as well; selected (pinned, flagged) the accent wash and the accent glyph; disabled
//!   or unavailable only the glyph in `dim`, the tooltip saying why. On the navigation rail it is drawn in one ink.
//! - [`TextButton`]: words in mono 13, 32 high, 12 beside the text, in one of four [`Tone`]s.
//! - [`link`] and [`link_sized`]: the accent as text, underlined under the pointer.
//!
//! Disabled is never a fade: a box gets a dashed `line` edge and `muted` text, a glyph `dim`.

use egui::{Color32, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::draw::paint::{self, Ring};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::Theme;

use super::{Shown, control_side, lit, soon, tooltip};

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
    shown: Option<Shown>,
}

impl<'a> IconButton<'a> {
    /// A button showing `icon`, called `name` (its accessible name and the text of its tooltip).
    #[must_use]
    pub fn new(icon: IconId, name: &'a str) -> Self {
        Self { icon, name, action: None, active: false, enabled: true, note: None, ink: None, side: None, shown: None }
    }

    /// The action it runs, whose key its tooltip shows.
    #[must_use]
    pub fn action(mut self, action: &'a str) -> Self {
        self.action = Some(action);
        self
    }

    /// Shown as selected (pinned, flagged): the accent wash and the accent glyph.
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

    /// Drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
    #[must_use]
    pub fn shown(mut self, shown: Shown) -> Self {
        self.shown = Some(shown);
        self
    }

    /// Draws it into `ui`; its response is clicked when it was pressed.
    pub fn show(self, ui: &mut Ui, theme: &Theme, keymap: &Keymap) -> Response {
        let p = &theme.palette;
        let side = self.side.filter(|side| side.is_finite() && *side >= 1.0).unwrap_or_else(|| control_side(theme)).min(512.0);
        // Coming soon, it is drawn and explains itself like a disabled one.
        let soon = soon::is_soon(ui);
        let enabled = self.enabled && !soon && ui.is_enabled();
        if let Some(action) = self.action {
            soon::offer(ui, action);
        }
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(side, side), sense);
        response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, self.active, self.name));
        let lit = lit(ui, theme, &response, enabled, self.shown);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            // The rail's buttons (drawn in its ink) stand on the `panel`, whose hover is `grid`; every other on a
            // `surface`, whose hover is `stage`.
            let hover = if self.ink.is_some() { p.grid } else { p.stage };
            if self.active && enabled {
                painter.rect_filled(rect, 0.0, p.cyan_muted);
            } else if lit.pressed {
                painter.rect_filled(rect, 0.0, hover);
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, p.muted), StrokeKind::Inside);
            } else if lit.hover {
                painter.rect_filled(rect, 0.0, hover);
            }
            let colour = match (self.ink, enabled, self.active, lit.hover || lit.pressed) {
                (_, false, _, _) => p.dim,
                (Some(ink), true, _, _) => ink,
                (None, true, true, _) => p.cyan_text,
                (None, true, false, true) => p.ink,
                (None, true, false, false) => p.muted,
            };
            let icon = if self.side.is_some() { side * 0.5 } else { theme.icon_side() };
            crate::iconset::paint(painter, egui::Rect::from_center_size(rect.center(), vec2(icon, icon)), self.icon, colour);
            if lit.focus {
                paint::focus_ring(painter, theme, rect, Ring::Outside);
            }
        }
        // Coming soon, its key would do nothing: the tooltip shows none.
        tooltip::attach_note(response, theme, keymap, self.name, self.action.filter(|_| !soon), self.note)
    }
}

/// How far a disabled text button is faded.
const DISABLED: f32 = 0.45;

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
        let soon = soon::is_soon(ui);
        let enabled = self.note.is_none() && !soon;
        if let Some(action) = self.action {
            soon::offer(ui, action);
        }
        let ink = match self.tone {
            _ if soon => p.muted,
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
            // Coming soon is drawn faint by colour, never faded.
            faded.multiply_opacity(if enabled || soon { 1.0 } else { DISABLED });
            let painter = &faded;
            let round = 0.0;
            match self.tone {
                // Coming soon: no fill, a dashed edge round every button that has a box.
                Tone::Primary | Tone::Danger | Tone::Secondary if soon => paint::dashed(painter, rect, p.muted),
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
        // Coming soon, its key would do nothing: the tooltip shows none.
        tooltip::attach_note(response, theme, keymap, self.name, self.action.filter(|_| !soon), self.note)
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
    let soon = soon::is_soon(ui);
    let ink = soon::ink(ui, theme, p.cyan_text);
    let galley = ui.painter().layout_no_wrap(name.to_owned(), theme.fonts.button.id(), ink);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(2.0 * LINK_PAD, 2.0 * s.s1), soon::sense(ui, Sense::click()));
    response.widget_info(|| WidgetInfo::labeled(Role::Button, !soon, name));
    if ui.is_rect_visible(rect) {
        if response.hovered() && !soon {
            ui.painter().rect_filled(rect, 0.0, p.stage);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, p.cyan_edge), StrokeKind::Outside);
        }
        ui.painter().galley(rect.min + vec2(LINK_PAD, s.s1), galley, ink);
    }
    tooltip::attach(response, theme, keymap, name, None)
}
