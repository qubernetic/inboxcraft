// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's "why here" line: WHY HERE in `label` and `cyan_text`, 12 right of it the reasons in `mono_s` and
//! `muted` on one line, and at the right end the small link `open log →`, on the `panel` fill in a 1 px `panel_line`
//! edge, 8 above and below, 12 left and right and between its parts, 32 high. The reasons are data (a folder's name, a
//! group's): drawn as plain words, cut between grapheme clusters with one ellipsis; a screen reader hears them whole.
//! The log the link opens is not built yet: the link stands unavailable with the `coming soon` tag before it.
//! Nothing in it moves.

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
    // The link and the tag at the right end, measured first: the reasons take what is left.
    let link_words = tr!("open log →");
    let link_width = super::galley(ui, link_words, f.mono_s, p.muted).size().x.ceil();
    let tag_width = super::galley(ui, tr!("coming soon"), f.mono_s, p.muted).size().x.ceil() + 12.0;
    let end = PAD_X + tag_width + PAD_X + link_width + PAD_X;
    let left = rect.left() + PAD_X + label_width + PAD_X;
    let room = (rect.right() - end - left).max(1.0);
    let words = super::fitted(ui, reasons, f.mono_s, p.muted, room);
    let words_rect = Rect::from_min_size(pos2(left, rect.top()), vec2(room, high));
    let said = ui.interact(words_rect, id.with("words"), Sense::hover());
    said.widget_info(|| WidgetInfo::labeled(Role::Label, true, super::spoken(reasons)));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect(rect, 0.0, p.panel, Stroke::new(1.0, p.panel_line), StrokeKind::Inside);
        let top = |line: f32| (rect.center().y - line / 2.0).round();
        paint::run_at(painter, pos2(rect.left() + PAD_X, top(label.size().y)), label, p.cyan_text);
        paint::run_at(painter, pos2(left, top(words.size().y)), words, p.muted);
    }
    let row = Rect::from_min_max(pos2(rect.right() - end + PAD_X, rect.top()), pos2(rect.right() - PAD_X, rect.bottom()));
    let mut child = ui.new_child(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)));
    child.spacing_mut().item_spacing.x = PAD_X;
    soon(&mut child, theme, |ui| {
        soon_tag(ui, theme);
        link_sized(ui, theme, keymap, link_words, LinkSize::Small);
    });
    response
}

#[cfg(test)]
#[path = "why_line_tests.rs"]
mod tests;
