// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The card: a rounded surface with a header that opens and closes it, for a grouped inbox group, the screener and
//! each message of a thread in the reader. Its body unrolls and rolls up over the card motion token; while closed it
//! is not drawn at all. The header's height comes from the heading font, never fixed points.

use egui::{Id, Pos2, Rect, Response, Role, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::motion;
use crate::theme::Theme;

use super::{galley, line_height};

/// What a card drew.
#[derive(Debug)]
pub struct Card<R> {
    /// The header: clicked when the person asked to open or close the card.
    pub header: Response,
    /// What the body gave, when it was drawn.
    pub inner: Option<R>,
    /// How far the card is open, 0 to 1.
    pub openness: f32,
    /// The whole card.
    pub rect: Rect,
}

/// A card `id` with `title` in its header across the width of `ui`, open or closed, its body drawn by `body`.
pub fn card<R>(ui: &mut Ui, theme: &Theme, id: Id, title: &str, open: bool, body: impl FnOnce(&mut Ui) -> R) -> Card<R> {
    let p = &theme.palette;
    let s = &theme.spacing;
    let slot = ui.painter().add(Shape::Noop);
    let width = ui.available_width().max(1.0);
    let header_height = line_height(ui, theme.fonts.heading) + s.m * 2.0;
    let (header_rect, header) = ui.allocate_exact_size(vec2(width, header_height), Sense::click());
    header.widget_info(|| WidgetInfo::selected(Role::Button, true, open, title));
    let lit = header.hovered() || header.has_focus();
    let hover = motion::toggle(ui.ctx(), id.with("hover"), lit, theme.motion.hover_fade);
    let text = galley(ui, title, theme.fonts.heading, p.text);
    let text_at = pos2(header_rect.left() + s.l, header_rect.center().y - text.size().y / 2.0);
    let openness = motion::toggle(ui.ctx(), id.with("open"), open, theme.motion.card);
    let mut inner = None;
    let mut body_height = 0.0;
    if openness > 0.0 {
        let top = header_rect.bottom();
        let room = Rect::from_min_size(pos2(header_rect.left() + s.l, top), vec2((width - s.l * 2.0).max(1.0), f32::INFINITY));
        let mut child = ui.new_child(UiBuilder::new().id_salt(id.with("body")).max_rect(room));
        // Unrolling, the body shows as far as the height it had when last drawn whole allows.
        let remembered: f32 = ui.data(|data| data.get_temp(id.with("height"))).unwrap_or(0.0);
        let shown = if openness >= 1.0 { f32::INFINITY } else { remembered * openness };
        child.shrink_clip_rect(Rect::from_min_max(pos2(header_rect.left(), top), pos2(header_rect.right(), top + shown)));
        inner = Some(body(&mut child));
        let full_height = child.min_rect().height() + s.l;
        ui.data_mut(|data| data.insert_temp(id.with("height"), full_height));
        body_height = if openness >= 1.0 { full_height } else { full_height * openness };
        ui.allocate_exact_size(vec2(width, body_height), Sense::hover());
    }
    let rect = Rect::from_min_max(header_rect.min, Pos2::new(header_rect.right(), header_rect.bottom() + body_height));
    let mut shapes =
        vec![Shape::rect_filled(rect, s.card, p.surface), Shape::rect_stroke(rect, s.card, Stroke::new(1.0, p.line), StrokeKind::Inside)];
    if hover > 0.0 {
        let round = egui::CornerRadius { nw: super::corner(s.card).nw, ne: super::corner(s.card).ne, sw: 0, se: 0 };
        let round = if openness > 0.0 { round } else { super::corner(s.card) };
        shapes.push(Shape::rect_filled(header_rect, round, p.hover.gamma_multiply(hover)));
    }
    shapes.push(Shape::galley(text_at, text, p.text));
    if header.has_focus() {
        shapes.push(Shape::rect_stroke(header_rect.shrink(1.0), s.card, Stroke::new(2.0, p.focus), StrokeKind::Inside));
    }
    ui.painter().set(slot, Shape::Vec(shapes));
    Card { header, inner, openness, rect }
}

#[cfg(test)]
#[path = "card_tests.rs"]
mod tests;
