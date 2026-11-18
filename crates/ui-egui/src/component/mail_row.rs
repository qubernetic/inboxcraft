// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mail row of the design (`mail-row`): 440 × 56 on [`super::row`], two lines of text 34 in from its left edge.
//! Line one: the sender in `body` (`strong` when unread), the account bar 8 after it in a list that mixes accounts, a
//! tag 8 after that (New sender, Returned, Not delivered), and at the right the marks (14 px: the paperclip and the
//! invitation's calendar `muted`, the flag and the pin `ink`, the snooze `warn`, the done row's tick `muted`) 8 before
//! the time in `mono_s` (`cyan_text` when unread, `warn` for a snoozed row's "back …"). Line two: the subject and the
//! preview in `caption`, one line, `ink` when unread and `muted` when read. Unread is a 6 px `cyan_edge` square 16 in
//! from the left, on the row's middle. Each line is cut with one `…`; the sender is cut short of what stands after it.
//!
//! A hovered row and the row the keyboard moved to show the hover actions on line one: hovered, they take the time's
//! place, their right edge on the text's; keyed, they stand left of the time, which stays. The marks stay left of
//! them. A checked row (bulk select, drawn and coming soon in this stage) shows the `checkbox` set in the first column
//! on the `stage` fill.
//!
//! [`top`] lays out line one for the later row and the outbox row too: the same columns, the same gaps.

use egui::{Color32, Id, Rect, Response, Role, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::{Font, Theme};

use super::Shown;
use super::hover::{self, Action};
use super::row::{Row, RowGround, row_with};
use super::tag::{self, TagKind};

/// The row's height at 100 % text, in points.
pub const HEIGHT: f32 = 56.0;

/// Where the text starts, from the row's left edge, in points: the unread square's column and the gap after it.
pub const TEXT_LEFT: f32 = 34.0;

/// The room right of the text, in points.
pub const RIGHT: f32 = 16.0;

/// Where the unread square stands from the row's left edge, and its side, in points.
const SQUARE_LEFT: f32 = 16.0;
const SQUARE: f32 = 6.0;

/// Where the check box stands from the row's left edge, in points (the sheet's `checked` state).
const CHECK_LEFT: f32 = 11.0;

/// The gap between the parts of line one, in points.
pub const GAP: f32 = 8.0;

/// The least room the sheet's spacer keeps between the left part of line one and its right part, in points.
const SPACER: f32 = 3.0;

/// The gap between the two lines, in points.
const LINE_GAP: f32 = 2.0;

/// The account bar's size, in points.
const BAR: egui::Vec2 = egui::vec2(3.0, 12.0);

/// A mark on line one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Done, under "Show done": the tick, `muted`.
    Done,
    /// It carries attachments: the paperclip, `muted`.
    Attachment,
    /// It carries an invitation: `calendar-blank`, `muted`.
    Invitation,
    /// Flagged: the flag, `ink`.
    Flag,
    /// Pinned: the pin, `ink`.
    Pin,
    /// Snoozed: the clock, `warn` (the time says when it comes back, `warn` too).
    Snoozed,
}

impl Mark {
    /// Its icon and colour in `theme`.
    #[must_use]
    pub fn look(self, theme: &Theme) -> (IconId, Color32) {
        let p = &theme.palette;
        match self {
            Self::Done => (IconId::Check, p.muted),
            Self::Attachment => (IconId::Attach, p.muted),
            Self::Invitation => (IconId::Calendar, p.muted),
            Self::Flag => (IconId::Flag, p.ink),
            Self::Pin => (IconId::Pin, p.ink),
            Self::Snoozed => (IconId::Snooze, p.warn),
        }
    }
}

/// What a mail row shows.
#[derive(Debug, Clone, Copy)]
pub struct MailRow<'a> {
    /// Its id: its focus and its response.
    pub id: Id,
    /// What a screen reader hears for it.
    pub spoken: &'a str,
    /// The sender (or "To …" for mail the owner sent).
    pub sender: &'a str,
    /// The subject and the preview, as one line.
    pub line: &'a str,
    /// The time.
    pub time: &'a str,
    /// Unread: the square, the sender in `strong`, line two and the time in their unread colours.
    pub unread: bool,
    /// The row open in the reader: the `stage` fill and the bar.
    pub current: bool,
    /// The last row of its list: no line under it.
    pub last: bool,
    /// The account counted from 0, in a list that mixes accounts: its bar after the sender.
    pub account: Option<u64>,
    /// A tag after the sender (and the bar).
    pub tag: Option<(&'a str, TagKind)>,
    /// The marks before the time, left to right.
    pub marks: &'a [Mark],
    /// Done, under "Show done": the sender in `muted`.
    pub done: bool,
    /// Checked for bulk select: the check box in the first column, on the `stage` fill.
    pub checked: bool,
}

/// What a mail row did this frame.
#[derive(Debug)]
pub struct Drawn {
    /// The row: where it is, its response, hovered or focused. Its focus edge is painted.
    pub row: Row,
    /// The hover action pressed, by its place in the list given.
    pub acted: Option<usize>,
}

