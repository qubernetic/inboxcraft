// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The confirmation dialog over the dimmed window, as the confirm frames draw it (`settings-templates/delete-confirm`,
//! `scheduled-list/cancel-confirm`, `account-remove/confirm`).
//!
//! The `scrim` lies over the whole window down to the status line, which stays as it is. Over it, in the middle of the
//! dimmed part, a card 480 wide whose height follows its words: the float's `surface`, 1 px `cyan_edge` inside and its
//! one shadow ([`crate::draw::paint::float_shapes`]), square; 24 in at the top and the sides, 21 at the foot (each 1
//! further for the edge); its parts 12 apart: the `CONFIRM` label in `muted`, the question in `heading`, the lines
//! in `body` (`ink` or `muted`), one point of room, then the buttons 12 apart in a row 3 in on every side (room for
//! the focus ring): the safe one (`secondary`), then the danger one.
//!
//! The keyboard is the dialog's while it shows: it opens on the safe button, `Tab` and `Shift Tab` go round its two
//! buttons only, Enter presses the button with the keyboard. `Esc` and a press on the dimmed window are the safe
//! answer. The danger answer comes only from the owner's press of the danger button ([`owner_pressed`]): never from
//! `Esc`, a focus, a hover or a key held down from before. Nothing under it takes a press or the keyboard (its layer
//! is the modal one).
//!
//! The words come from the caller with names in them (a template's, an account's, a recipient's) and are drawn as
//! plain words: each control character a space, each bidi override left out, at most [`MOST_CHARS`] characters, the
//! question on at most [`QUESTION_ROWS`] rows and each line on at most [`LINE_ROWS`], the last cut with an ellipsis.

use egui::{Align, Area, Context, Id, Key, LayerId, Layout, Margin, Modifiers, Order, Rect, Role, Sense, Shape, Ui, WidgetInfo, pos2, vec2};

use super::button::{TextButton, owner_pressed};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The card's width at 100 %, in points.
pub const WIDTH: f32 = 480.0;

/// The card's room inside its edge at the top and the sides, in points.
pub const PAD: f32 = 24.0;

/// The card's room inside its edge at the foot, in points.
pub const PAD_FOOT: f32 = 21.0;

/// Between the card's parts, and between its two buttons, in points.
pub const GAP: f32 = 12.0;

/// The room round the buttons in their row, for the focus ring, in points.
pub const RING: f32 = 3.0;

/// The card's edge, in points: its parts stand this much further in.
const EDGE: f32 = 1.0;

/// The card's room inside it: [`PAD`] and [`PAD_FOOT`], each with the edge.
const MARGIN: Margin = Margin { left: 25, right: 25, top: 25, bottom: 22 };

/// The least room between the card and the window's sides, in points: a narrow window narrows the card.
const CLEAR: f32 = 16.0;

/// The most characters of one text drawn: a name of 10 000 letters costs no more than this.
pub const MOST_CHARS: usize = 600;

/// The most rows the question takes.
pub const QUESTION_ROWS: usize = 2;

/// The most rows one line takes.
pub const LINE_ROWS: usize = 4;

/// The colour of a line of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// `ink`: what goes.
    Ink,
    /// `muted`: what stays.
    Muted,
}

/// What the owner answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The safe button, `Esc` or a press on the dimmed window: nothing is done.
    Keep,
    /// The owner's press of the danger button.
    Act,
}

/// What a dialog says: the question, its lines, its two buttons.
#[derive(Debug, Clone, Copy)]
pub struct Dialog<'a> {
    /// Its own id: the same while it stays open.
    pub id: Id,
    /// The question, its heading.
    pub question: &'a str,
    /// The lines under the question, each with its colour.
    pub lines: &'a [(&'a str, Tone)],
    /// The safe button's words (Cancel, Keep it).
    pub keep: &'a str,
    /// The danger button's words (Delete template, Remove account).
    pub act: &'a str,
}

/// What the dialog kept from the pass before: when it showed and where.
#[derive(Debug, Clone, Copy)]
struct Kept {
    pass: u64,
    rect: Rect,
}

/// The window's part the scrim lies over in `ctx`: all of it down to the status line's top (the hairline over the
/// status line is dimmed too, the status line is not), at the window's whole pixels.
#[must_use]
pub fn cover(ctx: &Context, theme: &Theme) -> Rect {
    let screen = ctx.content_rect();
    let ppp = ctx.pixels_per_point();
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    let line = ppp.round().max(1.0) / ppp;
    let status = (theme.boxes().status * ppp).round() / ppp;
    let bottom = (screen.bottom() - line - status).max(screen.top());
    Rect::from_min_max(screen.min, pos2(screen.right(), bottom))
}

/// The card's width in a window whose dimmed part is `cover`: [`WIDTH`], less where the window is narrower.
fn width(cover: Rect) -> f32 {
    WIDTH.min((cover.width() - 2.0 * CLEAR).max(1.0)).round()
}

/// `text` as the dialog draws it: plain, at most [`MOST_CHARS`] characters.
fn clean(text: &str) -> String {
    let (kept, _) = super::capped(text, MOST_CHARS);
    super::plain(kept).into_owned()
}

