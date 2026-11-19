// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the command line's sets for `ui_shot` (`"command_sets": true`): the boards of the command
//! field (its twelve states), of the key hint (default, hover) and of the window's own controls (default, the three
//! hovers, maximised, unfocused), every state drawn as its sheet says with the sheet's words, on the boards' `bg`.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::command_field::{Scope, command_field};
use super::gallery::{cell, cells, title};
use super::key_hint::key_hint;
use super::margin;
use super::window_controls::{Control, Window, window_controls};

/// The page's width, in points.
const WIDTH: f32 = 1200.0;

/// The page's height, in points.
const HEIGHT: f32 = 720.0;

/// The command field's width on its board, as its sheet draws it.
const FIELD: f32 = 560.0;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The command field's states in the sheet's order: its name, scope, text and what is forced.
const FIELDS: [(&str, Scope<'static>, &str, Option<Shown>); 12] = [
    ("rest", Scope::Open, "", None),
    ("hover", Scope::Open, "", Some(Shown::Hover)),
    ("focus", Scope::Open, "", Some(Shown::Focus)),
    ("typing", Scope::Open, "bak", Some(Shown::Focus)),
    ("prefix-command", Scope::Commands, "snooze", Some(Shown::Focus)),
    ("prefix-person", Scope::People, "teo", Some(Shown::Focus)),
    ("prefix-mailbox", Scope::Mailboxes, "inv", Some(Shown::Focus)),
    ("search-mail", Scope::Mailbox("Inbox"), "", Some(Shown::Focus)),
    ("search-mail-typing", Scope::Mailbox("Inbox"), "bakery", Some(Shown::Focus)),
    ("search-mail-set", Scope::Mailbox("Inbox"), "bakery", None),
    ("search-all-typing", Scope::AllMail, "bakery", Some(Shown::Focus)),
    ("long", Scope::Open, "from:teodor bakery sign first sketch with the door on the left and the wheat kept in", Some(Shown::Focus)),
];

/// The command field's board, two states to a row.
fn command_fields(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Command field"));
    for pair in FIELDS.chunks(2) {
        cells(ui, theme, |ui| {
            for (state, scope, text, forced) in pair {
                cell(ui, theme, state, |ui| {
                    ui.set_max_width(FIELD);
                    let mut text = (*text).to_owned();
                    command_field(ui, theme, Id::unique(("gallery-command", *state)), &mut text, *scope, "Ctrl K", *forced);
                });
            }
        });
    }
}

/// The key hint's board: "D done" at rest and hovered.
fn key_hints(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Key hint"));
    cells(ui, theme, |ui| {
        for (state, forced) in [("default", Some(Shown::Rest)), ("hover", Some(Shown::Hover))] {
            cell(ui, theme, state, |ui| {
                key_hint(ui, theme, "D", tr!("done"), forced);
            });
        }
    });
}

/// The window controls' board, in the sheet's order.
fn controls(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Window controls"));
    let front = Window { maximised: false, focused: true };
    let boards = [
        ("default", front, (Control::Minimise, Shown::Rest)),
        ("hover-minimise", front, (Control::Minimise, Shown::Hover)),
        ("hover-maximise", front, (Control::Maximise, Shown::Hover)),
        ("hover-close", front, (Control::Close, Shown::Hover)),
        ("maximised", Window { maximised: true, focused: true }, (Control::Minimise, Shown::Rest)),
        ("unfocused", Window { maximised: false, focused: false }, (Control::Minimise, Shown::Rest)),
    ];
    cells(ui, theme, |ui| {
        for (state, window, forced) in boards {
            cell(ui, theme, state, |ui| {
                window_controls(ui, theme, Id::unique(("gallery-window", state)), window, Some(forced));
            });
        }
    });
}

/// Draws the page into `ui` in `theme`.
pub fn page(ui: &mut Ui, theme: &Theme, _keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let rect = Rect::from_min_size(pos2(s.s2, s.s2), vec2(WIDTH - s.s2 * 2.0, HEIGHT - s.s2 * 2.0));
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::NONE.inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            command_fields(ui, theme);
            ui.add_space(24.0);
            key_hints(ui, theme);
            ui.add_space(24.0);
            controls(ui, theme);
        });
    });
}

#[cfg(test)]
#[path = "gallery_commands_tests.rs"]
mod tests;
