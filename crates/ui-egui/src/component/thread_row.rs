// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A message of a thread on one line, 40 high, a 1 px `panel_line` under it: the caret (`▸` folded, `▾` open), the
//! unread square, the sender's avatar, the sender 152 wide, the first words, and the date at the right end, 10 apart
//! and 12 in from the sides. Folded, the sender is `body` and the first words `caption`, both quiet; unread, the sender
//! turns `strong`, the words `ink`, the date `cyan_text` and the square `cyan_edge`. Open, the line is the message's
//! head: the sender `strong`, the address and to whom in `mono_s`, and `details ▾` before the date. Under the pointer
//! the row takes the `stage` fill and its avatar the `surface`; the focus ring stands inside it. The sender, the words
//! and the address are data: plain words on one line, cut between grapheme clusters with one ellipsis; a screen reader
//! hears them whole. Nothing moves: the row opens and folds at once (the design gives the fold no timing).
//!
//! A row narrower than its parts gives way by the design's widths rule: the words (the row's fill) first, then
//! `details ▾` whole, then the sender is cut below its 152; the date is never cut. Nothing paints past the row.

use egui::{Align, Layout, Rect, Response, Role, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::avatar::{AvatarSize, paint_on};
use super::button::{Caret, baseline, caret_width, link_caret, link_caret_width, paint_caret};
use super::{Hover, Shown, lit};

/// What a thread row shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThreadLine<'a> {
    /// Who wrote it.
    pub sender: &'a str,
    /// Their address, for the avatar's initials and, open, the line.
    pub address: &'a str,
    /// Folded, the message's first words; open, the address and to whom.
    pub preview: &'a str,
    /// When, as the app writes dates.
    pub date: &'a str,
    /// Whether it is unread.
    pub unread: bool,
    /// Whether it is the open message's head.
    pub open: bool,
}

/// What was pressed on a thread row.
#[derive(Debug, Clone)]
pub struct ThreadPressed {
    /// The row: a click folds or unfolds it.
    pub row: Response,
    /// Whether `details ▾` (open only) was pressed.
    pub details: bool,
}

/// The room left and right of the row's parts, in points.
const PAD: f32 = 12.0;

/// The room between the row's parts, in points.
const GAP: f32 = 10.0;

/// The side of the unread square, in points.
const SQUARE: f32 = 6.0;

/// The width of the sender's column, in points.
const SENDER: f32 = 152.0;

/// What a screen reader hears for `line`.
pub(crate) fn spoken(line: &ThreadLine<'_>) -> String {
    let (sender, date, preview) = (super::spoken(line.sender), super::spoken(line.date), super::spoken(line.preview));
    match (line.open, line.unread) {
        (true, _) => tr!("{sender}, {date}, open: {preview}", sender = sender, date = date, preview = preview),
        (false, true) => tr!("{sender}, {date}, unread, folded: {preview}", sender = sender, date = date, preview = preview),
        (false, false) => tr!("{sender}, {date}, folded: {preview}", sender = sender, date = date, preview = preview),
    }
}

/// A thread row across `ui`'s width showing `line`; what was pressed.
pub fn thread_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, line: &ThreadLine<'_>) -> ThreadPressed {
    thread_row_with(ui, theme, keymap, line, None)
}

