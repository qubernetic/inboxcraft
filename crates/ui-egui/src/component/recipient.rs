// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A recipient line of the composer, as its sheets draw it (`recipient-chip`, `recipient-field`).
//!
//! A chip is 24 high on `grid` with a 1 px `line` edge: the name in `body`, 8 in, and the cross that removes it, 14
//! in `muted`, 4 from its right end. One that is not an address has a `minus` edge, a warning before its name and the
//! name in `minus`. Under the pointer it is the `stage` with a `muted` edge and an `ink` cross; while its cross has
//! the keyboard its edge is `cyan_edge`; pressed, which the design does not draw, it takes the press of a secondary
//! button (`stage`, an `ink` edge). Disabled: no fill, a dashed `line` edge, the name `muted`, the cross `dim`.
//!
//! The field is a box on `term` 36 high: the line's name ("TO") in the `label` style 12 in, the chips and the text
//! being typed after it, and "Cc Bcc" in `mono_s` at the right end while those lines are hidden. Focused its edge is
//! `cyan_edge` with the caret; a line with a chip that is not an address has a `minus` edge and the reason under the
//! box. Chips that do not fit on one row wrap, 10 apart, and the box grows. Nothing moves: the design gives a chip and
//! the field no transition.

use egui::{Color32, Id, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::theme::Theme;
use crate::tr;

use super::field::{self, Field};
use super::{Hover, Shown, cut, galley, lit, solid, soon};

/// Whether a recipient is an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientState {
    /// An address the mail can go to.
    Valid,
    /// Text that is not an address: it blocks sending.
    Invalid,
}

/// The Phosphor warning glyph before a recipient that is not an address.
const WARNING: char = '\u{e4e0}';

/// The least width of the line's name ("TO"), in points.
const NAME_COLUMN: f32 = 24.0;

/// The widest a chip gets before its name ends with an ellipsis, in points.
const WIDEST: f32 = 176.0;

/// The gap between two rows of chips, in points.
const ROW_GAP: f32 = 10.0;

/// A recipient's chip (see the module), 24 high: its response is the cross's, clicked when the recipient is to go.
/// A screen reader hears the chip as `name` and the cross as "Remove `name`".
pub fn recipient_chip(ui: &mut Ui, theme: &Theme, name: &str, state: RecipientState) -> Response {
    chip_with(ui, theme, name, name, state, None)
}

/// [`recipient_chip`] heard as `spoken`, drawn in `shown` whatever the pointer and the keyboard do (a board of the
/// gallery).
pub(crate) fn chip_with(ui: &mut Ui, theme: &Theme, name: &str, spoken: &str, state: RecipientState, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let enabled = ui.is_enabled() && !soon::is_soon(ui);
    let height = theme.boxes().pill;
    let glyph = theme.boxes().mark.min(height);
    let invalid = state == RecipientState::Invalid;
    let ink = if !enabled {
        p.muted
    } else if invalid {
        p.minus
    } else {
        p.ink
    };
    // 1 px edge and 8 before the name (the warning and 4 first when it is not an address), 4 and the cross, 4 and the
    // edge after it.
    let lead = 1.0 + s.s2 + if invalid { glyph + s.s1 } else { 0.0 };
    let tail = s.s1 + glyph + s.s1 + 1.0;
    let words = cut(ui, name, theme.fonts.body, ink, (WIDEST - lead - tail).max(1.0));
    let width = (lead + words.size().x + tail).ceil();
    let (rect, chip) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    chip.widget_info(|| WidgetInfo::labeled(Role::ListItem, enabled, spoken));
    let cross = Rect::from_min_size(pos2(rect.right() - 1.0 - s.s1 - glyph, rect.center().y - glyph / 2.0), vec2(glyph, glyph));
    let remove = tr!("Remove {recipient}", recipient = name);
    // Two chips may show one name; what a screen reader hears holds the address.
    let id = ui.make_persistent_id(("recipient-chip", spoken));
    let response = ui.interact(cross.expand(s.s1 / 2.0), id, if enabled { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &remove));
    let lit = lit(ui, theme, &response, enabled, shown, Hover::AtOnce);
    let hover = enabled && (lit.hover || (shown.is_none() && chip.hovered()) || shown == Some(Shown::Hover));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = solid(ui);
    let (fill, edge, cross_ink) = if !enabled {
        (None, None, p.dim)
    } else if lit.pressed {
        (Some(p.stage), Some(p.ink), p.ink)
    } else if lit.focus {
        (Some(p.grid), Some(p.cyan_edge), p.muted)
    } else if hover {
        (Some(p.stage), Some(p.muted), p.ink)
    } else {
        (Some(p.grid), Some(if invalid { p.minus } else { p.line }), p.muted)
    };
    if let Some(fill) = fill {
        painter.rect_filled(rect, 0.0, fill);
    }
    match edge {
        Some(edge) => {
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
        }
        None => paint::dashed(&painter, rect, p.line),
    }
    let mut left = rect.left() + 1.0 + s.s2;
    if invalid {
        let at = Rect::from_min_size(pos2(left, rect.center().y - glyph / 2.0), vec2(glyph, glyph));
        crate::iconset::paint_glyph(&painter, at, WARNING, if enabled { p.minus } else { p.dim });
        left += glyph + s.s1;
    }
    painter.galley(pos2(left, rect.center().y - words.size().y / 2.0), words, ink);
    crate::iconset::paint(&painter, cross, IconId::Close, cross_ink);
    response
}