/// The id of the dialog's layer: the modal one while it shows.
#[must_use]
pub fn layer(id: Id) -> LayerId {
    LayerId::new(Order::Foreground, id)
}

/// Shows `dialog` over the dimmed window, as the module says; the owner's answer in this pass, if any. The caller
/// shows it every pass while its question is open and stops when an answer closed it.
pub fn show(ctx: &Context, theme: &Theme, keymap: &Keymap, dialog: &Dialog<'_>) -> Option<Answer> {
    let key = dialog.id.with("dialog-kept");
    let pass = ctx.cumulative_pass_nr();
    let kept = ctx.data(|data| data.get_temp::<Kept>(key)).filter(|kept| kept.pass.saturating_add(1) >= pass);
    let opening = kept.is_none();
    let cover = cover(ctx, theme);
    let mine = layer(dialog.id);
    let mut answer = None;
    if let Some(kept) = kept {
        // Esc and a press on the dimmed window are the safe answer; a press on the card is its own.
        let (escaped, beside) = ctx.input_mut(|input| {
            let beside = input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(|at| cover.contains(at) && !kept.rect.contains(at));
            (input.consume_key(Modifiers::NONE, Key::Escape), beside)
        });
        if escaped || beside {
            answer = Some(Answer::Keep);
        }
    }
    let scrim = Area::new(dialog.id.with("scrim")).order(Order::Foreground).fixed_pos(cover.min).constrain(false).fade_in(false).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(cover.size(), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, theme.palette.scrim);
    });
    ctx.move_to_top(scrim.response.layer_id);
    // Only the card takes the keyboard and the pointer: Tab goes round its buttons.
    ctx.memory_mut(|memory| memory.set_modal_layer(mine));
    ctx.move_to_top(mine);
    let width = width(cover);
    let high = kept.map_or(0.0, |kept| kept.rect.height());
    let at = pos2((cover.center().x - width / 2.0).round(), (cover.center().y - high / 2.0).round().max(cover.top()));
    let shown = Area::new(dialog.id)
        .order(Order::Foreground)
        .fixed_pos(at)
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| card(ui, theme, keymap, dialog, width, opening));
    ctx.data_mut(|data| data.insert_temp(key, Kept { pass, rect: shown.response.rect }));
    // An answer of the keyboard or the pointer this pass goes before Esc's: both are the owner's.
    shown.inner.or(answer)
}

/// The card `width` wide in `ui`: its parts and its buttons; the button pressed, if one was. The safe button takes the
/// keyboard when the dialog is `opening`.
fn card(ui: &mut Ui, theme: &Theme, keymap: &Keymap, dialog: &Dialog<'_>, width: f32, opening: bool) -> Option<Answer> {
    let p = &theme.palette;
    let f = &theme.fonts;
    let slot = ui.painter().add(Shape::Noop);
    let inner = (width - 2.0 * (PAD + EDGE)).max(1.0);
    let framed = egui::Frame::new().inner_margin(MARGIN).show(ui, |ui| {
        ui.set_width(inner);
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        let label = super::galley(ui, tr!("Confirm"), f.label, p.muted);
        let (rect, _) = ui.allocate_exact_size(vec2(inner, label.size().y.ceil()), Sense::hover());
        ui.painter().galley(rect.min, label, p.muted);
        let question = clean(dialog.question);
        let heading = super::wrapped(ui, &question, f.heading, p.ink, inner, QUESTION_ROWS, Align::Min);
        let (rect, said) = ui.allocate_exact_size(vec2(inner, heading.size().y.ceil()), Sense::hover());
        ui.painter().galley(rect.min, heading, p.ink);
        said.widget_info(|| WidgetInfo::labeled(Role::Heading, true, &question));
        for (text, tone) in dialog.lines {
            let colour = match tone {
                Tone::Ink => p.ink,
                Tone::Muted => p.muted,
            };
            let text = clean(text);
            let line = super::wrapped(ui, &text, f.body, colour, inner, LINE_ROWS, Align::Min);
            let (rect, said) = ui.allocate_exact_size(vec2(inner, line.size().y.ceil()), Sense::hover());
            ui.painter().galley(rect.min, line, colour);
            said.widget_info(|| WidgetInfo::labeled(Role::Label, true, &text));
        }
        // The frames' point of room before the buttons.
        ui.allocate_exact_size(vec2(1.0, 1.0), Sense::hover());
        let row = super::control_side(theme) + 2.0 * RING;
        ui.allocate_ui_with_layout(vec2(inner, row), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_height(row);
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(RING);
            let keep = TextButton::new(dialog.keep).secondary().show(ui, theme, keymap);
            if opening {
                keep.request_focus();
            }
            ui.add_space(GAP);
            let act = TextButton::new(dialog.act).danger().show(ui, theme, keymap);
            if owner_pressed(ui, &act) {
                Some(Answer::Act)
            } else if keep.clicked() {
                Some(Answer::Keep)
            } else {
                None
            }
        })
        .inner
    });
    let shapes = crate::draw::paint::float_shapes(theme, framed.response.rect, crate::draw::paint::FloatEdge::Keyboard, ui.ctx().pixels_per_point());
    ui.painter().set(slot, Shape::Vec(shapes));
    framed.inner
}

#[cfg(test)]
#[path = "dialog_tests.rs"]
mod tests;
