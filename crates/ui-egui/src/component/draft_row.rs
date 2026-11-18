// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The draft row (`draft-row`, Home and Drafts): 440 × 44 on [`super::row`], one line, its parts 12 apart and 16 in
//! from both ends: the compose mark (16 px, `muted`, `ink` while hovered), the subject in `body` (`ink`; "(no
//! subject)" in `muted`) taking the room left, whom it goes to in `caption` and `muted` (at most 100 wide, cut with one
//! `…`), and when it was last edited in `mono_s` and `muted`. Hovered or keyed, "open" and "discard" (small links, 16
//! apart) stand in the time's place.

use egui::{Id, Rect, Role, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::Shown;
use super::button::{LinkSize, link_with};
use super::row::{Row, RowGround, row_with};

/// The row's height at 100 % text, in points.
pub const HEIGHT: f32 = 44.0;

/// The room at both ends, the gap between the parts and between the links, in points.
const PAD: f32 = 16.0;
const GAP: f32 = 12.0;
const LINK_GAP: f32 = 16.0;

/// The widest whom it goes to is drawn, in points.
const TO_WIDEST: f32 = 100.0;

/// What a draft row shows.
#[derive(Debug, Clone, Copy)]
pub struct DraftRow<'a> {
    /// Its id.
    pub id: Id,
    /// What a screen reader hears for it.
    pub spoken: &'a str,
    /// The subject; `None` when it has none ("(no subject)" in `muted`).
    pub subject: Option<&'a str>,
    /// Whom it goes to: "to Sunniva Aalto", "no recipient yet".
    pub to: &'a str,
    /// When it was last edited: "edited 12:10".
    pub when: &'a str,
    /// The last row of its list.
    pub last: bool,
}

/// What a draft row did this frame.
#[derive(Debug)]
pub struct Drawn {
    /// The row.
    pub row: Row,
    /// "open" was pressed.
    pub open: bool,
    /// "discard" was pressed.
    pub discard: bool,
}

/// The draft row `row` across `ui`'s width; `keyed` shows the links as a hovered row does; `shown` draws it hovered or
/// focused whatever the pointer does (the gallery's board).
pub fn draft_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: &DraftRow<'_>, keyed: bool, shown: Option<Shown>) -> Drawn {
    let p = &theme.palette;
    let f = &theme.fonts;
    let drawn = row_with(ui, theme, row.id, HEIGHT, RowGround::Surface, false, row.last, shown);
    let spoken = row.spoken;
    drawn.response.widget_info(|| WidgetInfo::labeled(Role::ListItem, true, spoken));
    let rect = drawn.rect;
    let (mut open, mut discard) = (false, false);
    if !ui.is_rect_visible(rect) {
        return Drawn { row: drawn, open, discard };
    }
    // Without the line under it, the row's middle: where its one line stands.
    let middle = rect.top() + (rect.height() - 1.0) / 2.0;
    let at = |high: f32| (middle - high / 2.0).round();
    let side = theme.boxes().icon;
    let lit = drawn.hovered || keyed;
    let mark = if drawn.hovered { p.ink } else { p.muted };
    crate::iconset::paint(ui.painter(), Rect::from_min_size(pos2(rect.left() + PAD, at(side)), vec2(side, side)), IconId::Compose, mark);
    let mut right = rect.right() - PAD;
    if lit {
        let high = theme.boxes().row_link;
        let words = [crate::tr!("open"), crate::tr!("discard")];
        let wides: Vec<f32> = words.iter().map(|words| super::galley(ui, words, f.mono_s, p.cyan_text).size().x.ceil()).collect();
        let total = wides.iter().sum::<f32>() + LINK_GAP;
        let mut x = right - total;
        for (index, (words, wide)) in words.iter().zip(&wides).enumerate() {
            let place = Rect::from_min_size(pos2(x, at(high)), vec2(*wide, high));
            let clicked = ui
                .scope_builder(UiBuilder::new().max_rect(place).id_salt(index), |ui| {
                    link_with(ui, theme, keymap, words, LinkSize::Small, true, None).clicked()
                })
                .inner;
            if index == 0 {
                open = clicked;
            } else {
                discard = clicked;
            }
            x += wide + LINK_GAP;
        }
        right -= total;
    } else {
        let galley = super::galley(ui, row.when, f.mono_s, p.muted);
        right -= paint::run_box(galley.size().x);
        paint::run_at(ui.painter(), pos2(right, at(f.mono_s.line)), galley, p.muted);
    }
    let to = super::fitted(ui, row.to, f.caption, p.muted, TO_WIDEST);
    right -= GAP + paint::run_box(to.size().x);
    paint::run_at(ui.painter(), pos2(right, at(f.caption.line)), to, p.muted);
    let left = rect.left() + PAD + side + GAP;
    let (words, ink) = match row.subject {
        Some(subject) => (subject, p.ink),
        None => (crate::tr!("(no subject)"), p.muted),
    };
    let subject = super::fitted(ui, words, f.body, ink, (right - GAP - left).max(0.0));
    paint::run_at(ui.painter(), pos2(left, at(f.body.line)), subject, ink);
    drawn.finish(ui, theme);
    Drawn { row: drawn, open, discard }
}

#[cfg(test)]
#[path = "draft_row_tests.rs"]
mod tests;
