// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the screens' own sets, for `ui_shot` (`"screens": true`): the boards of the sets the composer,
//! calendar, settings pages and onboarding bring, every state of each sheet in its order, labelled, on `bg`. The
//! file on a draft (`draft-file`) is coming soon: attaching files is not built, so its board carries the tag. The data
//! is invented; the keys come from [`super::gallery::keymap`].

use egui::{Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::draft_file::{DraftFile, draft_file_with};
use super::galley;
use super::soon::soon_tag;

/// The width of the page's column, in points.
const COLUMN: f32 = 760.0;

/// The page's height, in points.
const HEIGHT: f32 = 220.0;

/// The width of a file's row on its board: the sheet's 320.
const FILE: f32 = 320.0;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(COLUMN, HEIGHT)
}

/// A board's title: the set's name, and the coming-soon tag when it is not built yet.
fn title(ui: &mut Ui, theme: &Theme, text: &str, coming: bool) {
    ui.add_space(theme.spacing.s2);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.s2;
        ui.label(galley(ui, text, theme.fonts.heading, theme.palette.ink));
        if coming {
            soon_tag(ui, theme);
        }
    });
    ui.add_space(theme.spacing.s1);
}

/// One state of a board: its name in `mono_s` and `muted` over the control `add` draws.
fn cell(ui: &mut Ui, theme: &Theme, state: &str, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.s1;
        ui.label(galley(ui, state, theme.fonts.mono_s, theme.palette.muted));
        ui.push_id(state, add);
    });
}

/// A row of a board's cells, apart enough that a focus ring never touches the next control.
fn cells(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.s6;
        add(ui);
    });
    ui.add_space(theme.spacing.s2);
}

/// The board of a file on a draft: default, hover, focus, too-large, with the sheet's words.
fn draft_files(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("File on a draft"), true);
    let quote = DraftFile { name: "shop-front-quote.pdf", size: "184 KB", too_large: false };
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("default"), |ui| {
            draft_file_with(ui, theme, keymap, &quote, FILE, None);
        });
        cell(ui, theme, tr!("hover"), |ui| {
            draft_file_with(ui, theme, keymap, &DraftFile { name: "lettering-samples.zip", size: "3.1 MB", ..quote }, FILE, Some(Shown::Hover));
        });
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("focus"), |ui| {
            draft_file_with(ui, theme, keymap, &DraftFile { name: "fascia-measurements.txt", size: "2 KB", ..quote }, FILE, Some(Shown::Focus));
        });
        cell(ui, theme, tr!("too-large"), |ui| {
            draft_file_with(ui, theme, keymap, &DraftFile { name: "open-day-all.zip", size: "48 MB", too_large: true }, FILE, None);
        });
    });
}

/// Draws the page into `ui` in `theme` with the keys of `keymap`: the boards on `bg`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    ui.scope_builder(UiBuilder::new().max_rect(whole), |ui| {
        egui::Frame::NONE.inner_margin(super::margin(s.s4, s.s3)).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.push_id("draft-file", |ui| draft_files(ui, theme, keymap));
        });
    });
}

#[cfg(test)]
#[path = "gallery_screens_tests.rs"]
mod tests;
