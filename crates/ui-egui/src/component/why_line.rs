// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's "why here" line: WHY HERE in `label` and `cyan_text`, 12 right of it the reasons in `mono_s` and
//! `muted` on one line, two spaces, a middle dot and two spaces between them, and at the right end the small link
//! `open log →`, on the `panel` fill in a 1 px `panel_line` edge, 8 above and below, 12 left and right and between its
//! parts, 32 high. The reasons are the engine's, in the window's words (see `crate::why`), most specific first; they
//! hold names from outside (an address, a folder), drawn as plain words. What does not fit leaves whole from the end, so
//! no reason is cut in the middle, except the first when it stands alone, which is cut between grapheme clusters with
//! one ellipsis. The rule reads the width the line has, never the window's. A screen reader hears every reason as one
//! sentence, the ones that left included. The log the link opens is not built yet: the link stands unavailable while
//! the first reason fits whole beside it; the `coming soon` tag stands where the reasons go while there are none, and
//! beside reasons, as the frames draw them, not at all. Nothing paints past the line, nothing in it moves, and it takes no Tab stop.

use std::sync::Arc;

use egui::{Align, Galley, Layout, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::{LinkSize, link_sized};
use super::soon::{soon, soon_tag};

/// The room left and right of the line's parts and between them, in points.
const PAD_X: f32 = 12.0;

/// What stands between two reasons, as the design writes it.
pub const SEPARATOR: &str = "  \u{b7}  ";

/// What a screen reader hears for a why line saying `reasons`: one sentence, every reason in it.
pub(crate) fn spoken<S: AsRef<str>>(reasons: &[S]) -> String {
    if reasons.is_empty() {
        return tr!("Why here: coming soon").to_owned();
    }
    tr!("Why here: {reasons}", reasons = said(reasons))
}

/// The reasons as one sentence, cleaned for a screen reader.
fn said<S: AsRef<str>>(reasons: &[S]) -> String {
    reasons.iter().map(|reason| super::spoken(reason.as_ref())).collect::<Vec<_>>().join(", ")
}

/// The reasons that fit in `room`: as many whole from the first as fit, joined; the first alone, cut, when even it does
/// not fit.
fn fitting<S: AsRef<str>>(ui: &Ui, theme: &Theme, reasons: &[S], room: f32) -> Arc<Galley> {
    let (f, p) = (&theme.fonts, &theme.palette);
    let mut joined = String::new();
    let mut kept = None;
    for reason in reasons {
        let before = joined.len();
        if before > 0 {
            joined.push_str(SEPARATOR);
        }
        joined.push_str(reason.as_ref());
        let galley = super::fitted(ui, &joined, f.mono_s, p.muted, f32::INFINITY);
        if paint::run_box(galley.size().x) > room {
            joined.truncate(before);
            break;
        }
        kept = Some(galley);
    }
    match (kept, reasons.first()) {
        (Some(galley), _) => galley,
        (None, first) => super::fitted(ui, first.map_or("", AsRef::as_ref), f.mono_s, p.muted, room),
    }
}

/// The why line across `ui`'s width saying `reasons`, most specific first; the line's response, a group named by
/// [`spoken`].
pub fn why_line<S: AsRef<str>>(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reasons: &[S]) -> Response {
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
    // The link stands where the sheet puts it while the first reason fits whole beside it (with none, while the tag
    // does): the reasons leave whole from the end first, then the link, which leads nowhere yet, and only then is the
    // first reason, standing alone, cut.
    let link_left = right - link_width;
    let wanted = match reasons.first() {
        Some(first) => paint::run_box(super::fitted(ui, first.as_ref(), f.mono_s, p.muted, f32::INFINITY).size().x),
        None => tag_width,
    };
    let link = left + wanted <= link_left - PAD_X;
    let words_right = if link { link_left - PAD_X } else { right };
    let room = (words_right - left).max(0.0);
    let words = fitting(ui, theme, reasons, room);
    // With no reason the tag stands where the reasons go; beside reasons the frames draw none.
    let tag_left = left;
    let tag = reasons.is_empty() && tag_left + tag_width <= words_right;
    let words_rect = Rect::from_min_size(pos2(left, rect.top()), vec2(room.max(1.0), high));
    if !reasons.is_empty() {
        let said_rect = ui.interact(words_rect, id.with("words"), Sense::hover());
        said_rect.widget_info(|| WidgetInfo::labeled(Role::Label, true, said(reasons)));
    }
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