/// A recipient on a line: what its chip shows, what a screen reader hears, and whether it is an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    /// The name, or the address when there is none.
    pub shown: String,
    /// What a screen reader says for it.
    pub spoken: String,
    /// Whether it is an address.
    pub state: RecipientState,
}

/// What a recipient line shows.
#[derive(Debug, Clone, Copy)]
pub struct Recipients<'a> {
    /// The line's name ("To", "Cc", "Bcc"): in capitals before the chips, and the text field's name.
    pub name: &'a str,
    /// The chips, in order.
    pub chips: &'a [Recipient],
    /// What stands in the empty line ("Name or address").
    pub hint: &'a str,
    /// Whether "Cc Bcc" stands at the right end (the To line while those lines are hidden).
    pub copies: bool,
    /// Why the line cannot go, under the box; its edge is `minus` then, and with a chip that is not an address.
    pub error: Option<&'a str>,
    /// Whether it can be used.
    pub enabled: bool,
}

/// What a person did to a recipient line in this frame.
#[derive(Debug)]
pub struct RecipientsOutput {
    /// The text field's response: changed, lost focus, its rect.
    pub field: Response,
    /// The chip whose cross was pressed, by its place.
    pub removed: Option<usize>,
    /// "Cc Bcc" was pressed.
    pub copies: bool,
}

/// The least room the text being typed keeps beside the last chip before it goes to a row of its own, in points.
const TYPING: f32 = 32.0;

/// A recipient line (see the module), filling the width `ui` offers: the field `id` with `typing` in it.
pub fn recipient_field(ui: &mut Ui, theme: &Theme, id: Id, typing: &mut String, line: &Recipients<'_>) -> RecipientsOutput {
    recipient_field_with(ui, theme, id, typing, line, None)
}

