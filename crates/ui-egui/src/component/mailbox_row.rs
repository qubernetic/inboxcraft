// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A mailbox of the index (`mailbox-row`): 232 × 30 on the `panel`, its icon (16 px) 44 in and its name in `body` 70
//! in (`muted`; `ink` hovered or shown), and at the right, 14 in, how many wait in it in `mono_s` (`muted`;
//! `cyan_text` shown), written by [`super::label::count`]: none for 0, `9 999+` at most. A name that does not fit is
//! cut with one `…` 10 before the count. It stands as a place's row does ([`super::nav_row::base`]). Narrow (56 wide)
//! it shows its icon in the middle (`muted`; `ink` shown) and, while anything waits in it, a 4 px `cyan_edge` square
//! at its top right.

use egui::{Id, Rect, Response, Role, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::theme::Theme;

use super::Shown;
use super::nav_row::{ICON, RIGHT, base, paint_icon};

/// How high a mailbox's row is at 100 % text, in points.
pub const HEIGHT: f32 = 30.0;

/// The icon's left edge, from the row's left, in points.
const ICON_LEFT: f32 = 44.0;

/// The name's left edge, from the row's left, in points.
const NAME_LEFT: f32 = 70.0;

/// The room between the name and the count, in points.
const COUNT_GAP: f32 = 10.0;

/// The narrow row's square: its left edge and top from the row's, and its side, in points.
const SQUARE_LEFT: f32 = 40.0;
const SQUARE_TOP: f32 = 6.0;
const SQUARE: f32 = 4.0;

/// What a mailbox's row shows.
#[derive(Debug, Clone, Copy)]
pub struct MailboxRow<'a> {
    /// Its id: its focus and its response.
    pub id: Id,
    /// What a screen reader hears for it.
    pub spoken: &'a str,
    /// Its icon, a Phosphor glyph.
    pub icon: char,
    /// Its name, already cleaned of control and bidi characters.
    pub name: &'a str,
    /// How many wait in it; 0 shows none.
    pub count: u64,
    /// The mailbox shown.
    pub active: bool,
}

/// The row `row` across `ui`'s width, 56 wide when `narrow`; `shown` draws it hovered or focused whatever the pointer
/// does (the gallery's board).
pub fn mailbox_row(ui: &mut Ui, theme: &Theme, row: &MailboxRow<'_>, narrow: bool, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let drawn = base(ui, theme, row.id, HEIGHT, row.active, true, shown);
    let (spoken, active) = (row.spoken, row.active);
    let enabled = !super::soon::is_soon(ui);
    drawn.response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, active, spoken));
    if !ui.is_rect_visible(drawn.rect) {
        return drawn.response;
    }
    let rect = drawn.rect;
    let ink = super::soon::ink(ui, theme, if row.active || drawn.hovered { p.ink } else { p.muted });
    if narrow {
        paint_icon(ui, &drawn, row.icon, rect.center().x - ICON / 2.0, ink);
        if row.count > 0 {
            let square = Rect::from_min_size(rect.min + vec2(SQUARE_LEFT, SQUARE_TOP), vec2(SQUARE, SQUARE));
            ui.painter().rect_filled(square, 0.0, p.cyan_edge);
        }
        drawn.finish(ui, theme);
        return drawn.response;
    }
    paint_icon(ui, &drawn, row.icon, rect.left() + ICON_LEFT, ink);
    let right = rect.right() - RIGHT;
    let count = super::label::count(row.count);
    let end = if count.is_empty() {
        right
    } else {
        let tone = if row.active { p.cyan_text } else { p.muted };
        let galley = super::galley(ui, &count, f.mono_s, tone);
        paint::run_right(ui.painter(), right, drawn.middle(f.mono_s.line), galley, tone) - COUNT_GAP
    };
    let left = rect.left() + NAME_LEFT;
    let name = super::fitted(ui, row.name, f.body, ink, (end - left).max(0.0));
    paint::run_at(ui.painter(), pos2(left, drawn.middle(f.body.line)), name, ink);
    drawn.finish(ui, theme);
    drawn.response
}

#[cfg(test)]
#[path = "mailbox_row_tests.rs"]
mod tests;
