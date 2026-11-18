// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The outbox row (`outbox-row`): 440 × 56 on [`super::row`], a refused send 72. A 16 px mark 16 in from the left
//! says where the send stands (the countdown `warn`, a scheduled send's clock `cyan_text`, sending `muted`, refused
//! `minus`); 44 in, line one has whom it goes to in `body`, the account bar, and where it stands in `mono_s` at the right
//! in the mark's colour; line two the subject in `caption` and `muted`. The secondary buttons stand 12 right of the
//! text, 8 apart, 16 in from the right: Undo while the send is held, Edit for a scheduled one, Send again and Edit for a
//! refused one. A held send draws its hold as a `cyan_edge` bar along the row's foot, as long as the share left, 2 px
//! high with the row's line drawn over its lower pixel row, as the design draws it;
//! a refused one has the server's reply under the rest, `caption` in `minus`.

use egui::{Color32, Id, Rect, Role, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::Shown;
use super::button::TextButton;
use super::mail_row::{Top, lines_top, top};
use super::row::{RowGround, row_with};

/// The row's height at 100 % text, and a refused send's, in points.
pub const HEIGHT: f32 = 56.0;
pub const REFUSED_HEIGHT: f32 = 72.0;

/// Where the mark and the text start from the row's left edge, and the room right of the buttons, in points.
const MARK_LEFT: f32 = 16.0;
const TEXT_LEFT: f32 = 44.0;
const RIGHT: f32 = 16.0;

/// The gap between the text and the buttons, and between two buttons, in points.
const BUTTONS_GAP: f32 = 12.0;
const BUTTON_GAP: f32 = 8.0;

/// The height of the countdown's bar, in points.
const BAR: f32 = 2.0;

/// The gap between the upper part of a refused row and the reply, in points.
const REPLY_GAP: f32 = 2.0;

/// Where a send stands, as its row draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Standing {
    /// Held before it goes, `share` of the hold left (0 to 1): it can be taken back.
    Countdown {
        /// The share of the hold left.
        share: f32,
    },
    /// Scheduled for a time: it can be taken back and edited.
    Scheduled,
    /// On its way.
    Sending,
    /// The server refused it.
    Refused,
}

impl Standing {
    /// Its mark and colour in `theme`.
    #[must_use]
    pub fn look(self, theme: &Theme) -> (IconId, Color32) {
        let p = &theme.palette;
        match self {
            Self::Countdown { .. } => (IconId::Countdown, p.warn),
            Self::Scheduled => (IconId::Snooze, p.cyan_text),
            Self::Sending => (IconId::Send, p.muted),
            Self::Refused => (IconId::Failed, p.minus),
        }
    }
}

/// A button of an outbox row: its words and what a screen reader hears for it.
#[derive(Debug, Clone, Copy)]
pub struct SendButton<'a> {
    /// Its words.
    pub name: &'a str,
    /// What a screen reader hears.
    pub spoken: &'a str,
}

/// What an outbox row shows.
#[derive(Debug, Clone, Copy)]
pub struct OutboxRow<'a> {
    /// Its id.
    pub id: Id,
    /// What a screen reader hears for it.
    pub spoken: &'a str,
    /// Whom it goes to: "To Odette Marchand".
    pub to: &'a str,
    /// The account counted from 0, in a list that mixes accounts.
    pub account: Option<u64>,
    /// The subject.
    pub subject: &'a str,
    /// Where it stands, in words: "sending in 4 s", "Tue 08:00", "not sent".
    pub words: &'a str,
    /// Where it stands.
    pub standing: Standing,
    /// The server's reply, for a refused send.
    pub reply: Option<&'a str>,
    /// Its buttons, left to right.
    pub buttons: &'a [SendButton<'a>],
    /// The last row of its list: no line under it.
    pub last: bool,
}

/// The height of a row, at 100 % text: a refused send's is taller.
#[must_use]
pub fn height(standing: Standing) -> f32 {
    if standing == Standing::Refused { REFUSED_HEIGHT } else { HEIGHT }
}

