// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The row of Later (`later-row`): the mail row's 440 × 56 and its columns (the current row's bar, the unread square,
//! the text 34 in). Line one: who sent it in `body` (`strong` when unread), the account bar, a tag (Returned), and at
//! the right the pile's mark, 14 px: the snoozed pile's clock and a returned row's arrow in `warn`, the pin in `ink`,
//! the park pile's pause in `muted`. Line two: the subject in `caption` (`ink` when unread) and, 12 right of it at the
//! line's end, when the mail comes back in `mono_s` (`warn`) or the message's own date (`muted`). A snoozed row the
//! pointer is over, or the keyboard is on (keyed or focused), shows "bring back now" in the date's place, and `Tab`
//! goes on to it. The sender's room is what the columns leave (346 with the account bar), not the sheet's maxW of
//! 355, which would overrun the 390 text frame by 9.

use egui::{Color32, Id, Rect, Role, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::Shown;
use super::button::{LinkSize, link_with};
use super::mail_row::{HEIGHT, RIGHT, TEXT_LEFT, Top, lines_top, top};
use super::row::{Row, RowGround, row_with};
use super::tag::TagKind;

/// Where the unread square stands from the row's left edge, and its side, in points.
const SQUARE_LEFT: f32 = 16.0;
const SQUARE: f32 = 6.0;

/// The gap between the subject and when, in points.
const WHEN_GAP: f32 = 12.0;

/// The pile a row of Later stands in, as its mark says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pile {
    /// Snoozed: it comes back at a time.
    Snoozed,
    /// Pinned.
    Pinned,
    /// Parked, without a time.
    Parked,
    /// Back from a snooze, unread.
    Returned,
}

impl Pile {
    /// Its mark and colour, and the colour of when, in `theme`.
    #[must_use]
    pub fn look(self, theme: &Theme) -> (IconId, Color32, Color32) {
        let p = &theme.palette;
        match self {
            Self::Snoozed => (IconId::Snooze, p.warn, p.warn),
            Self::Pinned => (IconId::Pin, p.ink, p.muted),
            Self::Parked => (IconId::Pause, p.muted, p.muted),
            Self::Returned => (IconId::CameBack, p.warn, p.muted),
        }
    }
}

/// What a row of Later shows.
#[derive(Debug, Clone, Copy)]
pub struct LaterRow<'a> {
    /// Its id: its focus and its response.
    pub id: Id,
    /// What a screen reader hears for it.
    pub spoken: &'a str,
    /// Who sent it.
    pub who: &'a str,
    /// The subject.
    pub subject: &'a str,
    /// When it comes back, or the message's own date.
    pub when: &'a str,
    /// Unread.
    pub unread: bool,
    /// The row open in the reader.
    pub current: bool,
    /// The last row of its list.
    pub last: bool,
    /// The account counted from 0, in a list that mixes accounts.
    pub account: Option<u64>,
    /// Its pile.
    pub pile: Pile,
    /// The link a snoozed row shows while hovered or keyed ("bring back now").
    pub link: Option<&'a str>,
}

/// What a row of Later did this frame.
#[derive(Debug)]
pub struct Drawn {
    /// The row: where it is, its response, hovered or focused.
    pub row: Row,
    /// Its link was pressed.
    pub link: bool,
}

/// The row `row` across `ui`'s width; `keyed`, the row the keyboard moved to, shows the link as a hovered one does;
/// `shown` draws it hovered or focused whatever the pointer does (the gallery's board).
pub fn later_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: &LaterRow<'_>, keyed: bool, shown: Option<Shown>) -> Drawn {
    let p = &theme.palette;
    let f = &theme.fonts;
    // Taken before the row asks for the focus: in the frame `Tab` leaves the row it has given the focus away, and its
    // link must be there to take it.
    let had_focus = ui.memory(|memory| memory.has_focus(row.id));
    let drawn = row_with(ui, theme, row.id, HEIGHT, RowGround::Surface, row.current, row.last, shown);
    let spoken = row.spoken;
    drawn.response.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, row.current, spoken));
    let rect = drawn.rect;
    if !ui.is_rect_visible(rect) {
        return Drawn { row: drawn, link: false };
    }
    if row.unread {
        let square =
            Rect::from_min_size(pos2(rect.left() + SQUARE_LEFT, rect.top() + ((rect.height() - SQUARE) / 2.0).round()), vec2(SQUARE, SQUARE));
        ui.painter().rect_filled(square, 0.0, p.cyan_edge);
    }
    let (one, two) = lines_top(theme, rect.top(), rect.height());
    let left = rect.left() + TEXT_LEFT;
    let right = rect.right() - RIGHT;
    let (icon, mark, when_ink) = row.pile.look(theme);
    let font = if row.unread { f.strong } else { f.body };
    let tag = (row.pile == Pile::Returned).then_some((crate::tr!("Returned"), TagKind::Warn));
    let marks = [(icon, mark)];
    let first = Top { name: row.who, font, ink: p.ink, account: row.account, tag, marks: &marks, time: None };
    top(ui, theme, Rect::from_min_max(pos2(left, one), pos2(right, one + f.body.line)), &first, None);
    // Line two: the subject, and at its end when, or the link in its place.
    let low = f.caption.line;
    // Where the link's own id is kept: while it has the focus, it stays.
    let kept = row.id.with("link-id");
    let link_focused = ui.data(|data| data.get_temp::<Id>(kept)).is_some_and(|id| ui.memory(|memory| memory.has_focus(id)));
    let link = row.link.filter(|_| drawn.hovered || drawn.focused || had_focus || link_focused || keyed);
    let mut pressed = false;
    let end = if let Some(words) = link {
        let wide = super::galley(ui, words, f.mono_s, p.cyan_text).size().x.ceil();
        let high = theme.boxes().row_link;
        let at = Rect::from_min_size(pos2(right - wide, (two + (low - high) / 2.0).round()), vec2(wide, high));
        pressed = ui
            .scope_builder(UiBuilder::new().max_rect(at).id_salt("link"), |ui| {
                let response = link_with(ui, theme, keymap, words, LinkSize::Small, true, None);
                ui.data_mut(|data| data.insert_temp(kept, response.id));
                response.clicked()
            })
            .inner;
        at.left()
    } else {
        let galley = super::galley(ui, row.when, f.mono_s, when_ink);
        paint::run_right(ui.painter(), right, two + ((low - f.mono_s.line) / 2.0).round(), galley, when_ink)
    };
    let words = if row.unread { p.ink } else { p.muted };
    let subject = super::fitted(ui, row.subject, f.caption, words, (end - WHEN_GAP - left).max(0.0));
    paint::run_at(ui.painter(), pos2(left, two), subject, words);
    drawn.finish(ui, theme);
    Drawn { row: drawn, link: pressed }
}

#[cfg(test)]
#[path = "later_row_tests.rs"]
mod tests;
