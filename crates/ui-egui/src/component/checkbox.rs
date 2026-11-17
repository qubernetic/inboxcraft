// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Check boxes and radio buttons: a 16 px square before its name in mono 13, 12 between them, no round shapes. An
//! empty box has a `muted` edge (an edge that carries meaning is never the hairline colour). A ticked box is the
//! `cyan` fill with a `cyan_edge` edge and `×` in `on_cyan`; a chosen radio button keeps its square empty and carries
//! a solid 8 px `cyan_edge` square inside. Hovered, the empty box takes the `stage` fill and an `ink` edge and the
//! full one turns to `ink`; pressed, either is the accent wash with the accent edge. Focus is the ring round the box;
//! disabled is a dashed `muted` edge, the name and the cross in `muted`, the chosen square in `dim`. The whole row answers a press; none
//! of it moves (the design gives these no transition).

use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{Color32, Galley, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::{Font, Theme};

use super::{Hover, Shown, cut, lit, solid, soon};

/// Between the box and its name, in points.
const GAP: f32 = 12.0;

/// From a radio button's box to its chosen square, in points.
const DOT_INSET: f32 = 4.0;

/// A check box or a radio button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Check,
    Radio,
}

/// How the box is drawn: its fill, its edge (dashed or not) and the colour of its mark when it is on.
struct Look {
    fill: Color32,
    edge: Option<(Color32, bool)>,
    mark: Color32,
}

/// The box's look for `kind`, `on` or not, `enabled` or not, under what the pointer and the keyboard do (`lit`).
fn look(theme: &Theme, kind: Kind, on: bool, enabled: bool, lit: super::Lit) -> Look {
    let p = &theme.palette;
    let none = Color32::TRANSPARENT;
    if !enabled {
        // The cross is text a person reads, never `dim`; the radio button's square is a mark, `dim` as the switch's knob.
        let mark = if kind == Kind::Check { p.muted } else { p.dim };
        return Look { fill: none, edge: Some((p.muted, true)), mark };
    }
    if lit.pressed {
        let mark = if kind == Kind::Check { p.cyan_text } else { p.cyan_edge };
        return Look { fill: p.cyan_muted, edge: Some((p.cyan_edge, false)), mark };
    }
    match (kind, on, lit.hover) {
        // Hovered, a full box turns to `ink` (the check box's sheet; the radio button's says "hover as the checkbox").
        (Kind::Check, true, true) => Look { fill: p.ink, edge: None, mark: p.bg },
        (Kind::Check, true, false) => Look { fill: p.cyan, edge: Some((p.cyan_edge, false)), mark: p.on_cyan },
        (Kind::Radio, true, true) => Look { fill: p.ink, edge: None, mark: p.bg },
        (Kind::Radio, true, false) => Look { fill: none, edge: Some((p.cyan_edge, false)), mark: p.cyan_edge },
        (_, _, true) => Look { fill: p.stage, edge: Some((p.ink, false)), mark: p.ink },
        (_, false, false) => Look { fill: none, edge: Some((p.muted, false)), mark: p.muted },
    }
}

/// What a name too long for its room does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fit {
    /// It is cut with an ellipsis on its one line (a calendar's name in a narrow panel).
    Cut,
    /// It wraps onto the lines under its first, the box beside the first: a sentence that must be read whole.
    Wrap,
}

/// `text` in `font` and `colour`, wrapped at `width`.
fn wrapped(ui: &Ui, text: &str, font: Font, colour: Color32, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(font.text(text), font.format(colour));
    job.wrap.max_width = width.max(1.0);
    ui.painter().layout_job(job)
}