/// [`thread_row`] drawn in `shown` whatever the pointer and the keyboard do (the gallery's boards).
pub(crate) fn thread_row_with(ui: &mut Ui, theme: &Theme, keymap: &Keymap, line: &ThreadLine<'_>, shown: Option<Shown>) -> ThreadPressed {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(1.0).floor();
    let [_, _, high, ..] = theme.boxes().reader;
    let (rect, row) = ui.allocate_exact_size(vec2(width, high), Sense::click());
    row.widget_info(|| WidgetInfo::labeled(Role::Button, true, spoken(line)));
    let lit = lit(ui, theme, &row, true, shown, Hover::AtOnce);
    let strong = line.open || line.unread;
    let sender_font = if strong { f.strong } else { f.body };
    let (preview_font, preview_ink) = match (line.open, line.unread) {
        (true, _) => (f.mono_s, p.muted),
        (false, true) => (f.caption, p.ink),
        (false, false) => (f.caption, p.muted),
    };
    let date_ink = if line.unread && !line.open { p.cyan_text } else { p.muted };
    let caret_left = rect.left() + PAD;
    let square_left = caret_left + caret_width(f.mono_s).ceil() + GAP;
    let avatar_left = square_left + SQUARE + GAP;
    let side = AvatarSize::S.side(theme);
    let sender_left = avatar_left + side + GAP;
    let date_right = rect.right() - PAD;
    // The date keeps whole: it may take all the room after the avatar.
    let date = super::fitted(ui, line.date, f.mono_s, date_ink, (date_right - sender_left).max(1.0));
    let date_left = date_right - paint::run_box(date.size().x);
    // The room the sender, the words and `details ▾` share. The words give way first, then `details ▾` whole, then the
    // sender below its column's width.
    let shared = (date_left - GAP - sender_left).max(0.0);
    let link_words = tr!("details");
    let link = if line.open { link_caret_width(ui, theme, link_words) + GAP } else { 0.0 };
    let linked = line.open && shared >= SENDER + link;
    let sender_room = if linked { SENDER } else { SENDER.min(shared) };
    let preview_left = sender_left + SENDER + GAP;
    let preview_right = date_left - GAP - if linked { link } else { 0.0 };
    if ui.is_rect_visible(rect) {
        // Below the least the row holds (the avatar and the date), what does not fit is clipped at the row's edge.
        let painter = &ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        if lit.hover {
            painter.rect_filled(rect, 0.0, p.stage);
        }
        paint::hairline(painter, pos2(rect.left(), rect.bottom() - 1.0), pos2(rect.right() - 1.0, rect.bottom() - 1.0), p.panel_line);
        let middle = rect.center().y;
        let caret = if line.open { Caret::Down } else { Caret::Right };
        paint_caret(painter, f.mono_s, caret_left, baseline(f.mono_s, middle), caret, p.muted);
        if line.unread {
            let square = Rect::from_min_size(pos2(square_left, (middle - SQUARE / 2.0).round()), vec2(SQUARE, SQUARE));
            painter.rect_filled(square, 0.0, p.cyan_edge);
        }
        let face = Rect::from_min_size(pos2(avatar_left, (middle - side / 2.0).round()), vec2(side, side));
        paint_on(painter, theme, face, line.sender, line.address, if lit.hover { p.surface } else { p.stage });
        let top = |high: f32| (middle - high / 2.0).round();
        let sender = super::fitted(ui, line.sender, sender_font, p.ink, sender_room);
        paint::run_at(painter, pos2(sender_left, top(sender.size().y)), sender, p.ink);
        if preview_right > preview_left {
            let preview = super::fitted(ui, line.preview, preview_font, preview_ink, preview_right - preview_left);
            paint::run_at(painter, pos2(preview_left, top(preview.size().y)), preview, preview_ink);
        }
        paint::run_right(painter, date_right, top(date.size().y), date, date_ink);
        if lit.focus {
            let inside = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 1.0));
            paint::focus_ring(painter, theme, inside, Ring::Inside);
        }
    }
    let mut details = false;
    if linked {
        let link_rect = Rect::from_min_max(pos2(date_left - link, rect.top()), pos2(date_left - GAP, rect.bottom()));
        let mut child = ui.new_child(UiBuilder::new().max_rect(link_rect).layout(Layout::left_to_right(Align::Center)));
        details = link_caret(&mut child, theme, keymap, link_words, Caret::Down).clicked();
    }
    ThreadPressed { row, details }
}

#[cfg(test)]
#[path = "thread_row_tests.rs"]
mod tests;