/// The outbox row `row` across `ui`'s width; `shown` draws it hovered or focused whatever the pointer does (the
/// gallery's board). The button pressed, by its place.
pub fn outbox_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: &OutboxRow<'_>, shown: Option<Shown>) -> Option<usize> {
    let p = &theme.palette;
    let f = &theme.fonts;
    let mut drawn = row_with(ui, theme, row.id, height(row.standing), RowGround::Surface, false, row.last, shown);
    // The focus edge stays above the hold's bar (the sheet's 2,2 436 × 50), on every row of the outbox alike.
    drawn.foot = BAR;
    let spoken = row.spoken;
    drawn.response.widget_info(|| WidgetInfo::labeled(Role::ListItem, true, spoken));
    let rect = drawn.rect;
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let (icon, ink) = row.standing.look(theme);
    let refused = row.reply.is_some();
    let upper = f.body.line + 2.0 + f.caption.line;
    // The stack's top: the upper part and, for a refused send, its reply under it, centred in the row.
    let block = if refused { upper + REPLY_GAP + f.caption.line } else { upper };
    let stack_top = rect.top() + ((rect.height() - block) / 2.0).round();
    let (one, two) = lines_top(theme, stack_top, upper);
    let side = theme.boxes().icon;
    let mark = Rect::from_min_size(pos2(rect.left() + MARK_LEFT, stack_top + ((upper - side) / 2.0).round()), vec2(side, side));
    crate::iconset::paint(ui.painter(), mark, icon, ink);
    // The buttons from the right.
    let high = theme.boxes().button;
    let mut right = rect.right() - RIGHT;
    let mut pressed = None;
    if !row.buttons.is_empty() {
        let y = stack_top + ((upper - high) / 2.0).round();
        let band = Rect::from_min_max(pos2(rect.left() + TEXT_LEFT, y), pos2(right, y + high));
        let layout = egui::Layout::right_to_left(egui::Align::Center);
        let placed = ui.scope_builder(UiBuilder::new().max_rect(band).layout(layout), |ui| {
            ui.spacing_mut().item_spacing.x = BUTTON_GAP;
            for (index, button) in row.buttons.iter().enumerate().rev() {
                let response = TextButton::new(button.name).secondary().show(ui, theme, keymap);
                let said = button.spoken;
                ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(said));
                if response.clicked() {
                    pressed = Some(index);
                }
            }
            ui.min_rect().left()
        });
        right = placed.inner - BUTTONS_GAP;
    }
    let left = rect.left() + TEXT_LEFT;
    let line = Rect::from_min_max(pos2(left, one), pos2(right.max(left), one + f.body.line));
    let first = Top { name: row.to, font: f.body, ink: p.ink, account: row.account, tag: None, marks: &[], time: Some((row.words, ink)) };
    top(ui, theme, line, &first, None);
    let subject = super::fitted(ui, row.subject, f.caption, p.muted, (right - left).max(0.0));
    paint::run_at(ui.painter(), pos2(left, two), subject, p.muted);
    if let Some(reply) = row.reply {
        let reply = super::fitted(ui, reply, f.caption, p.minus, (rect.right() - RIGHT - left).max(0.0));
        paint::run_at(ui.painter(), pos2(left, stack_top + upper + REPLY_GAP), reply, p.minus);
    }
    if let Standing::Countdown { share } = row.standing {
        let wide = (rect.width() * share.clamp(0.0, 1.0)).round();
        if wide > 0.0 {
            // Its foot's last pixel row is the row's line, which the design draws over the bar.
            let line = if row.last { 0.0 } else { 1.0 };
            ui.painter().rect_filled(Rect::from_min_size(pos2(rect.left(), rect.bottom() - BAR), vec2(wide, BAR - line)), 0.0, p.cyan_edge);
        }
    }
    drawn.finish(ui, theme);
    pressed
}

#[cfg(test)]
#[path = "outbox_row_tests.rs"]
mod tests;
