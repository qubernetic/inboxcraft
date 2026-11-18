// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A quote of a message, folded: one line 28 high with a 2 px `line` edge at its left, the caret `▸` and how many lines
//! it holds in `mono_s` and `muted`, 12 in from the edge. Under the pointer it takes the `stage` fill and its words
//! `ink`; the focus ring stands inside it. Open, the caret turns `▾` and the line says `hide quoted text`; under it the
//! attribution in `caption` and the quoted text in `body`, both `muted`, 8 above its foot, the edge running down the
//! whole height. The line is one stop for Tab and opens or folds the quote on Enter or a click. The quoted text is data:
//! plain words, wrapped, its own line breaks kept. Nothing moves: the quote opens and folds at once (the design gives the fold no timing).

use std::sync::Arc;

use egui::{Galley, Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::Theme;
use crate::tr;

use super::button::{Caret, baseline, caret_width, paint_caret};
use super::{Hover, Shown, lit};

/// The width of the edge at the left, in points.
const EDGE: f32 = 2.0;

/// The room between the edge's outer side and the words, in points.
const PAD: f32 = 12.0;

/// The room on the right of the words, in points.
const PAD_RIGHT: f32 = 8.0;

/// The room under the quoted text, in points.
const FOOT: f32 = 8.0;

/// The words of the fold's line: how many lines it holds, or how to fold it again.
pub(crate) fn words(lines: usize, open: bool) -> String {
    match (open, lines) {
        (true, _) => tr!("hide quoted text").to_owned(),
        (false, 1) => tr!("1 quoted line").to_owned(),
        (false, count) => tr!("{count} quoted lines", count = count),
    }
}

/// What a screen reader hears for the fold's line.
pub(crate) fn spoken(lines: usize, open: bool) -> String {
    if open { tr!("Hide quoted text").to_owned() } else { tr!("Show {words}", words = words(lines, false)) }
}

/// A quote of `lines` lines, `open` or folded, with its `attribution` and `text` shown when open; the line's response
/// (a click opens or folds it).
pub fn quote_fold(ui: &mut Ui, theme: &Theme, lines: usize, open: bool, attribution: Option<&str>, text: &str) -> Response {
    quote_fold_with(ui, theme, lines, open, attribution, text, None)
}

/// [`quote_fold`] drawn in `shown` whatever the pointer and the keyboard do (the gallery's boards).
pub(crate) fn quote_fold_with(
    ui: &mut Ui,
    theme: &Theme,
    lines: usize,
    open: bool,
    attribution: Option<&str>,
    text: &str,
    shown: Option<Shown>,
) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(PAD + PAD_RIGHT + 1.0).floor();
    let [.., high] = theme.boxes().reader;
    let room = width - PAD - PAD_RIGHT;
    let slot = ui.next_auto_id();
    let said = attribution.filter(|_| open).map(|said| kept_lines(ui, slot.with("said"), said, f.caption, p.muted, room));
    let quoted = open.then(|| kept_lines(ui, slot.with("quoted"), text, f.body, p.muted, room));
    let below = said.as_ref().map_or(0.0, |said| said.size().y) + quoted.as_ref().map_or(0.0, |quoted| quoted.size().y);
    let total = if open { (high + below + FOOT).ceil() } else { high };
    let (id, whole) = ui.allocate_space(vec2(width, total));
    let head = Rect::from_min_size(whole.min, vec2(width, high));
    let response = ui.interact(head, id.with("head"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, spoken(lines, open)));
    let lit = lit(ui, theme, &response, true, shown, Hover::AtOnce);
    if ui.is_rect_visible(whole) {
        let painter = ui.painter();
        if lit.hover {
            painter.rect_filled(head, 0.0, p.stage);
        }
        painter.rect_filled(Rect::from_min_size(whole.min, vec2(EDGE, total)), 0.0, p.line);
        let ink = if lit.hover { p.ink } else { p.muted };
        let left = whole.left() + PAD;
        let middle = head.center().y;
        let caret = if open { Caret::Down } else { Caret::Right };
        paint_caret(painter, f.mono_s, left, baseline(f.mono_s, middle), caret, ink);
        let line = super::cut(ui, &format!(" {}", words(lines, open)), f.mono_s, ink, room - caret_width(f.mono_s));
        paint::run_at(painter, pos2(left + caret_width(f.mono_s), (middle - line.size().y / 2.0).round()), line, ink);
        let mut top = head.bottom();
        if let Some(said) = said.clone() {
            let high = said.size().y;
            paint::rows_at(painter, pos2(left, top), room, said, p.muted);
            top += high;
        }
        if let Some(quoted) = quoted.clone() {
            paint::rows_at(painter, pos2(left, top), room, quoted, p.muted);
        }
        if lit.focus {
            paint::focus_ring(painter, theme, head, Ring::Inside);
        }
    }
    // The quote's words for a screen reader, whole.
    let mut top = head.bottom();
    let left = whole.left() + PAD;
    for (index, (galley, words)) in [(said, attribution.unwrap_or_default()), (quoted, text)].into_iter().enumerate() {
        let Some(galley) = galley else { continue };
        let rect = Rect::from_min_size(pos2(left, top), vec2(room, galley.size().y));
        let heard = ui.interact(rect, id.with(("words", index)), Sense::hover());
        heard.widget_info(|| WidgetInfo::labeled(Role::Label, true, super::spoken(words)));
        top += galley.size().y;
    }
    response
}

/// `text` of many lines from outside, its line breaks kept ([`super::plain_lines`]), wrapped at `room`. The galley is
/// kept under `slot` with what it was laid out from, so an open quote is not copied and laid out again every frame:
/// only its words are hashed to see that they are the same.
fn kept_lines(ui: &Ui, slot: egui::Id, text: &str, font: crate::theme::Font, colour: egui::Color32, room: f32) -> Arc<Galley> {
    let key = {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (text, room.to_bits(), font.size.to_bits(), colour).hash(&mut hasher);
        hasher.finish()
    };
    if let Some((kept, galley)) = ui.data(|data| data.get_temp::<(u64, Arc<Galley>)>(slot))
        && kept == key
    {
        return galley;
    }
    let galley = super::wrapped(ui, &super::plain_lines(text), font, colour, room, usize::MAX, egui::Align::LEFT);
    ui.data_mut(|data| data.insert_temp(slot, (key, galley.clone())));
    galley
}

#[cfg(test)]
#[path = "quote_fold_tests.rs"]
mod tests;
