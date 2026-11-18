// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's "why here" line: WHY HERE in `label` and `cyan_text`, 12 right of it the reasons in `mono_s` and
//! `muted` on one line, and at the right end the small link `open log →`, on the `panel` fill in a 1 px `panel_line`
//! edge, 8 above and below, 12 left and right and between its parts, 32 high. The reasons are data (a folder's name, a
//! group's): drawn as plain words, cut between grapheme clusters with one ellipsis; a screen reader hears them whole.
//! The log the link opens is not built yet: the link stands unavailable, and the `coming soon` tag stands where the
//! reasons go while there are none (the app cannot say why yet), else in the room the reasons leave. The words keep the
//! places the sheet gives them; when room runs short the link gives way first, then the tag, and the reasons are cut
//! last. Nothing paints past the line, and nothing in it moves.

use egui::{Align, Layout, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::{LinkSize, link_sized};
use super::soon::{soon, soon_tag};

/// The room left and right of the line's parts and between them, in points.
const PAD_X: f32 = 12.0;

/// What a screen reader hears for a why line saying `reasons`.
pub(crate) fn spoken(reasons: &str) -> String {
    if reasons.is_empty() {
        return tr!("Why here: coming soon").to_owned();
    }
    tr!("Why here: {reasons}", reasons = super::spoken(reasons))
}

/// The why line across `ui`'s width saying `reasons`; the line's response, a group named by [`spoken`].
pub fn why_line(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reasons: &str) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(1.0).floor();
    let [high, ..] = theme.boxes().reader;
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let id = ui.scope_id().with("why-line");
    let response = ui.interact(rect, id, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, spoken(reasons)));
    let label = super::galley(ui, tr!("Why here"), f.label, p.cyan_text);
    let label_width = paint::run_box(label.size().x);
    let link_words = tr!("open log →");
    let link_width = super::galley(ui, link_words, f.mono_s, p.muted).size().x.ceil();
    let tag_width = super::tag::width(ui, theme, tr!("coming soon"));
    let left = rect.left() + PAD_X + label_width + PAD_X;
    let right = rect.right() - PAD_X;
    // The words keep the places the sheet gives them: the reasons from 12 right of the label, the link at the right
    // end. The parts that are not built give way first: the link, then the tag; the reasons are cut last.
    let wanted = if reasons.is_empty() { tag_width } else { paint::run_box(super::fitted(ui, reasons, f.mono_s, p.muted, f32::INFINITY).size().x) };
    let link_left = right - link_width;
    let link = left + wanted <= link_left - PAD_X;
    let words_right = if link { link_left - PAD_X } else { right };
    let room = (words_right - left).max(0.0);
    let words = super::fitted(ui, reasons, f.mono_s, p.muted, room);
    // With no reason yet the tag stands where the reasons go; with reasons, only in the room they leave.
    let tag_left = if reasons.is_empty() { left } else { left + paint::run_box(words.size().x) + PAD_X };
    let tag = tag_left + tag_width <= words_right;
    let words_rect = Rect::from_min_size(pos2(left, rect.top()), vec2(room.max(1.0), high));
    let said = ui.interact(words_rect, id.with("words"), Sense::hover());
    said.widget_info(|| WidgetInfo::labeled(Role::Label, true, super::spoken(reasons)));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect(rect, 0.0, p.panel, Stroke::new(1.0, p.panel_line), StrokeKind::Inside);
        let top = |line: f32| (rect.center().y - line / 2.0).round();
        let inside = painter.with_clip_rect(rect.intersect(ui.clip_rect()));
        paint::run_at(&inside, pos2(rect.left() + PAD_X, top(label.size().y)), label, p.cyan_text);
        if !reasons.is_empty() {
            paint::run_at(&inside, pos2(left, top(words.size().y)), words, p.muted);
        }
    }
    let mut part = |from: f32, width: f32, add: &mut dyn FnMut(&mut Ui)| {
        let place = Rect::from_min_size(pos2(from, rect.top()), vec2(width.max(1.0), high));
        let mut child = ui.new_child(UiBuilder::new().max_rect(place).layout(Layout::left_to_right(Align::Center)));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        soon(&mut child, theme, |ui| add(ui));
    };
    if tag {
        part(tag_left, tag_width, &mut |ui| {
            soon_tag(ui, theme);
        });
    }
    if link {
        part(link_left, link_width, &mut |ui| {
            link_sized(ui, theme, keymap, link_words, LinkSize::Small);
        });
    }
    response
}

#[cfg(test)]
#[path = "why_line_tests.rs"]
mod tests;