/// [`recipient_field`] drawn in `shown`: a board of the gallery.
pub(crate) fn recipient_field_with(ui: &mut Ui, theme: &Theme, id: Id, typing: &mut String, line: &Recipients<'_>, shown: Option<Shown>) -> RecipientsOutput {
    let p = &theme.palette;
    let s = &theme.spacing;
    let width = if ui.available_width().is_finite() { ui.available_width().max(theme.boxes().field * 4.0) } else { theme.boxes().field * 4.0 };
    let enabled = line.enabled && ui.is_enabled() && !soon::is_soon(ui);
    let pad = s.s3 + 1.0;
    let chip = theme.boxes().pill;
    let field_height = theme.boxes().field;
    let label = theme.fonts.label;
    let name_width = galley(ui, line.name, label, p.muted).size().x.max(NAME_COLUMN);
    let copies_text = tr!("Cc Bcc");
    let copies = line.copies.then(|| galley(ui, copies_text, theme.fonts.mono_s, p.muted));
    let right = pad + copies.as_ref().map_or(0.0, |copies| copies.size().x + s.s2);
    let left = pad + name_width + s.s2;
    let room = (width - left - right).max(1.0);
    // Where each chip goes and where the typing goes, rows 10 apart.
    let widths: Vec<f32> = line.chips.iter().map(|recipient| chip_width(ui, theme, recipient)).collect();
    let mut places = Vec::with_capacity(widths.len());
    let (mut x, mut row) = (0.0_f32, 0_usize);
    for width in &widths {
        if x > 0.0 && x + width > room {
            x = 0.0;
            row = row.saturating_add(1);
        }
        places.push((x, row));
        x += width + s.s1;
    }
    if x > 0.0 && room - x < TYPING.min(room) {
        x = 0.0;
        row = row.saturating_add(1);
    }
    let typing_at = (x, row);
    let rows = row.saturating_add(1) as f32;
    let wrapped = rows > 1.0;
    let inner = rows * chip + (rows - 1.0) * ROW_GAP;
    let height = if wrapped { inner + (s.s1 + s.s1 / 2.0 + 1.0) * 2.0 } else { field_height };
    ui.vertical(|ui| {
        let (mut removed, mut copies_pressed) = (None, false);
        ui.spacing_mut().item_spacing.y = s.s2;
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let focused = shown == Some(Shown::Focus) || (enabled && ui.memory(|memory| memory.has_focus(id)));
        let invalid = line.error.is_some() || line.chips.iter().any(|chip| chip.state == RecipientState::Invalid);
        let painter = solid(ui);
        if ui.is_rect_visible(rect) {
            if !enabled {
                paint::dashed(&painter, rect, p.line);
            } else {
                let edge = if focused {
                    p.cyan_edge
                } else if invalid {
                    p.minus
                } else {
                    p.line
                };
                painter.rect(rect, 0.0, p.term, Stroke::new(1.0, edge), StrokeKind::Inside);
            }
        }
        // The first row's middle: the name, the chips and "Cc Bcc" line up on it.
        let top = if wrapped { rect.top() + (height - inner) / 2.0 } else { rect.top() + (height - chip) / 2.0 };
        let first = top + chip / 2.0;
        let name = galley(ui, line.name, label, p.muted);
        painter.galley(pos2(rect.left() + pad, first - name.size().y / 2.0), name, p.muted);
        let origin = pos2(rect.left() + left, top);
        let at = |(x, row): (f32, usize)| origin + vec2(x, row as f32 * (chip + ROW_GAP));
        for (index, (recipient, place)) in line.chips.iter().zip(places.iter()).enumerate() {
            let place = Rect::from_min_size(at(*place), vec2(widths.get(index).copied().unwrap_or(0.0), chip));
            let mut child = ui.new_child(UiBuilder::new().max_rect(place));
            if !enabled {
                child.disable();
            }
            let chip_shown = if enabled { None } else { Some(Shown::Rest) };
            if chip_with(&mut child, theme, &recipient.shown, &recipient.spoken, recipient.state, chip_shown).clicked() {
                removed = Some(index);
            }
        }
        // The text being typed: the rest of its row, the field's caret and hint its own.
        let start = at(typing_at);
        let typing_rect = Rect::from_min_max(pos2(start.x, start.y + (chip - field_height) / 2.0), pos2(rect.right() - right, start.y + (chip + field_height) / 2.0));
        let mut child = ui.new_child(UiBuilder::new().max_rect(typing_rect));
        let hint = if line.chips.is_empty() { line.hint } else { "" };
        let field = Field { label: "", hint, error: None, enabled, read_only: false };
        let typed = field::bare_line(&mut child, theme, id, typing, &field, shown);
        let spoken = line.name;
        ui.ctx().accesskit_node_builder(typed.id, |node| node.set_label(spoken));
        if let Some(copies) = copies {
            let size = copies.size();
            let place = Rect::from_min_size(pos2(rect.right() - pad - size.x, first - size.y / 2.0), size);
            let response = ui.interact(place, id.with("copies"), if enabled { Sense::click() } else { Sense::hover() });
            response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, tr!("Cc  Bcc")));
            let ink = if enabled && (response.hovered() || response.has_focus()) { p.ink } else { p.muted };
            painter.galley(place.min, copies, ink);
            if response.has_focus() {
                paint::focus_ring(&painter, theme, place, paint::Ring::Outside);
            }
            copies_pressed = response.clicked();
        }
        if let Some(error) = line.error {
            let mut job = egui::text::LayoutJob::single_section(error.to_owned(), theme.fonts.caption.format(p.minus));
            job.wrap.max_width = width.max(1.0);
            let text = ui.painter().layout_job(job);
            let (place, response) = ui.allocate_exact_size(vec2(width, text.size().y), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, error));
            painter.galley(place.min, text, p.minus);
        }
        RecipientsOutput { field: typed, removed, copies: copies_pressed }
    })
    .inner
}

/// The width a recipient's chip takes, in points.
fn chip_width(ui: &Ui, theme: &Theme, recipient: &Recipient) -> f32 {
    let s = &theme.spacing;
    let glyph = theme.boxes().mark.min(theme.boxes().pill);
    let invalid = recipient.state == RecipientState::Invalid;
    let lead = 1.0 + s.s2 + if invalid { glyph + s.s1 } else { 0.0 };
    let tail = s.s1 + glyph + s.s1 + 1.0;
    let words = cut(ui, &recipient.shown, theme.fonts.body, Color32::PLACEHOLDER, (WIDEST - lead - tail).max(1.0));
    (lead + words.size().x + tail).ceil()
}

#[cfg(test)]
#[path = "recipient_tests.rs"]
mod tests;
