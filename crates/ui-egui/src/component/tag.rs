// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tag: a short mark in mono 11 in a 1 px box, 20 high, 6 beside the text, never a button. The app's tags are
//! hollow: a signal's tag takes the signal's colour for its edge and its text, the fill stays empty. The coming-soon
//! mark ([`super::soon::soon_tag`]) is a tag too: `muted` words in a dashed `muted` edge.

use egui::{Color32, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::theme::Theme;

/// What a tag says, and so its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    /// A plain mark (Draft): the `line` edge, `muted` text.
    Default,
    /// The accent (New sender): the `cyan_edge` and `cyan_text`.
    Accent,
    /// Waiting or returned: `warn`.
    Warn,
    /// An error (Not delivered): `minus`.
    Minus,
    /// Passed or let in: `plus`.
    Plus,
}

/// The widest a tag's words get before they end with an ellipsis, in points.
const WIDEST: f32 = 240.0;

/// The room left and right of a tag's words, in points.
const PAD: f32 = 6.0;

/// A tag with `text` in `kind`'s colours.
pub fn tag(ui: &mut Ui, theme: &Theme, text: &str, kind: TagKind) -> Response {
    let p = &theme.palette;
    let (edge, ink) = match kind {
        TagKind::Default => (p.line, p.muted),
        TagKind::Accent => (p.cyan_edge, p.cyan_text),
        TagKind::Warn => (p.warn, p.warn),
        TagKind::Minus => (p.minus, p.minus),
        TagKind::Plus => (p.plus, p.plus),
    };
    draw(ui, theme, text, edge, ink, false)
}

/// The coming-soon mark: `text` (the words "coming soon") in `muted` in a dashed `muted` edge, nothing inside.
pub(crate) fn coming_soon(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    draw(ui, theme, text, theme.palette.muted, theme.palette.muted, true)
}

/// How wide a tag of `text` is drawn.
pub(crate) fn width(ui: &Ui, theme: &Theme, text: &str) -> f32 {
    super::cut(ui, text, theme.fonts.mono_s, theme.palette.muted, WIDEST).size().x.ceil().min(WIDEST) + PAD * 2.0
}

/// A tag with `text` in `ink`, its edge in `edge`, dashed or not.
fn draw(ui: &mut Ui, theme: &Theme, text: &str, edge: Color32, ink: Color32, dashed: bool) -> Response {
    let galley = super::cut(ui, text, theme.fonts.mono_s, ink, WIDEST);
    let width = galley.size().x.ceil().min(WIDEST);
    let (rect, response) = ui.allocate_exact_size(vec2(width + PAD * 2.0, theme.boxes().chip), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, ui.is_enabled(), text));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if dashed {
            paint::dashed(painter, rect, edge);
        } else {
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
        }
        let top = (rect.center().y - galley.size().y / 2.0).round();
        painter.galley(pos2(rect.left() + PAD, top), galley, ink);
    }
    response
}

#[cfg(test)]
#[path = "tag_tests.rs"]
mod tests;
