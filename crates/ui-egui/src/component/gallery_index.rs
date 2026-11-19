// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the index's rows for `ui_shot` (`"index": true`): the boards of the place's row, the
//! mailbox's row and the account's row, every state drawn as its sheet says with the sheet's words, labelled in the
//! sheet's order, on the boards' `bg`. The states' samples are here, so the state tests of the sets draw what the
//! boards draw.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::iconset::IconId;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::gallery::{cell, title};
use super::margin;
use super::nav_row::{self, NavRow, Standing};

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The page's width, in points: four cells of a board side by side.
const WIDTH: f32 = 1200.0;

/// The page's height, in points.
const HEIGHT: f32 = 900.0;

/// The width of a wide row on the boards, in points.
pub(crate) const WIDE: f32 = 232.0;

/// The width of a narrow row on the boards, in points.
pub(crate) const NARROW: f32 = 56.0;

/// One state of the place row's sheet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NavState {
    pub name: &'static str,
    pub count: u64,
    pub standing: Standing,
    pub narrow: bool,
    pub shown: Shown,
}

/// The place row's states, in the sheet's order, with the sheet's words.
pub(crate) fn nav_states() -> [NavState; 10] {
    let rest = NavState { name: "default", count: 0, standing: Standing::Rest, narrow: false, shown: Shown::Rest };
    [
        rest,
        NavState { name: "hover", shown: Shown::Hover, ..rest },
        NavState { name: "focus", shown: Shown::Focus, ..rest },
        NavState { name: "active", count: 7, standing: Standing::Active, ..rest },
        NavState { name: "open", count: 7, standing: Standing::Open, ..rest },
        NavState { name: "with-count", count: 7, ..rest },
        NavState { name: "overflow-count", count: 12_000, ..rest },
        NavState { name: "narrow", narrow: true, ..rest },
        NavState { name: "narrow-active", narrow: true, standing: Standing::Active, ..rest },
        NavState { name: "narrow-open", narrow: true, standing: Standing::Open, ..rest },
    ]
}

/// What a screen reader hears for the place row of `state`.
pub(crate) fn nav_spoken(state: &NavState) -> String {
    format!("Mail ({})", state.name)
}

/// The place row of `state` at `ui`'s cursor, as wide as the state says.
pub(crate) fn nav(ui: &mut Ui, theme: &Theme, state: &NavState) {
    let spoken = nav_spoken(state);
    let row = NavRow {
        id: Id::unique("gallery-nav-row").with(state.name),
        spoken: &spoken,
        index: "[ 01 ]",
        label: tr!("Mail"),
        icon: IconId::Inbox,
        count: state.count,
        standing: state.standing,
    };
    let forced = (state.shown != Shown::Rest).then_some(state.shown);
    sized(ui, theme, state.narrow, nav_row::HEIGHT, |ui| {
        nav_row::nav_row(ui, theme, &row, state.narrow, forced);
    });
}

/// `add` in a column as wide as a wide or a narrow row and `height` high at 100 % text, on the index's `panel`.
fn sized(ui: &mut Ui, theme: &Theme, narrow: bool, height: f32, add: impl FnOnce(&mut Ui)) {
    let wide = if narrow { NARROW } else { WIDE };
    let at = Rect::from_min_size(ui.cursor().min, vec2(wide, Theme::size(theme, height)));
    ui.painter().rect_filled(at, 0.0, theme.palette.panel);
    ui.allocate_ui(vec2(wide, at.height()), |ui| {
        ui.set_width(wide);
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui);
    });
}

/// A board: `title`, then `states` four to a line.
fn board<S>(ui: &mut Ui, theme: &Theme, name: &str, states: &[S], label: fn(&S) -> &'static str, draw: fn(&mut Ui, &Theme, &S)) {
    title(ui, theme, name);
    for line in states.chunks(4) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.s4;
            for state in line {
                cell(ui, theme, label(state), |ui| draw(ui, theme, state));
            }
        });
        ui.add_space(theme.spacing.s2);
    }
}

/// Draws the page into `ui` in `theme`.
pub fn page(ui: &mut Ui, theme: &Theme) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let rect = Rect::from_min_size(pos2(s.s2, s.s2), vec2(WIDTH - s.s2 * 2.0, HEIGHT - s.s2 * 2.0));
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::NONE.inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            board(ui, theme, tr!("Place row"), &nav_states(), |state| state.name, nav);
        });
    });
}

#[cfg(test)]
#[path = "gallery_index_tests.rs"]
mod tests;
