// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The key chip: a key in mono 11 SemiBold in a 1 px box with a 2 px radius, 20 high, 5 beside the text. Keys are
//! written out as words ([`key_words`]): `Ctrl Enter`, `Ctrl K`, `Del`, `Esc`; the arrow keys are the signs ↓ ↑ ← →.
//! Keys pressed together share one chip. An action without a key shows no chip.

use egui::{Color32, Key, KeyboardShortcut, Response, Role, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::theme::Theme;

/// How a key chip stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipState {
    /// The `line` edge, the key in `muted`.
    Default,
    /// On a cyan fill (the primary button): it carries that fill itself, with an `on_cyan` edge and text.
    OnCyan,
    /// Waiting for the new keys of a shortcut: the `cyan_edge` and the accent text.
    Capture,
    /// No key, or a key that cannot be used: a dashed `line` edge, the text `muted`.
    Disabled,
}

/// The room left and right of the key, in points.
const PAD: f32 = 5.0;

/// The corner radius of a key chip, in points.
const RADIUS: u8 = 2;

/// A key chip laid out and not yet placed: what a control that holds one (the primary button's) paints itself.
pub(crate) struct Chip {
    text: std::sync::Arc<egui::Galley>,
    state: ChipState,
    fill: Color32,
    edge: Color32,
    ink: Color32,
    size: Vec2,
}

impl Chip {
    /// The chip of `key` in `state` at the theme's chip height, its width on whole points.
    pub(crate) fn new(ui: &Ui, theme: &Theme, key: &str, state: ChipState) -> Self {
        let p = &theme.palette;
        let (fill, edge, ink) = match state {
            ChipState::Default | ChipState::Disabled => (Color32::TRANSPARENT, p.line, p.muted),
            ChipState::OnCyan => (p.cyan, p.on_cyan, p.on_cyan),
            ChipState::Capture => (Color32::TRANSPARENT, p.cyan_edge, p.cyan_text),
        };
        let text = super::galley(ui, key, theme.fonts.key, ink);
        let size = vec2((text.size().x + PAD * 2.0).ceil(), theme.boxes().chip);
        Self { text, state, fill, edge, ink, size }
    }

    /// Its size.
    pub(crate) fn size(&self) -> Vec2 {
        self.size
    }

    /// Paints it with its top left corner at `at`.
    pub(crate) fn paint(self, painter: &egui::Painter, at: egui::Pos2) {
        let rect = egui::Rect::from_min_size(at, self.size);
        if self.state == ChipState::Disabled {
            paint::dashed(painter, rect, self.edge);
        } else {
            painter.rect(rect, RADIUS, self.fill, Stroke::new(1.0, self.edge), StrokeKind::Inside);
        }
        painter.galley(pos2(rect.left() + PAD, rect.center().y - self.text.size().y / 2.0), self.text, self.ink);
    }
}

/// The chip of `key` (already in words, [`key_words`]) in `state`, as high as the theme's chip.
pub fn key_chip(ui: &mut Ui, theme: &Theme, key: &str, state: ChipState) -> Response {
    let chip = Chip::new(ui, theme, key, state);
    let (rect, response) = ui.allocate_exact_size(chip.size(), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, ui.is_enabled(), key));
    if ui.is_rect_visible(rect) {
        chip.paint(ui.painter(), rect.min);
    }
    response
}

/// `shortcut` written out as a chip shows it: the modifiers as words in the order Ctrl, Alt, Shift (the command key
/// reads `Ctrl` on Linux and Windows), then the key: the arrows as `↓ ↑ ← →`, `Del`, `Esc`, `Comma`, every other key
/// by its name. Keys pressed together are one chip: `Ctrl Enter`.
#[must_use]
pub fn key_words(shortcut: &KeyboardShortcut) -> String {
    let m = shortcut.modifiers;
    let command = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
    let mut words: Vec<&str> = Vec::new();
    if m.ctrl || (m.command && !m.mac_cmd) {
        words.push(if m.ctrl { "Ctrl" } else { command });
    }
    if m.mac_cmd {
        words.push("Cmd");
    }
    if m.alt {
        words.push("Alt");
    }
    if m.shift {
        words.push("Shift");
    }
    words.push(match shortcut.logical_key {
        Key::ArrowDown => "↓",
        Key::ArrowUp => "↑",
        Key::ArrowLeft => "←",
        Key::ArrowRight => "→",
        Key::Delete => "Del",
        Key::Escape => "Esc",
        Key::Comma => "Comma",
        key => key.name(),
    });
    words.join(" ")
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod tests;
