// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The section label, `[ 01 ] MAIL`: the bracketed number in the accent and the word in `muted`, mono 11 in capitals
//! with 8 % tracking, 8 between them; the word in `ink` while the keyboard is in its section, and in `ink` on the
//! `grid` fill under the pointer when the label folds its section, with a `muted` edge round it while pressed. And [`count`], the written form of a count.

use egui::{Color32, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::Theme;

use super::{Face, Hover, Shown, between, lit};

/// The room between the number and the word, in points.
const GAP: f32 = 8.0;

/// How far the hover fill reaches round a folding label: 4 beside it, 2 over and under.
const HOVER_REACH: egui::Vec2 = vec2(4.0, 2.0);

/// The section label `index word` (`[ 01 ]`, `Mail`): the word in `ink` when `active`, else `muted`. A screen reader
/// hears the word as written; the screen shows it in capitals.
pub fn section_label(ui: &mut Ui, theme: &Theme, index: &str, word: &str, active: bool) -> Response {
    section_label_with(ui, theme, index, word, active, false, None)
}

/// [`section_label`]; one that `folds` its section is a button with the hover fill and a focus ring, drawn in
/// `shown` when that is given (the gallery's boards).
pub(crate) fn section_label_with(ui: &mut Ui, theme: &Theme, index: &str, word: &str, active: bool, folds: bool, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let font = theme.fonts.label;
    let number = super::galley(ui, index, font, p.cyan_text);
    let words = super::galley(ui, word, font, Color32::PLACEHOLDER);
    let (left, right) = (number.size().x.ceil(), words.size().x.ceil());
    let enabled = folds && ui.is_enabled() && !super::soon::is_soon(ui);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    // One line of the label style, on whole points.
    let (rect, response) = ui.allocate_exact_size(vec2(left + GAP + right, font.line.ceil()), sense);
    let name = format!("{index} {word}");
    response.widget_info(|| WidgetInfo::labeled(if folds { Role::Button } else { Role::Label }, ui.is_enabled(), &name));
    let lit = if folds { lit(ui, theme, &response, enabled, shown, Hover::AtOnce) } else { super::Lit::default() };
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let reach = rect.expand2(HOVER_REACH);
        let word = if active { p.ink } else { p.muted };
        let face = if lit.pressed {
            Face { fill: Some(p.grid), edge: Some(p.muted), ink: p.ink }
        } else {
            between(Face { fill: None, edge: None, ink: word }, Face { fill: Some(p.grid), edge: None, ink: p.ink }, lit.fade, lit.hover)
        };
        if let Some(fill) = face.fill {
            painter.rect_filled(reach, 0.0, fill);
        }
        if let Some(edge) = face.edge {
            // Pressed: a muted edge round the hover fill, as a pressed icon button takes one (the sheet has no
            // pressed state for the label).
            painter.rect_stroke(reach, 0.0, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
        }
        let ink = face.ink;
        let top = (rect.center().y - number.size().y / 2.0).round();
        painter.galley(pos2(rect.left(), top), number, p.cyan_text);
        painter.galley(pos2(rect.left() + left + GAP, top), words, ink);
        if lit.focus {
            paint::focus_ring(painter, theme, rect.expand2(HOVER_REACH), Ring::Outside);
        }
    }
    response
}

/// The most a [`count`] writes before it caps.
const MOST: u64 = 9_999;

/// `n` as a count is written: nothing for 0, groups of three apart by a thin space (U+202F) from 1 000, and `9 999+`
/// above 9 999.
#[must_use]
pub fn count(n: u64) -> String {
    if n == 0 {
        return String::new();
    }
    let digits = n.min(MOST).to_string();
    let mut out = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && digits.len().saturating_sub(at).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(digit);
    }
    if n > MOST {
        out.push('+');
    }
    out
}

#[cfg(test)]
#[path = "label_tests.rs"]
mod tests;
