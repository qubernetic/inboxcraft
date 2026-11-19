// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The block (`block`, Home's brief): a hairline box on `surface` with a `line` edge, as wide as its room and as high
//! as what it holds. Its head is 36 high, the section label 16 in and on its middle, a `panel_line` hairline under it;
//! under that its content: lines of text 12 down and 16 in, 4 apart ([`Inset::Text`]), or rows and block lines edge to
//! edge ([`Inset::Flush`]). A block that presses is a button: the pointer over it turns its edge `muted`, the keyboard
//! on it puts the focus ring 2 px outside it, and it returns the press. A selected block stands on `stage` with a
//! `cyan_edge` edge. [`text_line`] is a line of its content in `mono` and `ink`, cut with one `…`.

use egui::{Id, Rect, Response, Role, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::Theme;

use super::{Hover, Shown, lit};

/// The head's height at 100 % text, in points.
pub const HEAD: f32 = 36.0;

/// The room beside the head's label and the content's text, in points.
const PAD_X: f32 = 16.0;

/// The room over and under text content, and between its lines, in points.
const PAD_Y: f32 = 12.0;
const LINE_GAP: f32 = 4.0;

/// How a block's content stands in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inset {
    /// Lines of text: 12 over and under, 16 beside, 4 apart (the agenda).
    Text,
    /// Rows and block lines across the whole width, edge to edge (the drafts).
    Flush,
}

/// What a block shows round its content.
#[derive(Debug, Clone, Copy)]
pub struct Block<'a> {
    /// Its id.
    pub id: Id,
    /// The section label's number: `[ 02 ]`.
    pub index: &'a str,
    /// The section label's word: "Agenda" (drawn in capitals).
    pub word: &'a str,
    /// How its content stands in it.
    pub inset: Inset,
    /// Whether the whole block is a button.
    pub presses: bool,
    /// Whether it is the selected one (`stage`, `cyan_edge`).
    pub selected: bool,
}

/// What a block did this frame.
#[derive(Debug)]
pub struct Drawn<R> {
    /// Its box.
    pub rect: Rect,
    /// It was pressed (only a block that presses).
    pub pressed: bool,
    /// What its content returned.
    pub inner: R,
}

/// The block `block` across `ui`'s width at its cursor, its content drawn by `content`; `shown` draws a block that
/// presses hovered or focused whatever the pointer does (the gallery's boards).
pub fn block<R>(ui: &mut Ui, theme: &Theme, block: &Block<'_>, shown: Option<Shown>, content: impl FnOnce(&mut Ui) -> R) -> Drawn<R> {
    let p = &theme.palette;
    let width = ui.available_width().max(PAD_X * 2.0 + 1.0).round();
    let top = ui.cursor().min;
    let head = Theme::size(theme, HEAD);
    // The ground and the edge go under the content: painted once its height is known.
    let ground = ui.painter().add(Shape::Noop);
    // A block that presses takes the pointer and the keyboard before its content does, so the content's own controls
    // (laid down after it) stay over it and the Tab order follows the drawn order. Its height is last frame's.
    let remembered = ui.data(|data| data.get_temp::<f32>(block.id.with("high"))).unwrap_or(head + 1.0);
    let response = block.presses.then(|| {
        let response = ui.interact(Rect::from_min_size(top, vec2(width, remembered)), block.id, Sense::click());
        // Its word: the section label in it is heard as well, with its number.
        response.widget_info(|| WidgetInfo::labeled(Role::Button, true, block.word));
        response
    });
    let lit = response.as_ref().map(|response| lit(ui, theme, response, true, shown, Hover::AtOnce)).unwrap_or_default();
    // The head: the section label 16 in, on the head's middle.
    let label_high = theme.fonts.label.line.ceil();
    let label_place = Rect::from_min_size(pos2(top.x + PAD_X, top.y + ((head - label_high) / 2.0).round()), vec2(width - PAD_X * 2.0, label_high));
    let mut child = ui.new_child(UiBuilder::new().max_rect(label_place).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.set_clip_rect(label_place.intersect(ui.clip_rect()));
    super::label::section_label(&mut child, theme, block.index, block.word, false);
    paint::hairline(ui.painter(), pos2(top.x, top.y + head), pos2(top.x + width, top.y + head), p.panel_line);
    // The content under the hairline.
    let (inner_x, inner_y, gap) = match block.inset {
        Inset::Text => (PAD_X, PAD_Y, LINE_GAP),
        Inset::Flush => (0.0, 0.0, 0.0),
    };
    let body_top = top.y + head + 1.0;
    let room = Rect::from_min_max(pos2(top.x + inner_x, body_top + inner_y), pos2(top.x + width - inner_x, f32::INFINITY));
    let mut body = ui.new_child(UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
    body.spacing_mut().item_spacing.y = gap;
    body.set_clip_rect(Rect::from_min_max(room.min, pos2(room.right(), ui.clip_rect().bottom())).intersect(ui.clip_rect()));
    let inner = content(&mut body);
    let used = (body.min_rect().bottom() - body_top).max(0.0) + inner_y;
    let high = (head + 1.0 + used).round();
    let rect = Rect::from_min_size(top, vec2(width, high));
    ui.allocate_rect(rect, Sense::hover());
    if (high - remembered).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(block.id.with("high"), high));
        if block.presses {
            ui.ctx().request_repaint();
        }
    }
    let (fill, edge) = if block.selected {
        (p.stage, p.cyan_edge)
    } else if lit.hover || lit.pressed {
        (p.surface, p.muted)
    } else {
        (p.surface, p.line)
    };
    ui.painter().set(ground, Shape::rect_filled(rect, 0.0, fill));
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    if lit.focus {
        paint::focus_ring(ui.painter(), theme, rect, Ring::Outside);
    }
    let pressed = response.is_some_and(|response| response.clicked());
    Drawn { rect, pressed, inner }
}

/// A line of a block's text content across `ui`'s width: `text` in `mono` and `ink`, plain, on one line cut with one
/// `…` (at most [`super::MOST`] characters of it laid out). Its response.
pub fn text_line(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    let f = theme.fonts.mono;
    let p = &theme.palette;
    let width = ui.available_width().max(1.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, f.line), Sense::hover());
    let (kept, cut) = super::capped(text, super::MOST);
    let words = super::plain(kept);
    let words = if cut { format!("{words}…") } else { words.into_owned() };
    let shown = super::fit_capped(ui, &words, f, width);
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &shown));
    if ui.is_rect_visible(rect) {
        paint::run_at(&ui.painter_at(rect), rect.min, super::galley(ui, &shown, f, p.ink), p.ink);
    }
    response
}

#[cfg(test)]
#[path = "block_tests.rs"]
mod tests;