/// The box and `name` after it, in the room there is, the name cut or wrapped as `fit` says, drawn in `shown` when a
/// board asks; clicked when it was pressed.
fn toggle(ui: &mut Ui, theme: &Theme, kind: Kind, name: &str, on: bool, shown: Option<Shown>, fit: Fit) -> Response {
    let p = &theme.palette;
    let enabled = ui.is_enabled() && !soon::is_soon(ui);
    let side = theme.boxes().check;
    let font = theme.fonts.mono;
    let line = font.line.max(side);
    let ink = if enabled { p.ink } else { p.muted };
    let text = cut(ui, name, font, ink, f32::INFINITY);
    let wanted = side + GAP + text.size().x.ceil();
    let room = ui.available_width().max(side);
    let wide = wanted.min(room);
    let lines = (fit == Fit::Wrap && wanted > room).then(|| wrapped(ui, name, font, ink, room - side - GAP));
    let high = lines.as_ref().map_or(line, |lines| lines.size().y.max(line));
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(wide, high), sense);
    let role = if kind == Kind::Check { Role::CheckBox } else { Role::RadioButton };
    response.widget_info(|| WidgetInfo::selected(role, enabled, on, name));
    let lit = lit(ui, theme, &response, enabled, shown, Hover::AtOnce);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = solid(ui);
    // The box stands beside the name's first line.
    let mark = Rect::from_min_size(pos2(rect.left(), rect.top() + (line - side) / 2.0), vec2(side, side));
    let (text, top) = match lines {
        Some(lines) => (lines, rect.top()),
        None if wide < wanted => {
            let text = cut(ui, name, font, ink, rect.right() - mark.right() - GAP);
            let top = rect.top() + (line - text.size().y) / 2.0;
            (text, top)
        }
        None => {
            let top = rect.top() + (line - text.size().y) / 2.0;
            (text, top)
        }
    };
    painter.galley(pos2(mark.right() + GAP, top), text, ink);
    let look = look(theme, kind, on, enabled, lit);
    match look.edge {
        Some((colour, true)) => {
            painter.rect_filled(mark, 0.0, look.fill);
            paint::dashed(&painter, mark, colour);
        }
        Some((colour, false)) => {
            painter.rect(mark, 0.0, look.fill, Stroke::new(1.0, colour), StrokeKind::Inside);
        }
        None => {
            painter.rect_filled(mark, 0.0, look.fill);
        }
    }
    if on {
        match kind {
            Kind::Check => {
                let cross = super::galley(ui, "×", theme.fonts.check_mark(), look.mark);
                paint::run_centred(&painter, mark, cross, look.mark);
            }
            Kind::Radio => {
                painter.rect_filled(mark.shrink(DOT_INSET), 0.0, look.mark);
            }
        }
    }
    if lit.focus {
        paint::focus_ring(&painter, theme, mark, Ring::Outside);
    }
    response
}

/// A check box named `name`; clicked when it was pressed.
pub fn checkbox(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    toggle(ui, theme, Kind::Check, name, on, None, Fit::Cut)
}

/// A check box like [`checkbox`] whose name, when it is too long for its room, wraps onto more lines instead of being
/// cut: a sentence that must be read whole, such as what ticking it tells someone else.
pub fn checkbox_wrapping(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    toggle(ui, theme, Kind::Check, name, on, None, Fit::Wrap)
}

/// [`checkbox`] drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
pub(crate) fn checkbox_with(ui: &mut Ui, theme: &Theme, name: &str, on: bool, shown: Option<Shown>) -> Response {
    toggle(ui, theme, Kind::Check, name, on, shown, Fit::Cut)
}

/// A radio button named `name`, the chosen one of its row or not; clicked when it was pressed.
pub fn radio(ui: &mut Ui, theme: &Theme, name: &str, on: bool) -> Response {
    toggle(ui, theme, Kind::Radio, name, on, None, Fit::Cut)
}

/// [`radio`] drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
pub(crate) fn radio_with(ui: &mut Ui, theme: &Theme, name: &str, on: bool, shown: Option<Shown>) -> Response {
    toggle(ui, theme, Kind::Radio, name, on, shown, Fit::Cut)
}

#[cfg(test)]
#[path = "checkbox_tests.rs"]
mod tests;
