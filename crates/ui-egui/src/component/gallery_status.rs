// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the status line's sets for `ui_shot` (`"status_sets": true`): the boards of the status item
//! (plain, ok, waiting, offline, error, accent, hover, focus) and of the status message (undo, send countdown, notice,
//! error, done), every state drawn as its sheet says with the sheet's words, on the status line's `panel`.

use egui::{Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::gallery::{cell, cells, title};
use super::margin;
use super::status_item::{Kind, StatusItem, status_item};
use super::status_message::{self, Left, StatusMessage, status_message};

/// The page's width, in points.
const WIDTH: f32 = 1200.0;

/// The page's height, in points.
const HEIGHT: f32 = 520.0;

/// The width of a message's board: the sheet's.
const MESSAGE: f32 = 560.0;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The status item's board, in the sheet's order and with its words; hover and focus on an item that can be pressed.
fn items(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Status item"));
    let rows: [&[(&str, StatusItem<'_>)]; 2] = [
        &[
            ("plain", StatusItem::new(tr!("2 accounts"), Kind::Plain)),
            ("ok", StatusItem::new(tr!("synced 14:02"), Kind::Ok)),
            ("waiting", StatusItem::new(tr!("outbox 1 · goes at 14:05"), Kind::Waiting)),
            ("offline", StatusItem::new(tr!("offline since 14:10"), Kind::Offline)),
        ],
        &[
            ("error", StatusItem::new(tr!("Studio: sign-in refused"), Kind::Error)),
            ("accent", StatusItem::new(tr!("next: Design review 15:00 · in 12 min"), Kind::Accent)),
            ("hover", StatusItem::new(tr!("synced 14:02"), Kind::Ok).pressable().shown(Shown::Hover)),
            ("focus", StatusItem::new(tr!("synced 14:02"), Kind::Ok).pressable().shown(Shown::Focus)),
        ],
    ];
    for row in rows {
        cells(ui, theme, |ui| {
            for (state, item) in row {
                cell(ui, theme, state, |ui| {
                    ui.set_max_width(super::status_item::MOST_TEXT + 40.0);
                    status_item(ui, theme, *item);
                });
            }
        });
    }
}

/// The status message's board, 560 wide, with the sheet's words: 4 of 5 seconds left to undo, and its key.
fn messages(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Status message"));
    let left = Some(Left { seconds: 4, share: 0.8, bar: true });
    // The undo key as the window's keymap writes it.
    let chip = keymap.chip(ui.ctx(), "ui.undo");
    let key = chip.as_deref();
    let boards = [
        ("undo", StatusMessage { kind: status_message::Kind::Undo { key, left }, text: tr!("done · Draft logo for the bakery") }),
        ("send-countdown", StatusMessage { kind: status_message::Kind::Countdown { key, left }, text: tr!("sending · Re: Thursday workshop") }),
        ("notice", StatusMessage { kind: status_message::Kind::Notice, text: tr!("Link copied: https://example.org/open-day") }),
        ("error", StatusMessage { kind: status_message::Kind::Error, text: tr!("Studio: sign-in refused, mail is not being fetched") }),
        ("done", StatusMessage { kind: status_message::Kind::Done, text: tr!("sent 14:02 · Re: Thursday workshop") }),
    ];
    for pair in boards.chunks(2) {
        cells(ui, theme, |ui| {
            for (state, message) in pair {
                cell(ui, theme, state, |ui| {
                    ui.set_width(MESSAGE);
                    status_message(ui, theme, keymap, message);
                });
            }
        });
    }
}

/// Draws the page into `ui` in `theme`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.panel);
    let rect = Rect::from_min_size(pos2(s.s2, s.s2), vec2(WIDTH - s.s2 * 2.0, HEIGHT - s.s2 * 2.0));
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::NONE.inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            items(ui, theme);
            ui.add_space(24.0);
            messages(ui, theme, keymap);
        });
    });
}

#[cfg(test)]
#[path = "gallery_status_tests.rs"]
mod tests;
