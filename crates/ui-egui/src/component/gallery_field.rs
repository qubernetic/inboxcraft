// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the character field for `ui_shot` (`"character_field": true`): the field at 100 % and at
//! 130 % text, each with an emblem centred on it, on the page's `bg`, at the full motion level and as Home's field,
//! so the pointer of a shot (`"pointer"`) warms the cells under it. The emblem here is a plain place holder of the
//! emblem's grid (76 × 40 cells): a ring in ink round a block in cyan, not the Qubernetic mark.

use std::sync::{Arc, Mutex};

use egui::{Id, Rect, Ui, Vec2, pos2, vec2};

use crate::field::{self, Emblem, Field, Level, Opts};
use crate::theme::{TextScale, Theme};
use crate::tr;

use super::gallery::title;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The page's width, in points: two boards and the room round them.
const WIDTH: f32 = 1520.0;

/// The page's height, in points.
const HEIGHT: f32 = 760.0;

/// A board's field, in points: the emblem (547 × 500) and room round it.
const BOARD: Vec2 = vec2(720.0, 620.0);

/// The room round and between the boards, in points.
const GAP: f32 = 24.0;

/// The place holder of the emblem: 76 × 40 cells, a ring two cells thick in ink (`#`), a block in cyan (density 9)
/// in its middle.
#[must_use]
pub fn placeholder() -> Emblem {
    let rows: Vec<String> = (0..40)
        .map(|row| {
            (0..76)
                .map(|column| {
                    let ring = !(2..38).contains(&row) || !(2..74).contains(&column);
                    let block = (14..26).contains(&row) && (26..50).contains(&column);
                    if ring {
                        '#'
                    } else if block {
                        '9'
                    } else {
                        ' '
                    }
                })
                .collect()
        })
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    Emblem::from_rows(&rows)
}

/// One board: its name over the field at `scale`, the emblem centred on it.
fn board(ui: &mut Ui, theme: &Theme, name: &str, scale: TextScale, at: egui::Pos2) {
    let label = Rect::from_min_size(at, vec2(BOARD.x, 20.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(label), |ui| {
        ui.label(super::galley(ui, name, theme.fonts.mono_s, theme.palette.muted));
    });
    let rect = Rect::from_min_size(at + vec2(0.0, 28.0), BOARD);
    let id = Id::unique("gallery-character-field").with(scale.percent());
    let shown = theme.with_text_scale(scale);
    // Kept between frames in the context, shared, so a frame never copies its cells.
    let kept =
        ui.data_mut(|data| Arc::clone(data.get_temp_mut_or_insert_with(id, || Arc::new(Mutex::new(Field::new(0).with_emblem(placeholder()))))));
    if let Ok(mut kept) = kept.lock() {
        field::show(ui, &shown, &mut kept, rect, Level::Full, Opts { on_home: true, on_pane: false, emblem: Some(rect) });
    }
}

/// The page: its title and the two boards.
pub fn page(ui: &mut Ui, theme: &Theme) {
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let title_rect = Rect::from_min_size(pos2(GAP, GAP), vec2(WIDTH - GAP * 2.0, 48.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(title_rect), |ui| title(ui, theme, tr!("Character field")));
    let top = GAP + 64.0;
    let large = TextScale::from_percent(130).unwrap_or_default();
    board(ui, theme, tr!("100 % text"), TextScale::default(), pos2(GAP, top));
    board(ui, theme, tr!("130 % text"), large, pos2(GAP * 2.0 + BOARD.x, top));
}

#[cfg(test)]
#[path = "gallery_field_tests.rs"]
mod tests;
