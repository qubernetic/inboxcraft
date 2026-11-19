// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An account of the index (`account-row`): 232 × 28 on the `panel`, the account's bar (3 × 12 in its colour) in a
//! 16 px box 44 in, its name in `mono_s` 70 in (`muted`; `ink` hovered, open or refused), and a 12 px caret 14 from
//! the right (right while closed, down while open; `muted`, `ink` hovered, open or refused). An account that is
//! syncing, offline or refused says so between the name and the caret, 10 before it: a ring of eight dots in `warn`
//! and "syncing" in `ink`, a `warn` ○ and "offline" in `ink`, or a `minus` ● and "refused" in `minus`. The name is cut
//! with one `…` before what follows it. It stands as a place's row does ([`super::nav_row::base`]). Narrow (56 wide)
//! it shows its bar, with the state's mark 4 right of it, in the middle.

use egui::{Color32, Id, Rect, Response, Role, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::{self, IconId};
use crate::theme::Theme;

use super::Shown;
use super::nav_row::{RIGHT, base};

/// How high an account's row is at 100 % text, in points.
pub const HEIGHT: f32 = 28.0;

/// The bar's box's left edge from the row's left, and its side, in points.
const BOX_LEFT: f32 = 44.0;
const BOX: f32 = 16.0;

/// The account bar's size, in points.
const BAR: egui::Vec2 = vec2(3.0, 12.0);

/// The name's left edge, from the row's left, in points.
const NAME_LEFT: f32 = 70.0;

/// The caret's side, in points.
const CARET: f32 = 12.0;

/// The room before the caret and between the name and the state, in points.
const GAP: f32 = 10.0;

/// The room between a state's mark and its word, and between the narrow bar and its mark, in points.
const MARK_GAP: f32 = 4.0;

/// The side of the syncing ring, in points.
const RING: f32 = 10.0;

/// Where the syncing ring's eight dots stand in its 10 × 10 box, each 2 × 2.
const DOTS: [(f32, f32); 8] = [(4.0, 0.0), (7.0, 1.0), (8.0, 4.0), (7.0, 7.0), (4.0, 8.0), (1.0, 7.0), (0.0, 4.0), (1.0, 1.0)];

/// How an account is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sync {
    /// Nothing to say.
    Quiet,
    /// Fetching or sending now.
    Syncing,
    /// Its server cannot be reached.
    Offline,
    /// Its server refused the sign-in.
    Refused,
}

/// What an account's row shows.
#[derive(Debug, Clone, Copy)]
pub struct AccountRow<'a> {
    /// Its id: its focus and its response.
    pub id: Id,
    /// What a screen reader hears for it.
    pub spoken: &'a str,
    /// Its name, already cleaned of control and bidi characters.
    pub name: &'a str,
    /// The account counted from 0: its bar's colour.
    pub account: u64,
    /// Its folders shown under it.
    pub open: bool,
    /// How it is.
    pub sync: Sync,
}

/// The ring of eight dots in `colour` in the 10 × 10 box at `at`.
fn ring(ui: &Ui, at: egui::Pos2, colour: Color32) {
    for (x, y) in DOTS {
        ui.painter().rect_filled(Rect::from_min_size(at + vec2(x, y), vec2(2.0, 2.0)), 0.0, colour);
    }
}

/// The row `row` across `ui`'s width, 56 wide when `narrow`; `shown` draws it hovered or focused whatever the pointer
/// does (the gallery's board).
pub fn account_row(ui: &mut Ui, theme: &Theme, row: &AccountRow<'_>, narrow: bool, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let drawn = base(ui, theme, row.id, HEIGHT, false, true, shown);
    let spoken = row.spoken;
    let enabled = !super::soon::is_soon(ui);
    drawn.response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, row.open, spoken));
    if !ui.is_rect_visible(drawn.rect) {
        return drawn.response;
    }
    let rect = drawn.rect;
    let colour = theme.account_colour(row.account);
    let bar_top = drawn.middle(BAR.y);
    let mark = match row.sync {
        Sync::Quiet | Sync::Syncing => None,
        Sync::Offline => Some(("○", p.warn)),
        Sync::Refused => Some(("●", p.minus)),
    };
    if narrow {
        let mark_wide = match (row.sync, mark) {
            (Sync::Syncing, _) => RING,
            (_, Some((glyph, tone))) => super::galley(ui, glyph, f.mono_s, tone).size().x.ceil(),
            _ => 0.0,
        };
        let whole = if mark_wide > 0.0 { BAR.x + MARK_GAP + mark_wide } else { BAR.x };
        let left = rect.left() + ((rect.width() - whole) / 2.0).floor();
        ui.painter().rect_filled(Rect::from_min_size(pos2(left, bar_top), BAR), 0.0, colour);
        let after = left + BAR.x + MARK_GAP;
        if row.sync == Sync::Syncing {
            ring(ui, pos2(after, drawn.middle(RING)), p.warn);
        } else if let Some((glyph, tone)) = mark {
            paint::run_at(ui.painter(), pos2(after, drawn.middle(f.mono_s.line)), super::galley(ui, glyph, f.mono_s, tone), tone);
        }
        drawn.finish(ui, theme);
        return drawn.response;
    }
    let bar_left = rect.left() + BOX_LEFT + ((BOX - BAR.x) / 2.0).floor();
    ui.painter().rect_filled(Rect::from_min_size(pos2(bar_left, bar_top), BAR), 0.0, colour);
    let strong = drawn.hovered || row.open || row.sync == Sync::Refused;
    let ink = super::soon::ink(ui, theme, if strong { p.ink } else { p.muted });
    let caret_left = rect.right() - RIGHT - CARET;
    let caret = if row.open { iconset::glyph(IconId::Caret) } else { iconset::glyph(IconId::Next) };
    iconset::paint_glyph(ui.painter(), Rect::from_min_size(pos2(caret_left, drawn.middle(CARET)), vec2(CARET, CARET)), caret, ink);
    let top = drawn.middle(f.mono_s.line);
    let mut end = caret_left - GAP;
    let note = match row.sync {
        Sync::Quiet => None,
        Sync::Syncing => Some((crate::tr!("syncing"), p.ink)),
        Sync::Offline => Some((crate::tr!("offline"), p.ink)),
        Sync::Refused => Some((crate::tr!("refused"), p.minus)),
    };
    if let Some((word, tone)) = note {
        let left = paint::run_right(ui.painter(), end, top, super::galley(ui, word, f.mono_s, tone), tone) - MARK_GAP;
        end = if row.sync == Sync::Syncing {
            ring(ui, pos2(left - RING, drawn.middle(RING)), p.warn);
            left - RING
        } else if let Some((glyph, mark_tone)) = mark {
            paint::run_right(ui.painter(), left, top, super::galley(ui, glyph, f.mono_s, mark_tone), mark_tone)
        } else {
            left
        } - GAP;
    }
    let left = rect.left() + NAME_LEFT;
    let name = super::fitted(ui, row.name, f.mono_s, ink, (end - left).max(0.0));
    paint::run_at(ui.painter(), pos2(left, top), name, ink);
    drawn.finish(ui, theme);
    drawn.response
}

#[cfg(test)]
#[path = "account_row_tests.rs"]
mod tests;