/// The two lines of a row of `high` points from `top`: line one's top, and line two's; both centred in the row as the
/// sheet's text frame is (7 under the top of a 56 row).
#[must_use]
pub fn lines_top(theme: &Theme, top: f32, high: f32) -> (f32, f32) {
    let f = &theme.fonts;
    let first = f.body.line;
    let block = first + LINE_GAP + f.caption.line;
    let one = top + ((high - block) / 2.0).round();
    (one, one + first + LINE_GAP)
}

/// The mail row `row` across `ui`'s width. `keyed`: the row the keyboard moved to, which shows `actions` as a hovered
/// row does; `shown` draws hovered or focused whatever the pointer does (the gallery's board).
pub fn mail_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: &MailRow<'_>, keyed: bool, actions: &[Action<'_>], shown: Option<Shown>) -> Drawn {
    let drawn = row_with(ui, theme, row.id, HEIGHT, RowGround::Surface, row.current, row.last, shown);
    let spoken = row.spoken;
    drawn.response.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, row.current, spoken));
    if !ui.is_rect_visible(drawn.rect) {
        return Drawn { row: drawn, acted: None };
    }
    let acted = content(ui, theme, keymap, row, &drawn, keyed, actions);
    drawn.finish(ui, theme);
    Drawn { row: drawn, acted }
}

/// Paints the row's content into `drawn`'s rect; the hover action pressed.
fn content(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: &MailRow<'_>, drawn: &Row, keyed: bool, actions: &[Action<'_>]) -> Option<usize> {
    let p = &theme.palette;
    let f = &theme.fonts;
    let rect = drawn.rect;
    let high = rect.height();
    if row.checked && !row.current && !drawn.hovered {
        // The sheet's checked row stands on `stage`, its line under it painted again over the fill.
        ui.painter().rect_filled(rect, 0.0, p.stage);
        if !row.last && high >= 2.0 {
            let y = rect.bottom() - 1.0;
            paint::hairline(ui.painter(), pos2(rect.left(), y), pos2(rect.right() - 1.0, y), p.panel_line);
        }
    }
    if row.checked {
        check(ui, theme, rect);
    } else if row.unread {
        let square = Rect::from_min_size(pos2(rect.left() + SQUARE_LEFT, rect.top() + ((high - SQUARE) / 2.0).round()), vec2(SQUARE, SQUARE));
        ui.painter().rect_filled(square, 0.0, p.cyan_edge);
    }
    let (one, two) = lines_top(theme, rect.top(), high);
    let left = rect.left() + TEXT_LEFT;
    let right = rect.right() - RIGHT;
    let line = Rect::from_min_max(pos2(left, one), pos2(right, one + f.body.line));
    let hovered = drawn.hovered;
    let reach = (hovered || keyed) && !actions.is_empty();
    let snoozed = row.marks.contains(&Mark::Snoozed);
    let time_ink = if snoozed {
        p.warn
    } else if row.unread {
        p.cyan_text
    } else {
        p.muted
    };
    // Hovered, the actions take the time's place; keyed, they stand left of the time, which stays.
    let time = (!hovered || !reach).then_some((row.time, time_ink));
    let time_left = time.map_or(right, |(time, _)| right - paint::run_box(super::galley(ui, time, f.mono_s, time_ink).size().x));
    let actions_line = reach.then(|| {
        let end = if time.is_some() { time_left - GAP } else { right };
        Rect::from_min_max(pos2(rect.center().x + theme.spacing.s1, one), pos2(end.max(rect.center().x + theme.spacing.s1), one + f.body.line))
    });
    let group = actions_line.map(|at| hover::place(ui, theme, keymap, at, actions)).filter(|group| group.width() > 0.0);
    let marks: Vec<(IconId, Color32)> = row.marks.iter().map(|mark| mark.look(theme)).collect();
    let font = if row.unread { f.strong } else { f.body };
    let ink = if row.done { p.muted } else { p.ink };
    let top_line = Top { name: row.sender, font, ink, account: row.account, tag: row.tag, marks: &marks, time };
    self::top(ui, theme, line, &top_line, group.map(|group| group.left() - GAP));
    let words = if row.unread { p.ink } else { p.muted };
    let text = super::fitted(ui, row.line, f.caption, words, right - left);
    paint::run_at(ui.painter(), pos2(left, two), text, words);
    // Last, so the group stands over whatever of the row lies under it.
    let shown = hover::actions(ui, theme, keymap, actions_id(row.id), actions_line.unwrap_or(line), reach, actions);
    shown.clicked
}

/// The id of the hover action group of the row `row`: its n-th action is `actions_id(row).with(n)`.
#[must_use]
pub fn actions_id(row: Id) -> Id {
    row.with("actions")
}

