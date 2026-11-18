// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the calendar's sets for `ui_shot` (`"calendar_sets": true`): the boards of the event block
//! (timed, band, chip, row, picked, hover, focus) and of the mini month (default, today, picked, hover), every state
//! drawn as its sheet says with the sheet's words, on the boards' `bg`.

use egui::{Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::calendar::Day;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::gallery::{cell, cells, title};
use super::margin;

/// The page's width, in points.
const WIDTH: f32 = 1200.0;

/// The page's height, in points.
const HEIGHT: f32 = 760.0;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The event block's board, in the sheet's order.
fn event_blocks(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Event block"));
    for row in [["timed", "band", "chip"].as_slice(), ["row"].as_slice(), ["picked", "hover", "focus"].as_slice()] {
        cells(ui, theme, |ui| {
            for state in row {
                cell(ui, theme, state, |ui| {
                    crate::draw::event_board(ui, theme, state);
                });
            }
        });
    }
}

/// The mini month's board: December 2026 at rest, November 2026 with today, a picked day and a hovered one.
fn mini_months(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Mini month"));
    let day = |month, date| Day::new(2026, month, date).unwrap_or(Day::EPOCH);
    let boards = [
        ("default", day(12, 1), day(11, 16), None, None),
        ("today", day(11, 1), day(11, 16), None, None),
        ("picked", day(11, 1), day(11, 16), Some(day(11, 19)), None),
        ("hover", day(11, 1), day(11, 16), None, Some(day(11, 24))),
    ];
    cells(ui, theme, |ui| {
        for (board, (state, month, today, picked, hover)) in (0_u8..).zip(boards) {
            cell(ui, theme, state, |ui| crate::draw::mini_month_board(ui, theme, board, month, today, (picked, hover)));
        }
    });
}

/// Draws the page into `ui` in `theme` (its sets show no key: `_keymap` keeps the pages' signature).
pub fn page(ui: &mut Ui, theme: &Theme, _keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let rect = Rect::from_min_size(pos2(s.s2, s.s2), vec2(WIDTH - s.s2 * 2.0, HEIGHT - s.s2 * 2.0));
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::NONE.inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            event_blocks(ui, theme);
            ui.add_space(24.0);
            mini_months(ui, theme);
        });
    });
}

#[cfg(test)]
#[path = "gallery_calendar_tests.rs"]
mod tests;
