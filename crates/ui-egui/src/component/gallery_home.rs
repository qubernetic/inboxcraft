// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of Home's sets for `ui_shot` (`"home_sets": true`): the boards of the count tile, the block and
//! the block line, every state drawn as its sheet says with the sheet's words, labelled in the sheet's order, on the
//! boards' `bg`, two columns; and under the block line a drafts block holding its empty line, as Home puts them
//! together. The states' samples are here, so the state tests of the sets draw what the boards draw.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::block::{self, Block, Inset};
use super::block_line::{self, BlockLine};
use super::count_tile::{self, CountTile, Tone};
use super::gallery::{cell, cells, title};
use super::margin;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The page's width and height, in points.
const WIDTH: f32 = 1520.0;
const HEIGHT: f32 = 640.0;

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// The room between two boards, in points.
const BOARD_GAP: f32 = 24.0;

/// A count tile's width on the boards, in points: the sheet's.
pub(crate) const TILE_WIDTH: f32 = 140.0;

/// A block's and a block line's width on the boards, in points: the sheets'.
pub(crate) const BLOCK_WIDTH: f32 = 320.0;

/// The drafts block's width on its board, in points: the drafts frame's.
pub(crate) const DRAFTS_WIDTH: f32 = 494.0;

/// One state of the count tile's sheet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TileState {
    /// Its name on the sheet.
    pub name: &'static str,
    pub count: u64,
    pub label: &'static str,
    pub tone: Tone,
    pub shown: Shown,
}

/// The count tile's states, in the sheet's order, with the sheet's words; the overflow's count is any above 9 999.
pub(crate) fn tile_states() -> [TileState; 8] {
    let read = TileState { name: "accent", count: 7, label: tr!("To read"), tone: Tone::Accent, shown: Shown::Rest };
    [
        TileState { name: "default", count: 3, label: tr!("To screen"), tone: Tone::Plain, shown: Shown::Rest },
        read,
        TileState { name: "warn", count: 2, label: tr!("Back today"), tone: Tone::Warn, shown: Shown::Rest },
        TileState { name: "minus", count: 1, label: tr!("Not sent"), tone: Tone::Minus, shown: Shown::Rest },
        TileState { name: "zero", count: 0, label: tr!("Nothing to read"), tone: Tone::Accent, shown: Shown::Rest },
        TileState { name: "overflow", count: 12_345, ..read },
        TileState { name: "hover", shown: Shown::Hover, ..read },
        TileState { name: "focus", shown: Shown::Focus, ..read },
    ]
}

/// The tile of `state`.
pub(crate) fn tile_of(state: &TileState) -> CountTile<'static> {
    CountTile { id: Id::unique("gallery-count-tile").with(state.name), count: state.count, label: state.label, tone: state.tone }
}

/// The count tile of `state`, 140 wide, at `ui`'s cursor.
pub(crate) fn tile(ui: &mut Ui, theme: &Theme, state: &TileState) -> egui::Response {
    ui.allocate_ui(vec2(TILE_WIDTH, 200.0), |ui| {
        ui.set_width(TILE_WIDTH);
        count_tile::count_tile(ui, theme, &tile_of(state), (state.shown != Shown::Rest).then_some(state.shown))
    })
    .inner
}

/// The count tile's board: every state of its sheet, four to a line.
fn tiles(ui: &mut Ui, theme: &Theme, _keymap: &Keymap) {
    title(ui, theme, tr!("Count tile"));
    for four in tile_states().chunks(4) {
        cells(ui, theme, |ui| {
            for state in four {
                cell(ui, theme, state.name, |ui| {
                    // The tile stands on `surface`, as Home's row of counts does.
                    let at = ui.cursor().min;
                    let high = count_tile::height(ui, theme, state.label, TILE_WIDTH);
                    ui.painter().rect_filled(Rect::from_min_size(at, vec2(TILE_WIDTH, high)), 0.0, theme.palette.surface);
                    tile(ui, theme, state);
                });
            }
        });
    }
}

/// One state of the block's sheet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockState {
    pub name: &'static str,
    pub selected: bool,
    pub shown: Shown,
}

/// The block's states, in the sheet's order.
pub(crate) fn block_states() -> [BlockState; 4] {
    let default = BlockState { name: "default", selected: false, shown: Shown::Rest };
    [
        default,
        BlockState { name: "hover", shown: Shown::Hover, ..default },
        BlockState { name: "focus", shown: Shown::Focus, ..default },
        BlockState { name: "selected", selected: true, ..default },
    ]
}

