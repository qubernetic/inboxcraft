// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the reader's sets for `ui_shot` (`"reader": true`): the boards of the why line, the content
//! bar, the thread row, the quote fold, the attachment, the hairline table and the invitation card, every state drawn
//! as its sheet says, labelled in the sheet's order with the design boards' words, on the boards' `bg`, three columns,
//! the boards evenly apart. A page of its own: one picture is at most 4 096 px a side.

use egui::{Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::gallery::{cell, cells, title};
use super::margin;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The page's width, in points: three columns of boards.
const WIDTH: f32 = 2400.0;

/// The page's height, in points: the tallest column and its margins.
const HEIGHT: f32 = 2560.0;

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// The why line's board: group, new sender, screener and folder, 640 wide, with the design board's words.
fn why_lines(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::why_line::why_line;
    title(ui, theme, tr!("Why line"));
    let states = [
        (tr!("group"), "let in by you on 3 Nov  ·  grouped as People  ·  synced 14:02 by IDLE"),
        (tr!("new-sender"), "first mail on 14 Nov  ·  let in by you today  ·  grouped as People"),
        (tr!("screener"), "first mail from this address  ·  SPF and DKIM pass  ·  held back"),
        (tr!("folder"), "moved by you on 9 Nov  ·  in Studio / Clients / Bakery"),
    ];
    for (state, reasons) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                why_line(ui, theme, keymap, reasons);
            });
        });
    }
}

/// The left column.
fn left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    why_lines(ui, theme, keymap);
}

/// The middle column.
fn middle(_ui: &mut Ui, _theme: &Theme, _keymap: &Keymap) {}

/// The right column.
fn right(_ui: &mut Ui, _theme: &Theme, _keymap: &Keymap) {}

/// Draws the page into `ui` in `theme` with the keys of `keymap`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let column = ((WIDTH - s.s2 * 4.0) / 3.0).floor();
    let draws: [Draw; 3] = [left, middle, right];
    for (index, draw) in draws.into_iter().enumerate() {
        let x = s.s2 + index as f32 * (column + s.s2);
        let rect = Rect::from_min_size(pos2(x.round(), s.s2), vec2(column, HEIGHT - s.s2 * 2.0));
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            // The boards stand on `bg`, as the design's boards do.
            egui::Frame::NONE.inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                draw(ui, theme, keymap);
            });
        });
    }
}

#[cfg(test)]
#[path = "gallery_reader_tests.rs"]
mod tests;