/// The check box of a checked row in the first column: bulk select is drawn and coming soon in this stage.
fn check(ui: &mut Ui, theme: &Theme, rect: Rect) {
    let side = theme.boxes().check;
    let line = theme.fonts.mono.line.max(side);
    let top = rect.top() + ((rect.height() - side) / 2.0).round() - (line - side) / 2.0;
    let room = Rect::from_min_size(pos2(rect.left() + CHECK_LEFT, top), vec2(side, line));
    ui.scope_builder(UiBuilder::new().max_rect(room), |ui| {
        super::soon::soon(ui, theme, |ui| {
            let response: Response = super::checkbox::checkbox(ui, theme, "", true);
            response.widget_info(|| WidgetInfo::selected(Role::CheckBox, false, true, crate::tr!("Select")));
        });
    });
}

/// What stands on line one of a list row, left to right: a name, the account bar, a tag; at the right its marks and
/// a time.
#[derive(Debug, Clone, Copy)]
pub struct Top<'a> {
    /// The name (a sender, "To …").
    pub name: &'a str,
    /// Its style.
    pub font: Font,
    /// Its colour.
    pub ink: Color32,
    /// The account counted from 0: its bar 8 after the name.
    pub account: Option<u64>,
    /// A tag 8 after the name or the bar.
    pub tag: Option<(&'a str, TagKind)>,
    /// The marks before the time, left to right, each 14 px, 8 apart.
    pub marks: &'a [(IconId, Color32)],
    /// The time in `mono_s` at the line's right end, and its colour.
    pub time: Option<(&'a str, Color32)>,
}

/// Paints `top` on `line` (the text's width, as high as `body`'s line). What stands at the right ends at `end` when
/// given (the hover actions' left edge less the gap), else at the line's right end; the name is cut short of what
/// stands after it, the sheet's spacer (3) kept.
pub fn top(ui: &mut Ui, theme: &Theme, line: Rect, top: &Top<'_>, end: Option<f32>) {
    spaced(ui, theme, line, top, end, SPACER);
}

/// [`top`] with the least room `spacer` between the left part and the right one. A name cut short keeps its whole
/// room, as the design's box at its greatest width does: the bar and the tag stand 8 after the room, not after the
/// cut words.
pub(crate) fn spaced(ui: &mut Ui, theme: &Theme, line: Rect, top: &Top<'_>, end: Option<f32>, spacer: f32) {
    let f = &theme.fonts;
    let painter = ui.painter().clone();
    let high = line.height();
    let middle = |side: f32| line.top() + ((high - side) / 2.0).round();
    // `right`: where the next part from the right ends; after the parts, a gap left of the leftmost of them.
    let mut right = line.right();
    if let Some((time, ink)) = top.time {
        right = paint::run_right(&painter, right, middle(f.mono_s.line), super::galley(ui, time, f.mono_s, ink), ink) - GAP;
    }
    if let Some(end) = end {
        right = right.min(end);
    }
    let side = theme.mark_side();
    for (icon, ink) in top.marks.iter().rev() {
        let at = Rect::from_min_size(pos2(right - side, middle(side)), vec2(side, side));
        crate::iconset::paint(&painter, at, *icon, *ink);
        right -= side + GAP;
    }
    // `right` now stands a gap left of the right part, or at the line's end when it holds nothing: the spacer and a gap
    // before it either way, as the sheet's auto layout puts a gap after the spacer.
    let tag = top.tag.map(|(words, kind)| (words, kind, paint::run_box(super::galley(ui, words, f.mono_s, Color32::PLACEHOLDER).size().x) + 12.0));
    let after = top.account.map_or(0.0, |_| GAP + BAR.x) + tag.map_or(0.0, |(_, _, wide)| GAP + wide);
    let room = (right - spacer - GAP - line.left() - after).max(0.0);
    let whole = super::plain(top.name);
    let kept = super::fit_capped(ui, &whole, top.font, room);
    let cut = kept != whole;
    let name = super::galley(ui, &kept, top.font, top.ink);
    let wide = if cut { room.floor() } else { paint::run_box(name.size().x) };
    paint::run_at(&painter, line.left_top(), name, top.ink);
    let mut x = line.left() + wide;
    if let Some(account) = top.account {
        x += GAP;
        painter.rect_filled(Rect::from_min_size(pos2(x, middle(BAR.y)), BAR), 0.0, theme.account_colour(account));
        x += BAR.x;
    }
    if let Some((words, kind, wide)) = tag {
        x += GAP;
        let chip = theme.boxes().chip;
        let at = Rect::from_min_size(pos2(x, middle(chip)), vec2(wide, chip));
        ui.scope_builder(UiBuilder::new().max_rect(at), |ui| tag::tag(ui, theme, words, kind));
    }
}

/// What a screen reader hears for a row of `sender` and `line` at `time`, unread or not.
#[must_use]
pub fn spoken(sender: &str, line: &str, time: &str, unread: bool) -> String {
    let (sender, line) = (super::spoken(sender), super::spoken(line));
    if unread {
        crate::tr!("{sender}, {line}, {time}, unread", sender = sender, line = line, time = time)
    } else {
        crate::tr!("{sender}, {line}, {time}", sender = sender, line = line, time = time)
    }
}

#[cfg(test)]
#[path = "mail_row_tests.rs"]
mod tests;