/// The block's sheet's label and lines.
pub(crate) const BLOCK_INDEX: &str = "[ 02 ]";
pub(crate) const AGENDA: [&str; 2] = ["10:00  Workshop prep", "15:00  Design review"];

/// The block of `state`, 320 wide, at `ui`'s cursor, holding the sheet's two lines of the agenda.
pub(crate) fn agenda(ui: &mut Ui, theme: &Theme, state: &BlockState) -> block::Drawn<()> {
    let shown = Block {
        id: Id::unique("gallery-block").with(state.name),
        index: BLOCK_INDEX,
        word: tr!("Agenda"),
        inset: Inset::Text,
        presses: true,
        selected: state.selected,
    };
    ui.allocate_ui(vec2(BLOCK_WIDTH, 400.0), |ui| {
        ui.set_width(BLOCK_WIDTH);
        block::block(ui, theme, &shown, (state.shown != Shown::Rest).then_some(state.shown), |ui| {
            for line in AGENDA {
                block::text_line(ui, theme, line);
            }
        })
    })
    .inner
}

/// The block's board: every state of its sheet, two to a line.
fn blocks(ui: &mut Ui, theme: &Theme, _keymap: &Keymap) {
    title(ui, theme, tr!("Block"));
    for pair in block_states().chunks(2) {
        cells(ui, theme, |ui| {
            for state in pair {
                cell(ui, theme, state.name, |ui| {
                    agenda(ui, theme, state);
                });
            }
        });
    }
}

/// One state of the block line's sheet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineState {
    pub name: &'static str,
    pub line: BlockLine<'static>,
    pub shown: Shown,
}

/// The block line's states, in the sheet's order, with the sheet's words.
pub(crate) fn line_states() -> [LineState; 3] {
    let more = LineState { name: "more", line: BlockLine::More(tr!("all drafts (5) →")), shown: Shown::Rest };
    [
        LineState { name: "empty", line: BlockLine::Empty(tr!("No drafts. What you start and do not send is kept here.")), ..more },
        more,
        LineState { name: "more-hover", shown: Shown::Hover, ..more },
    ]
}

/// The block line of `state`, `width` wide, on `surface`, at `ui`'s cursor; whether it was pressed.
pub(crate) fn line(ui: &mut Ui, theme: &Theme, keymap: &Keymap, state: &LineState, width: f32) -> bool {
    ui.allocate_ui(vec2(width, 200.0), |ui| {
        ui.set_width(width);
        let high = block_line::height(ui, theme, &state.line, width);
        ui.painter().rect_filled(Rect::from_min_size(ui.cursor().min, vec2(width, high)), 0.0, theme.palette.surface);
        ui.push_id(state.name, |ui| block_line::block_line(ui, theme, keymap, &state.line, (state.shown != Shown::Rest).then_some(state.shown))).inner
    })
    .inner
}

/// The block line's board: every state of its sheet, two to a line; then the drafts block of Home with nothing in it.
fn lines(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Block line"));
    for pair in line_states().chunks(2) {
        cells(ui, theme, |ui| {
            for state in pair {
                cell(ui, theme, state.name, |ui| {
                    line(ui, theme, keymap, state, BLOCK_WIDTH);
                });
            }
        });
    }
    cells(ui, theme, |ui| {
        cell(ui, theme, "drafts-empty", |ui| {
            drafts_empty(ui, theme, keymap);
        });
    });
}

/// Home's drafts block with nothing in it, 494 wide: its empty line edge to edge.
pub(crate) fn drafts_empty(ui: &mut Ui, theme: &Theme, keymap: &Keymap) -> block::Drawn<bool> {
    let shown =
        Block { id: Id::unique("gallery-drafts"), index: "[ 03 ]", word: tr!("Drafts"), inset: Inset::Flush, presses: false, selected: false };
    ui.allocate_ui(vec2(DRAFTS_WIDTH, 400.0), |ui| {
        ui.set_width(DRAFTS_WIDTH);
        block::block(ui, theme, &shown, None, |ui| {
            block_line::block_line(ui, theme, keymap, &BlockLine::Empty(tr!("No drafts. What you start and do not send is kept here.")), None)
        })
    })
    .inner
}

/// The left column: the count tile and the block line.
fn left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    tiles(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    lines(ui, theme, keymap);
}

/// The right column: the block.
fn right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    blocks(ui, theme, keymap);
}

/// Draws the page into `ui` in `theme` with the keys of `keymap`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let column = ((WIDTH - s.s2 * 3.0) / 2.0).floor();
    let draws: [(f32, Draw); 2] = [(s.s2, left), (s.s2 * 2.0 + column, right)];
    for (x, draw) in draws {
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
#[path = "gallery_home_tests.rs"]
mod tests;
