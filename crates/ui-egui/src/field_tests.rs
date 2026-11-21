// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The character field and the emblem: the fixed pattern, whole pixels, built once, hostile sizes, M1 and M2 at their
//! key frames at every level, rest, and that the field takes no click, key or focus.

use std::collections::BTreeSet;

use egui::epaint::TextShape;
use egui::{Event, Key, Pos2, Rect, Shape, Vec2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::*;
use crate::component::tests::{keyboard_walk, rests};
use crate::motion::tests::FRAME;
use crate::theme::{Mode, TextScale, Theme, install_fonts};

/// The field's rect in the harness.
const RECT: Rect = Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 960.0, y: 600.0 } };

/// Frames in 100 ms and in 200 ms, at [`FRAME`].
const MS_100: usize = 6;
const MS_200: usize = 12;

/// What the harness shows: a field and how it is shown.
struct Stage {
    field: Field,
    rect: Rect,
    level: Level,
    opts: Opts,
    mode: Mode,
    scale: TextScale,
    /// Presses of the button over the field, of the button under its edge, and of the key K.
    over: usize,
    under: usize,
    keys: usize,
    buttons: bool,
    /// Whether the button over the field and the one under its edge were hovered in the last frame.
    hovered: (bool, bool),
    /// Whether the field is drawn.
    shown: bool,
}

impl Stage {
    fn new(level: Level, opts: Opts) -> Self {
        Self {
            field: Field::new(7),
            rect: RECT,
            level,
            opts,
            mode: Mode::Dark,
            scale: TextScale::default(),
            over: 0,
            under: 0,
            keys: 0,
            buttons: false,
            hovered: (false, false),
            shown: true,
        }
    }
}

fn harness_at(stage: Stage, ppp: f32) -> Harness<'static, Stage> {
    let mut harness = Harness::<Stage>::builder()
        .with_context_setup(install_fonts)
        .with_step_dt(FRAME as f32)
        .with_max_steps(60)
        .with_size(vec2(1000.0, 700.0))
        .with_pixels_per_point(ppp)
        .build_ui_state(
            |ui, stage: &mut Stage| {
                let theme = Theme::new(stage.mode, stage.scale);
                // Under the field: drawn before it, across its bottom edge.
                if stage.buttons {
                    let under = Rect::from_min_size(pos2(500.0, 590.0), vec2(120.0, 32.0));
                    let response = ui.put(under, egui::Button::new("Under"));
                    stage.hovered.1 = response.hovered();
                    if response.clicked() {
                        stage.under += 1;
                    }
                }
                if stage.shown {
                    show(ui, &theme, &mut stage.field, stage.rect, stage.level, stage.opts);
                }
                if ui.input(|input| input.key_pressed(Key::K)) {
                    stage.keys += 1;
                }
                // Over the field: drawn after it.
                if stage.buttons {
                    let over = Rect::from_min_size(pos2(100.0, 100.0), vec2(120.0, 32.0));
                    let response = ui.put(over, egui::Button::new("Over"));
                    stage.hovered.0 = response.hovered();
                    if response.clicked() {
                        stage.over += 1;
                    }
                }
            },
            stage,
        );
    harness.run_steps(2);
    harness
}

fn harness(stage: Stage) -> Harness<'static, Stage> {
    harness_at(stage, 1.0)
}

/// The cells of a field over `rect` at `scale`, the emblem centred in `area` if it fits there whole, at `ppp`.
fn cells(rect: Rect, scale: TextScale, area: Option<Rect>, ppp: f32, seed: u32, emblem: Option<&Emblem>) -> Cells {
    let mut built = place_cells(rect, scale, area, ppp, emblem);
    fill(&mut built, seed, emblem);
    built
}

/// The cells a field over `rect` holds with `seed` at `scale`, one pixel a point.
fn built(rect: Rect, seed: u32, scale: TextScale) -> Cells {
    cells(rect, scale, None, 1.0, seed, None)
}

/// The warm cells: their spot, their glyph and whether they are out of the pointer's reach.
fn warm(harness: &Harness<'static, Stage>) -> Vec<(Spot, char, bool)> {
    let field = &harness.state().field;
    field.warm.iter().map(|(spot, warm)| (*spot, glyph(warm.glyph), warm.seen != field.frame)).collect()
}

/// The field's spots among `warm`.
fn field_spots(warm: &[(Spot, char, bool)]) -> Vec<Spot> {
    warm.iter().filter(|(spot, ..)| matches!(spot, Spot::Field(_))).map(|(spot, ..)| *spot).collect()
}

/// The centre of the field's cell `spot` in `harness`.
fn centre(harness: &Harness<'static, Stage>, spot: Spot) -> Pos2 {
    let cells = harness.state().field.cells.as_ref().unwrap();
    let index = spot_index(spot) as usize;
    let (row, column) = (index / cells.columns, index % cells.columns);
    pos2(cells.origin.x + (column as f32 + 0.5) * CELL_WIDTH, row_top(cells, row) + cells.height / 2.0)
}

/// How many of the field's cells lie within `reach` of `at` and are not under the emblem.
fn within(harness: &Harness<'static, Stage>, at: Pos2, reach: f32) -> usize {
    let cells = harness.state().field.cells.as_ref().unwrap();
    (0..cells.held.len()).filter(|index| cells.held[*index] != UNDER && centre(harness, Spot::Field(*index as u32)).distance(at) <= reach).count()
}

/// Whether a repaint is asked for within a second.
fn repaints(harness: &Harness<'static, Stage>) -> bool {
    harness.output().viewport_output.values().any(|viewport| viewport.repaint_delay < std::time::Duration::from_secs(1))
}

/// The text shapes of the last frame.
fn texts(harness: &Harness<'static, Stage>) -> Vec<TextShape> {
    fn walk(shape: &Shape, found: &mut Vec<TextShape>) {
        match shape {
            Shape::Text(text) => found.push(text.clone()),
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, &mut found);
    }
    found
}

/// An emblem of 76 × 40 cells, every cell on, a cyan block in its middle: 547 × 500 points.
fn block() -> Emblem {
    let ink = "#".repeat(76);
    let cyan = format!("{}{}{}", "#".repeat(30), "9".repeat(16), "#".repeat(30));
    let rows: Vec<&str> = (0..40).map(|row| if (15..25).contains(&row) { cyan.as_str() } else { ink.as_str() }).collect();
    Emblem::from_rows(&rows)
}

/// A stage with the block emblem centred in the right part of the field.
fn with_emblem(level: Level) -> Stage {
    let area = Rect::from_min_max(pos2(380.0, 40.0), pos2(960.0, 580.0));
    let mut stage = Stage::new(level, Opts { on_home: true, on_pane: false, emblem: Some(area) });
    stage.field = Field::new(7).with_emblem(block());
    stage
}

#[test]
fn the_same_rect_seed_and_scale_give_the_same_cells_and_another_seed_others() {
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 825.0));
    let one = built(rect, 7, TextScale::default());
    let two = built(rect, 7, TextScale::default());
    assert_eq!(one.held, two.held);
    assert_ne!(one.held, built(rect, 8, TextScale::default()).held, "another seed gives the same pattern");
    // A field shown twice, in two contexts, holds the same cells.
    let a = harness(Stage::new(Level::Off, Opts::default()));
    let b = harness(Stage::new(Level::Off, Opts::default()));
    assert_eq!(a.state().field.cells.as_ref().unwrap().held, b.state().field.cells.as_ref().unwrap().held);
}

/// The columns of `row` that hold a dot among `cells`.
fn dots(cells: &Cells, row: usize) -> Vec<usize> {
    (0..cells.columns).filter(|column| cells.held[row * cells.columns + column] == DOT).collect()
}

#[test]
fn seed_0_is_the_design_files_pattern() {
    // Rows of the character field of `screen/home/today` (150 × 55 cells), read from the design file: the columns
    // that hold a `·`, across the whole row. Rows far apart, so the row's part of the hash shows.
    let design: [(usize, &[usize]); 4] = [
        (0, &[0, 14, 28, 57, 72, 86, 100, 144]),
        (1, &[3, 27, 63, 75, 82, 90, 117, 149]),
        (30, &[27, 38, 130, 141]),
        (54, &[0, 42, 50, 62, 66, 68, 102, 110, 118, 122]),
    ];
    let cells = built(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 825.0)), 0, TextScale::default());
    assert_eq!((cells.columns, cells.rows), (150, 55));
    for (row, columns) in design {
        assert_eq!(dots(&cells, row), columns, "row {row}");
    }
    // Another seed: the same rows, worked out from the hash apart from this code (the design file draws only seed 0).
    let other: [(usize, &[usize]); 3] =
        [(1, &[20, 43, 58, 85, 93, 147]), (30, &[5, 37, 81, 87, 97, 99, 101, 115, 122, 141]), (54, &[40, 57, 62, 69, 76, 88, 110])];
    let cells = built(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 825.0)), 7, TextScale::default());
    for (row, columns) in other {
        assert_eq!(dots(&cells, row), columns, "seed 7, row {row}");
    }
}

#[test]
fn about_5_in_100_cells_hold_a_dot() {
    // A 4K rect: 480 × 144 cells. The share lies within half a point of 5 in 100.
    let cells = built(Rect::from_min_size(Pos2::ZERO, vec2(3840.0, 2160.0)), 7, TextScale::default());
    assert_eq!((cells.columns, cells.rows), (480, 144));
    let dots = cells.held.iter().filter(|held| **held == DOT).count();
    let share = dots as f64 / cells.held.len() as f64;
    assert!((0.045..=0.055).contains(&share), "{dots} dots in {} cells: {share}", cells.held.len());
}

#[test]
fn at_130_percent_the_cell_keeps_its_width_and_takes_the_texts_height() {
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let normal = built(rect, 7, TextScale::default());
    let large = built(rect, 7, TextScale::from_percent(130).unwrap());
    assert_eq!((normal.height, large.height), (15.0, 20.0));
    assert_eq!((normal.columns, normal.rows), (100, 40));
    assert_eq!((large.columns, large.rows), (100, 30));
}

#[test]
fn every_cell_sits_on_a_whole_pixel_at_every_scale() {
    for ppp in [1.0, 1.25, 1.5, 2.0] {
        let mut stage = Stage::new(Level::Full, Opts::default());
        // A rect off the pixel grid: the field starts at the next whole pixel.
        stage.rect = Rect::from_min_max(pos2(10.3, 20.7), pos2(900.0, 650.0));
        let mut harness = harness_at(stage, ppp);
        harness.hover_at(pos2(300.0, 300.0));
        harness.step();
        let texts = texts(&harness);
        assert!(texts.len() > 30, "{ppp}: {} rows painted", texts.len());
        let origin = harness.state().field.cells.as_ref().unwrap().origin;
        let whole = |points: f32| ((points * ppp) - (points * ppp).round()).abs() < 1e-3;
        for text in &texts {
            assert!(whole(text.pos.x) && whole(text.pos.y), "{ppp}: a row at {:?}", text.pos);
            assert_eq!(text.pos.x, origin.x, "{ppp}: a row starts off the first column");
            for row in &text.galley.rows {
                for glyph in &row.glyphs {
                    let x = text.pos.x + row.pos.x + glyph.pos.x;
                    assert!(whole(x), "{ppp}: {:?} at {x}", glyph.chr);
                    let column = (x - origin.x) / CELL_WIDTH;
                    assert!((column - column.round()).abs() < 1e-3, "{ppp}: {:?} between two columns at {x}", glyph.chr);
                }
            }
        }
    }
}

#[test]
fn the_cells_are_built_once_and_a_frame_at_rest_lays_out_nothing() {
    let mut harness = harness(Stage::new(Level::Full, Opts::default()));
    for frame in 0..40 {
        harness.hover_at(pos2(100.0 + frame as f32 * 10.0, 300.0));
        harness.step();
    }
    harness.hover_at(pos2(990.0, 690.0));
    harness.run_steps(MS_200 + 2);
    assert_eq!(harness.state().field.builds, 1, "the cells were built again while the pointer moved");
    let layouts = harness.state().field.layouts;
    harness.run_steps(10);
    assert_eq!(harness.state().field.layouts, layouts, "a frame at rest laid rows out");
    // A new rect builds again; a new mode lays out again but builds nothing.
    harness.state_mut().mode = Mode::Light;
    harness.run_steps(2);
    assert_eq!(harness.state().field.builds, 1);
    assert!(harness.state().field.layouts > layouts, "the light mode kept the dark rows");
    harness.state_mut().rect = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    harness.run_steps(2);
    assert_eq!(harness.state().field.builds, 2);
}

/// The warm cells of the field by their column and row, whatever its columns: their glyph and whether they cool.
fn warm_by_place(harness: &Harness<'static, Stage>) -> Vec<((usize, usize), char, bool)> {
    let columns = harness.state().field.cells.as_ref().unwrap().columns;
    let mut found: Vec<_> = warm(harness)
        .into_iter()
        .filter_map(|(spot, glyph, cooling)| match spot {
            Spot::Field(index) => Some(((index as usize % columns, index as usize / columns), glyph, cooling)),
            Spot::Emblem(_) => None,
        })
        .collect();
    found.sort_by_key(|(place, ..)| *place);
    found
}

#[test]
fn a_rect_drifting_by_parts_of_a_pixel_builds_once_and_keeps_its_warm_glyphs() {
    // 964 points wide: 120 columns wherever within a pixel the rect starts. The cells under a still pointer keep
    // flipping, so the drifting field is held against one that stands still, the pointer at the same place on both.
    let rect = |left: f32| Rect::from_min_size(pos2(left, 0.0), vec2(964.0, 600.0));
    let mut stage = Stage::new(Level::Full, Opts::default());
    stage.rect = rect(0.0);
    let mut harness = harness(stage);
    let mut still = Stage::new(Level::Full, Opts::default());
    still.rect = rect(0.0);
    let mut still = self::harness(still);
    for one in [&mut harness, &mut still] {
        one.hover_at(pos2(300.0, 300.0));
        one.step();
    }
    let first = warm_by_place(&harness);
    assert!(!first.is_empty() && first.iter().all(|(.., cooling)| !cooling));
    assert_eq!(first, warm_by_place(&still));
    // 0.3 points a frame for 30 frames, the pointer kept where it stands on the cells.
    for frame in 1..=30 {
        let left = frame as f32 * 0.3;
        harness.state_mut().rect = rect(left);
        harness.hover_at(pos2(300.0 + left.ceil(), 300.0));
        harness.step();
        still.step();
        assert_eq!(warm_by_place(&harness), warm_by_place(&still), "frame {frame}: the warm cells changed as the rect moved");
    }
    // The pointer still, the rect's right edge jittering by a hundredth of a point.
    for frame in 0..30 {
        harness.state_mut().rect.max.x = 973.0 + if frame % 2 == 0 { 0.01 } else { 0.0 };
        harness.step();
        still.step();
        assert_eq!(warm_by_place(&harness), warm_by_place(&still), "frame {frame}: the warm cells changed as the rect jittered");
    }
    assert_eq!(harness.state().field.builds, 1, "a rect that only moved built the cells again");
}

/// Home's field and the emblem's place move together (its rect is the place): wherever within a pixel they stand, at
/// every usual pixels per point, the emblem keeps its offset from the first cell, on a whole pixel, so the cells are
/// never built again for a move alone.
#[test]
fn a_field_and_its_emblem_moving_together_by_parts_of_a_pixel_keep_their_cells() {
    let emblem = block();
    for ppp in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let at = |step: u16| {
            let left = f32::from(step) / 200.0;
            let rect = Rect::from_min_size(pos2(833.0 + left, 46.0 + left / 2.0), vec2(605.0, 826.0));
            place_cells(rect, TextScale::default(), Some(rect), ppp, Some(&emblem))
        };
        let first = at(0);
        assert!(first.emblem_at.is_some(), "{ppp}: no emblem");
        let moved = (1..600).filter(|step| !at(*step).same_cells(&first)).count();
        assert_eq!(moved, 0, "{ppp}: {moved} of 600 positions build the cells again");
        for step in [0, 137, 599] {
            let corner = at(step).emblem_at.unwrap() * ppp;
            let off = (corner.x - corner.x.round()).abs().max((corner.y - corner.y.round()).abs());
            assert!(off < 1e-3, "{ppp}: the emblem {off} px off a whole pixel");
        }
    }
}

#[test]
fn a_rect_a_cell_wider_builds_again_and_the_warm_cells_keep_their_glyphs() {
    let mut harness = harness(Stage::new(Level::Full, Opts::default()));
    harness.hover_at(pos2(300.0, 300.0));
    harness.step();
    let first = warm_by_place(&harness);
    assert!(!first.is_empty());
    assert_eq!(harness.state().field.cells.as_ref().unwrap().columns, 120);
    harness.state_mut().rect.max.x += CELL_WIDTH;
    harness.step();
    assert_eq!(harness.state().field.cells.as_ref().unwrap().columns, 121);
    assert_eq!(harness.state().field.builds, 2);
    assert_eq!(warm_by_place(&harness), first, "the warm cells changed when the field grew a column");
    // A row more: the same.
    harness.state_mut().rect.max.y += CELL_HEIGHT;
    harness.step();
    assert_eq!(harness.state().field.builds, 3);
    assert_eq!(warm_by_place(&harness), first, "the warm cells changed when the field grew a row");
}

#[test]
fn pixels_per_point_that_are_not_usable_build_once() {
    // Stored sanitised and compared with what is stored: not built again on every frame.
    let theme = Theme::new(Mode::Dark, TextScale::default());
    for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut field = Field::new(7);
        for _ in 0..5 {
            field.build(RECT, TextScale::default(), None, ppp);
            field.lay_out(&theme);
        }
        assert_eq!(field.builds, 1, "{ppp}: built again on every frame");
        assert!(field.cells.as_ref().unwrap().held.is_empty(), "{ppp}: cells built");
    }
}

#[test]
fn a_4k_field_builds_its_cells_once_and_a_text_a_row() {
    // The work counted, not the time (the time is the bench's): one build of 480 × 144 cells in one allocation of
    // their size, and one text a row, each about as long as its dots.
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(3840.0, 2160.0));
    let mut field = Field::new(7);
    for _ in 0..3 {
        field.build(rect, TextScale::default(), None, 1.0);
        field.lay_out(&theme);
    }
    let cells = field.cells.as_ref().unwrap();
    assert_eq!(field.builds, 1);
    assert_eq!((cells.held.len(), cells.held.capacity()), (480 * 144, 480 * 144));
    let look = field.look.as_ref().unwrap();
    assert_eq!(look.rows.len(), 144);
    let texts = look.rows.iter().flatten().count();
    assert_eq!(field.layouts, texts as u64, "a row's text was built more than once");
    let dots = cells.held.iter().filter(|held| **held == DOT).count();
    let characters: usize = look.rows.iter().flatten().map(|job| job.text.chars().count()).sum();
    // A dot and at most the three spaces kept before it.
    assert!(characters <= dots * 4, "{characters} characters for {dots} dots");
}

#[test]
fn a_hostile_size_is_capped_or_draws_nothing() {
    let nothing = [
        Rect::NOTHING,
        Rect::from_min_size(Pos2::ZERO, Vec2::ZERO),
        Rect::from_min_max(pos2(100.0, 100.0), pos2(0.0, 0.0)),
        Rect::from_min_size(pos2(f32::NAN, 0.0), vec2(100.0, 100.0)),
        Rect::from_min_size(Pos2::ZERO, vec2(f32::NAN, f32::INFINITY)),
        Rect::EVERYTHING,
    ];
    for rect in nothing {
        let cells = cells(rect, TextScale::default(), Some(rect), 1.0, 7, Some(&block()));
        assert!(cells.held.is_empty() && cells.emblem_at.is_none(), "{rect:?} built {} cells", cells.held.len());
    }
    let huge = cells(Rect::from_min_size(Pos2::ZERO, vec2(1e9, 1e9)), TextScale::default(), None, 1.0, 7, None);
    assert_eq!((huge.columns, huge.rows), (MAX_COLUMNS, MAX_ROWS));
    // The pixels per point are hostile too.
    for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY, MAX_PPP * 2.0, f32::MAX] {
        assert!(cells(RECT, TextScale::default(), None, ppp, 7, None).held.is_empty());
    }
    // A rect across all of f32 at 1, 1.25 and 2 pixels a point: capped, and every place in it finite.
    for ppp in [1.0, 1.25, 2.0] {
        for reach in [f32::MAX, 3e38] {
            let rect = Rect::from_min_max(pos2(-reach, -reach), pos2(reach, reach));
            let cells = cells(rect, TextScale::default(), Some(rect), ppp, 7, Some(&block()));
            assert_eq!((cells.columns, cells.rows), (MAX_COLUMNS, MAX_ROWS), "{ppp}, {reach}");
            assert!(cells.origin.x.is_finite() && cells.origin.y.is_finite(), "{ppp}, {reach}: the origin is {:?}", cells.origin);
            assert!(cells.emblem_at.is_none_or(|at| at.x.is_finite() && at.y.is_finite()), "{ppp}, {reach}: {:?}", cells.emblem_at);
            let mut stage = Stage::new(Level::Full, Opts { on_home: true, on_pane: false, emblem: Some(rect) });
            stage.rect = rect;
            stage.field = Field::new(7).with_emblem(block());
            let mut harness = harness_at(stage, ppp);
            harness.hover_at(pos2(50.0, 50.0));
            harness.run_steps(2);
            harness.hover_at(pos2(500.0, 300.0));
            harness.run_steps(2);
        }
    }
    // Shown, none of them panics, the pointer on them or not.
    for rect in nothing.into_iter().chain([Rect::from_min_size(Pos2::ZERO, vec2(1e9, 1e9))]) {
        let mut stage = Stage::new(Level::Full, Opts { on_home: true, on_pane: true, emblem: Some(rect) });
        stage.rect = rect;
        let mut harness = harness(stage);
        harness.hover_at(pos2(50.0, 50.0));
        harness.run_steps(3);
    }
    // An emblem handed in too large is cut to its caps.
    let wide = "@".repeat(10_000);
    let rows = vec![wide.as_str(); 10_000];
    assert_eq!(Emblem::from_rows(&rows).grid(), (MAX_EMBLEM_COLUMNS, MAX_EMBLEM_ROWS));
}

/// The pointer comes in at `at` at `level`, then moves on to `away`: the warm cells at 0 ms, at 100 ms and at 200 ms
/// after it moved on.
fn key_frames(level: Level, on_home: bool, reach: f32) {
    let at = pos2(300.0, 300.0);
    let away = pos2(700.0, 300.0);
    let mut harness = harness(Stage::new(level, Opts { on_home, ..Opts::default() }));
    assert!(warm(&harness).is_empty(), "before: something is warm");
    // The cells within reach flip at once (at reduced, each with 0.4 of the chance, within 400 ms); none beyond.
    harness.hover_at(at);
    harness.step();
    let mut first = warm(&harness);
    for _ in 1..MS_200 * 2 {
        if reach <= 0.0 || level == Level::Full || !first.is_empty() {
            break;
        }
        harness.step();
        first = warm(&harness);
    }
    let spots = field_spots(&first);
    if reach <= 0.0 {
        assert!(first.is_empty(), "{level:?}: the field answered the pointer");
        assert!(!repaints(&harness) || rests(&mut harness, 4), "{level:?}: the still field does not rest");
        return;
    }
    let near = within(&harness, at, reach);
    assert!(!spots.is_empty() && spots.len() * 100 <= near * 40, "{level:?}: {} of {near} cells within {reach} flipped", spots.len());
    for spot in &spots {
        assert!(centre(&harness, *spot).distance(at) <= reach, "{level:?}: {spot:?} flipped beyond {reach}");
    }
    assert!(first.iter().all(|(.., cooling)| !cooling));
    assert!(first.iter().any(|(_, glyph, _)| *glyph != '·'));
    // The pointer stays for a second: the cells within reach keep flipping and going back, a frame is asked for each
    // time, and never a cell beyond reach is warm.
    let mut changes = 0;
    let mut ever = BTreeSet::new();
    let mut last = first.clone();
    for _ in 0..60 {
        harness.step();
        assert!(repaints(&harness), "{level:?}: no frame asked for under a still pointer");
        let now = warm(&harness);
        let spots = field_spots(&now);
        assert!(spots.iter().all(|spot| centre(&harness, *spot).distance(at) <= reach), "{level:?}: a cell beyond reach is warm");
        assert!(spots.len() < near, "{level:?}: all {near} warm under a still pointer");
        changes += usize::from(now != last);
        ever.extend(spots);
        last = now;
    }
    // At reduced each cell flips with 0.4 of the chance: the field changes less often.
    let (most_still, least_flipped) = if level == Level::Full { (20, 45) } else { (45, 25) };
    assert!(changes + most_still >= 60, "{level:?}: the field changed in {changes} of 60 frames under a still pointer");
    assert!(ever.len() * 100 >= near * least_flipped, "{level:?}: {} of {near} cells within reach flipped in a second", ever.len());
    // The pointer moves on: the cells it left take new glyphs as they go back, each at its own time; some are still
    // flipped at 100 ms (each stays 80 to 200 ms after its flip), none at 200 ms.
    let before = field_spots(&warm(&harness));
    harness.hover_at(away);
    harness.step();
    assert!(repaints(&harness), "{level:?}: no frame asked for while cells cool");
    harness.run_steps(MS_100 - 1);
    assert!(repaints(&harness), "{level:?}: no frame asked for while the trail cools");
    let mid = warm(&harness);
    let left: Vec<_> = mid.iter().filter(|(spot, ..)| before.contains(spot)).collect();
    assert!(left.iter().all(|(.., cooling)| *cooling));
    assert!(!left.is_empty() && left.len() * 4 <= before.len() * 3, "{level:?}: {} of {} still warm at 100 ms", left.len(), before.len());
    // 200 ms after it moved on the trail has cooled; once the pointer has left the window too, the field rests.
    harness.run_steps(MS_200 - MS_100 + 1);
    assert!(warm(&harness).iter().all(|(spot, ..)| !before.contains(spot)), "{level:?}: the trail is still warm at 200 ms");
    harness.event(Event::PointerGone);
    harness.run_steps(MS_200 + 1);
    assert!(warm(&harness).is_empty(), "{level:?}: warm 200 ms after the pointer left the window");
    assert!(rests(&mut harness, 3), "{level:?}: the field does not rest once the trail has cooled");
}

#[test]
fn m1_at_full_flips_the_cells_within_60_and_cools_them_in_200_ms() {
    key_frames(Level::Full, false, REACH_FULL);
}

#[test]
fn m1_at_reduced_on_home_flips_the_cells_within_30() {
    key_frames(Level::Reduced, true, REACH_REDUCED);
}

#[test]
fn m1_at_reduced_elsewhere_and_at_off_is_still_dots() {
    key_frames(Level::Reduced, false, 0.0);
    key_frames(Level::Off, true, 0.0);
    key_frames(Level::Off, false, 0.0);
}

#[test]
fn the_pointer_leaving_the_window_cools_everything_it_warmed() {
    for level in [Level::Full, Level::Reduced] {
        let mut stage = with_emblem(level);
        stage.opts.on_home = true;
        let mut harness = harness(stage);
        harness.hover_at(pos2(380.0 + 290.0 - 30.0, 300.0));
        harness.step();
        assert!(!warm(&harness).is_empty());
        harness.event(Event::PointerGone);
        harness.step();
        let before = warm(&harness);
        assert!(before.iter().all(|(.., cooling)| *cooling));
        assert!(repaints(&harness));
        harness.run_steps(MS_200);
        // The emblem twinkles on by itself: only the cells warm when the pointer left are cold.
        let stale = warm(&harness).into_iter().filter(|(spot, ..)| before.iter().any(|(was, ..)| was == spot)).count();
        assert_eq!(stale, 0, "{level:?}: still warm after the pointer left");
        assert!(field_spots(&warm(&harness)).is_empty(), "{level:?}: a cell of the field is warm with the pointer gone");
    }
}

#[test]
fn a_field_drawn_again_after_a_pause_shows_no_cell_that_cooled_meanwhile() {
    let mut harness = harness(Stage::new(Level::Full, Opts::default()));
    harness.hover_at(pos2(300.0, 300.0));
    harness.step();
    let held = field_spots(&warm(&harness));
    assert!(!held.is_empty());
    // Not drawn for 300 ms, the pointer gone elsewhere: drawn again, none of the cells it held is warm.
    harness.state_mut().shown = false;
    harness.hover_at(pos2(700.0, 300.0));
    harness.run_steps(MS_200 + MS_100);
    harness.state_mut().shown = true;
    harness.step();
    let stale: Vec<_> = field_spots(&warm(&harness)).into_iter().filter(|spot| held.contains(spot)).collect();
    assert!(stale.is_empty(), "{} cells held before the pause are still warm", stale.len());
}

/// A field over [`RECT`] with its cells built, and the cells flipped under the pointer at (300, 300) at 1 s in pass
/// 10.
fn flipped_at_1() -> (Field, BTreeSet<Spot>) {
    let mut field = Field::new(7);
    field.build(RECT, TextScale::default(), None, 1.0);
    field.update(1.0, 10, Some(pos2(300.0, 300.0)), Level::Full, false);
    let flipped: BTreeSet<Spot> = field.warm.keys().copied().collect();
    assert!(!flipped.is_empty());
    (field, flipped)
}

#[test]
fn a_flipped_cell_is_back_80_to_200_ms_after_its_flip_with_the_pointer_still_there() {
    let (mut field, flipped) = flipped_at_1();
    let mut back = 0;
    for step in 1..=14_u32 {
        let now = 1.0 + f64::from(step) * FRAME;
        field.update(now, u64::from(step) + 10, Some(pos2(300.0, 300.0)), Level::Full, false);
        for spot in &flipped {
            let warm = field.warm.get(spot);
            // Within 80 ms nothing is back: a cell flips again or stays.
            assert!(now >= 1.0 + HOT.0 || warm.is_some(), "{spot:?} back at {now} s");
            // From 200 ms on no flip of the first second is left.
            assert!(now < 1.0 + HOT.1 || warm.is_none_or(|warm| warm.at > 1.0), "{spot:?} still flipped at {now} s");
            back += usize::from(warm.is_none());
        }
    }
    assert!(back > 0, "no cell went back under the still pointer");
}

#[test]
fn a_field_not_drawn_for_a_while_shows_no_cell_that_went_back_meanwhile() {
    // Passes ran for 0.5 s without the field: drawn again, the pointer elsewhere, the cells it flipped are back.
    let (mut field, flipped) = flipped_at_1();
    field.update(1.5, 41, Some(pos2(700.0, 300.0)), Level::Full, false);
    assert!(field.warm.keys().all(|spot| !flipped.contains(spot)), "cells flipped before the field was hidden are still warm");
}

#[test]
fn a_clock_that_jumps_back_lets_the_field_rest_at_once() {
    let (mut field, _) = flipped_at_1();
    field.update(1.05, 11, None, Level::Full, false);
    assert!(!field.warm.is_empty());
    // 9 s back: no cell stays flipped until the clock catches up.
    field.update(1.05 - 9.0, 12, None, Level::Full, false);
    assert!(field.warm.is_empty() && field.next_change(1.05 - 9.0).is_none(), "the field asks for frames until the clock catches up");
}

#[test]
fn the_level_turning_off_stills_everything_at_once() {
    let mut harness = harness(with_emblem(Level::Full));
    harness.hover_at(pos2(370.0, 300.0));
    harness.step();
    assert!(!warm(&harness).is_empty());
    harness.state_mut().level = Level::Off;
    harness.step();
    assert!(warm(&harness).is_empty());
}

/// The emblem's warm spots.
fn emblem_spots(harness: &Harness<'static, Stage>) -> Vec<(Spot, bool)> {
    warm(harness).into_iter().filter(|(spot, ..)| matches!(spot, Spot::Emblem(_))).map(|(spot, _, cooling)| (spot, cooling)).collect()
}

#[test]
fn m2_the_emblems_cells_flip_under_the_pointer_and_settle_back_at_full_and_reduced() {
    let mut seen = Vec::new();
    for level in [Level::Full, Level::Reduced] {
        let mut harness = harness(with_emblem(level));
        let corner = harness.state().field.cells.as_ref().unwrap().emblem_at.unwrap();
        let at = corner + vec2(200.0, 250.0);
        harness.hover_at(at);
        harness.step();
        let first = emblem_spots(&harness);
        assert!(!first.is_empty(), "{level:?}: the emblem did not answer");
        for (spot, _) in &first {
            let index = spot_index(*spot) as usize;
            let centre = corner + vec2(((index % 76) as f32 + 0.5) * EMBLEM_CELL.x, ((index / 76) as f32 + 0.5) * EMBLEM_CELL.y);
            assert!(centre.distance(at) <= EMBLEM_REACH, "{level:?}: {spot:?} flipped beyond 60");
        }
        // Where the mark stands only its own cells react.
        let cells = harness.state().field.cells.as_ref().unwrap();
        for spot in field_spots(&warm(&harness)) {
            assert_ne!(cells.held[spot_index(spot) as usize], UNDER, "{level:?}: a field cell under the mark flipped");
        }
        harness.hover_at(corner + vec2(-200.0, 250.0));
        harness.step();
        let before = emblem_spots(&harness);
        assert!(before.iter().all(|(.., cooling)| *cooling));
        harness.run_steps(MS_200);
        // The emblem twinkles on by itself: only the cells the pointer warmed have settled.
        let stale = emblem_spots(&harness).into_iter().filter(|(spot, _)| before.iter().any(|(was, _)| was == spot)).count();
        assert_eq!(stale, 0, "{level:?}: the mark did not settle back in 200 ms");
        seen.push(first);
    }
    assert_eq!(seen[0], seen[1], "the emblem answers differently at full and at reduced");
}

#[test]
fn m2_at_off_the_emblem_is_still() {
    let mut harness = harness(with_emblem(Level::Off));
    let corner = harness.state().field.cells.as_ref().unwrap().emblem_at.unwrap();
    harness.hover_at(corner + vec2(200.0, 250.0));
    harness.step();
    assert!(warm(&harness).is_empty());
    assert!(rests(&mut harness, 3));
}

#[test]
fn the_emblem_is_drawn_only_where_it_fits_whole() {
    let emblem = block();
    assert_eq!(emblem.size(), vec2(547.0, 500.0));
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0));
    let area = |width: f32, height: f32| Some(Rect::from_min_size(pos2(100.0, 100.0), vec2(width, height)));
    let at = |width, height| cells(rect, TextScale::default(), area(width, height), 1.0, 7, Some(&emblem)).emblem_at;
    assert_eq!(at(547.0, 500.0), Some(pos2(100.0, 100.0)));
    assert_eq!(at(546.0, 500.0), None, "cut at its right edge");
    assert_eq!(at(547.0, 499.0), None, "cut at its bottom edge");
    assert_eq!(at(647.0, 600.0), Some(pos2(150.0, 150.0)), "not centred");
    // An area reaching out of the field's rect does not draw it either.
    let out = Some(Rect::from_min_size(pos2(700.0, 100.0), vec2(600.0, 500.0)));
    assert_eq!(cells(rect, TextScale::default(), out, 1.0, 7, Some(&emblem)).emblem_at, None);
    // Where it is drawn the field's cells under it stay empty; where not, they keep their dots.
    let drawn = cells(rect, TextScale::default(), area(547.0, 500.0), 1.0, 7, Some(&emblem));
    let not = cells(rect, TextScale::default(), area(546.0, 500.0), 1.0, 7, Some(&emblem));
    assert!(drawn.held.contains(&UNDER) && !not.held.contains(&UNDER));
    // Painted: its 40 rows show only where it fits.
    let mut stage = with_emblem(Level::Off);
    stage.opts.emblem = Some(Rect::from_min_size(pos2(380.0, 40.0), vec2(547.0, 500.0)));
    let mut harness = harness(stage);
    let rows = |harness: &Harness<'static, Stage>| {
        texts(harness).iter().filter(|text| text.galley.rows.iter().any(|row| row.glyphs.iter().any(|glyph| glyph.chr == '#'))).count()
    };
    assert_eq!(rows(&harness), 40);
    harness.state_mut().opts.emblem = Some(Rect::from_min_size(pos2(380.0, 40.0), vec2(546.0, 500.0)));
    harness.run_steps(2);
    assert_eq!(rows(&harness), 0, "a cut emblem was drawn");
}

#[test]
fn the_field_takes_no_click_no_key_and_no_focus() {
    let mut stage = with_emblem(Level::Full);
    stage.buttons = true;
    let mut harness = harness(stage);
    // The pointer on the button over the field and on the one under its edge hovers it as if the field were not
    // there, with cells warm under the pointer.
    harness.hover_at(pos2(160.0, 116.0));
    harness.run_steps(2);
    assert!(!warm(&harness).is_empty(), "no cell warm under the button over the field");
    assert_eq!(harness.state().hovered, (true, false), "the button over the field is not hovered");
    harness.hover_at(pos2(560.0, 596.0));
    harness.run_steps(2);
    assert!(!warm(&harness).is_empty(), "no cell warm under the button under the field's edge");
    assert_eq!(harness.state().hovered, (false, true), "the button under the field's edge is not hovered");
    // Nothing of the field is a node: the two buttons are all a screen reader or Tab finds.
    assert_eq!(harness.query_all_by_role(egui::accesskit::Role::Button).count(), 2);
    let pressable = harness
        .root()
        .children_recursive()
        .filter(|node| {
            let data = node.accesskit_node();
            data.data().supports_action(egui::accesskit::Action::Click) || data.data().supports_action(egui::accesskit::Action::Focus)
        })
        .count();
    assert_eq!(pressable, 2, "the field is something a screen reader or Tab lands on");
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "Tab never reaches {missed:?}");
    // A press on the button over the field and on the one under its edge lands as if the field were not there,
    // with cells warm under the pointer.
    harness.get_by_label("Over").click();
    harness.run_steps(2);
    harness.get_by_label("Under").click();
    harness.run_steps(2);
    assert_eq!((harness.state().over, harness.state().under), (1, 1));
}

#[test]
fn a_key_pressed_while_cells_are_warm_acts_in_that_frame() {
    let mut harness = harness(Stage::new(Level::Full, Opts::default()));
    harness.hover_at(pos2(300.0, 300.0));
    harness.step();
    harness.hover_at(pos2(600.0, 300.0));
    harness.step();
    assert!(warm(&harness).iter().any(|(.., cooling)| *cooling), "nothing is cooling");
    harness.key_press(Key::K);
    harness.step();
    assert_eq!(harness.state().keys, 1, "the key waited for the field");
}

#[test]
fn on_a_pane_the_field_shows_at_20_percent() {
    let mut harness = harness(Stage::new(Level::Off, Opts { on_pane: true, ..Opts::default() }));
    harness.step();
    let texts = texts(&harness);
    assert!(!texts.is_empty() && texts.iter().all(|text| text.opacity_factor == PANE_STRENGTH));
    harness.state_mut().opts.on_pane = false;
    harness.step();
    assert!(self::texts(&harness).iter().all(|text| text.opacity_factor == 1.0));
}

#[test]
fn after_the_fonts_change_every_row_is_drawn_with_the_new_atlas() {
    // egui drops its glyph atlas when the fonts change (or the atlas fills up): a galley kept from before would draw
    // the wrong glyphs. Every painted row is what the current fonts lay out for its text.
    let mut harness = harness(with_emblem(Level::Off));
    harness.ctx.set_fonts(egui::FontDefinitions::default());
    harness.run_steps(3);
    let painted = texts(&harness);
    assert!(painted.len() > 40);
    for text in painted {
        let fresh = harness.ctx.fonts_mut(|fonts| fonts.layout_job((*text.galley.job).clone()));
        let meshes = |galley: &egui::Galley| galley.rows.iter().map(|row| row.visuals.mesh.clone()).collect::<Vec<_>>();
        assert_eq!(meshes(&text.galley), meshes(&fresh), "a row drawn with glyphs of the old atlas");
    }
}

/// A field over [`RECT`] with the block emblem where [`with_emblem`] puts it, its cells built at one pixel a point.
fn emblem_field() -> Field {
    let area = Rect::from_min_max(pos2(380.0, 40.0), pos2(960.0, 580.0));
    let mut field = Field::new(7).with_emblem(block());
    field.build(RECT, TextScale::default(), Some(area), 1.0);
    assert!(field.emblem_at().is_some());
    field
}

/// A twinkle started: its spot, whether it took the other colour, when it started and how long it lasts.
type Twinkle = (Spot, bool, f64, f64);

/// The twinkles `field` starts in `seconds` from input time 1, updated `hz` times a second at `level` with the pointer
/// away, and the glyphs each of them showed.
fn run_away(field: &mut Field, hz: f64, seconds: f64, level: Level, on_home: bool) -> (Vec<Twinkle>, Vec<BTreeSet<u8>>) {
    let mut started: Vec<Twinkle> = Vec::new();
    let mut glyphs: Vec<BTreeSet<u8>> = Vec::new();
    let mut known: BTreeMap<(Spot, u64), usize> = BTreeMap::new();
    for step in 0..=(seconds * hz).round() as u64 {
        field.update(1.0 + step as f64 / hz, step + 1, None, level, on_home);
        for (spot, warm) in &field.warm {
            let at = *known.entry((*spot, warm.at.to_bits())).or_insert_with(|| {
                started.push((*spot, warm.tint, warm.at, warm.until - warm.at));
                glyphs.push(BTreeSet::new());
                glyphs.len() - 1
            });
            glyphs[at].insert(warm.glyph);
        }
    }
    (started, glyphs)
}

#[test]
fn m2_the_emblem_twinkles_by_itself_about_7_5_times_a_second_at_any_frame_rate() {
    for hz in [60.0, 20.0, 144.0] {
        let mut field = emblem_field();
        let (started, _) = run_away(&mut field, hz, 20.0, Level::Full, true);
        assert!((120..=180).contains(&started.len()), "{hz} Hz: {} twinkles in 20 s", started.len());
        assert!(started.iter().all(|(spot, ..)| matches!(spot, Spot::Emblem(_))), "{hz} Hz: a cell of the field twinkled");
        let first = started.iter().map(|(_, _, at, _)| *at).fold(f64::MAX, f64::min);
        assert!(first >= 1.25, "{hz} Hz: a twinkle at {first} s, before the field had settled");
        for (spot, _, _, life) in &started {
            assert!((TWINKLE_LIFE.0 - 1e-9..=TWINKLE_LIFE.1 + 1e-9).contains(life), "{hz} Hz: {spot:?} twinkles {life} s");
        }
    }
}

#[test]
fn m2_half_of_the_ink_cells_twinkle_in_cyan_and_each_twinkle_flickers() {
    let mut field = emblem_field();
    let (started, glyphs) = run_away(&mut field, 60.0, 40.0, Level::Full, true);
    let emblem = block();
    let cyan = |spot: Spot| {
        let index = spot_index(spot) as usize;
        emblem.cell(index % 76, index / 76).1
    };
    let (on_cyan, on_ink): (Vec<&Twinkle>, Vec<&Twinkle>) = started.iter().partition(|(spot, ..)| cyan(*spot));
    assert!(on_ink.len() >= 200 && !on_cyan.is_empty(), "{} twinkles on ink, {} on cyan in 40 s", on_ink.len(), on_cyan.len());
    assert!(on_cyan.iter().all(|(_, tint, ..)| !tint), "a cyan cell twinkled in ink");
    let other = on_ink.iter().filter(|(_, tint, ..)| *tint).count();
    assert!(other * 100 >= on_ink.len() * 35 && other * 100 <= on_ink.len() * 65, "{other} of {} ink twinkles in cyan", on_ink.len());
    let flickered = glyphs.iter().filter(|glyphs| glyphs.len() > 1).count();
    assert!(flickered * 100 >= glyphs.len() * 85, "{flickered} of {} twinkles took a second glyph", glyphs.len());
}

#[test]
fn m2_the_emblem_twinkles_only_while_the_field_is_live() {
    for (level, on_home, live) in [(Level::Off, true, false), (Level::Reduced, false, false), (Level::Reduced, true, true), (Level::Full, true, true)]
    {
        let mut field = emblem_field();
        let (started, _) = run_away(&mut field, 60.0, 4.0, level, on_home);
        assert_eq!(!started.is_empty(), live, "{level:?}, on Home {on_home}: {} twinkles", started.len());
    }
}

/// How many frames a 60 Hz window that draws only when asked draws in `seconds` from input time 1, `field` shown on
/// Home at `level` with the pointer at `pointer`.
pub(crate) fn frames_drawn(field: &mut Field, seconds: f64, pointer: Option<Pos2>, level: Level) -> usize {
    let (mut drawn, mut next, mut pass): (usize, Option<f64>, u64) = (0, Some(1.0), 0);
    for tick in 0..(seconds * 60.0).round() as u64 {
        let now = 1.0 + tick as f64 * FRAME;
        if next.is_some_and(|next| next <= now + 1e-9) {
            pass += 1;
            field.update(now, pass, pointer, level, true);
            next = field.next_change(now);
            drawn += 1;
        }
    }
    drawn
}

#[test]
fn the_field_asks_for_a_frame_only_when_something_is_due() {
    // No emblem and the pointer away: the first frame, then none.
    let mut bare = Field::new(7);
    bare.build(RECT, TextScale::default(), None, 1.0);
    assert_eq!(frames_drawn(&mut bare, 10.0, None, Level::Full), 1);
    // At off, nothing moves.
    assert_eq!(frames_drawn(&mut emblem_field(), 10.0, None, Level::Off), 1);
    // The emblem twinkling, the pointer away: the frames its twinkles need, well below the display's 60.
    for level in [Level::Full, Level::Reduced] {
        let drawn = frames_drawn(&mut emblem_field(), 10.0, None, level);
        eprintln!("{level:?}: {drawn} frames in 10 s with the pointer away");
        assert!((200..=400).contains(&drawn), "{level:?}: {drawn} frames in 10 s with the pointer away");
    }
    // The pointer still on the field: about as many frames as the display gives, since some cell is always due.
    let drawn = frames_drawn(&mut emblem_field(), 10.0, Some(pos2(200.0, 300.0)), Level::Full);
    eprintln!("still pointer: {drawn} frames in 10 s");
    assert!(drawn >= 300, "{drawn} frames in 10 s under a still pointer");
}

#[test]
fn m2_the_emblem_does_not_twinkle_in_its_first_quarter_second_so_a_picture_rests() {
    // A quarter second, by its number: over 40 seeds a twinkle in it would be all but certain; none starts in it, and some
    // start soon after it.
    let mut after = 0;
    for seed in 0..40 {
        let mut field = Field::new(seed).with_emblem(block());
        let area = Rect::from_min_max(pos2(380.0, 40.0), pos2(960.0, 580.0));
        field.build(RECT, TextScale::default(), Some(area), 1.0);
        let (started, _) = run_away(&mut field, 60.0, 0.5, Level::Full, true);
        for (spot, _, at, _) in &started {
            assert!(*at >= 1.25, "seed {seed}: {spot:?} twinkled at {at} s, before the field had settled");
        }
        after += started.len();
    }
    assert!(after >= 40, "{after} twinkles in the quarter second after the settling, over 40 seeds");
}

/// The share of the field's cells within `reach` of the pointer that are flipped, on average over the frames of the
/// second second under a pointer standing still at `level`, at 60 Hz, over a few places on the field.
fn warm_share(level: Level, reach: f32) -> f64 {
    let (mut warm, mut within) = (0_usize, 0_usize);
    for at in [pos2(300.0, 300.0), pos2(212.0, 157.0), pos2(640.0, 420.0), pos2(500.0, 250.0)] {
        let mut field = Field::new(7);
        field.build(RECT, TextScale::default(), None, 1.0);
        let cells = field.cells.clone().unwrap();
        let near: BTreeSet<u32> = (0..cells.columns * cells.rows)
            .filter(|index| {
                let (column, row) = (index % cells.columns, index / cells.columns);
                let centre = pos2(cells.origin.x + (column as f32 + 0.5) * CELL_WIDTH, row_top(&cells, row) + cells.height / 2.0);
                centre.distance(at) <= reach
            })
            .map(|index| index as u32)
            .collect();
        for step in 0..120_u64 {
            field.update(1.0 + step as f64 * FRAME, step + 1, Some(at), level, true);
            if step >= 60 {
                warm += field.warm.keys().filter(|spot| matches!(spot, Spot::Field(index) if near.contains(index))).count();
                within += near.len();
            }
        }
    }
    warm as f64 / within as f64
}

#[test]
fn m1_under_a_still_pointer_about_as_many_cells_are_flipped_as_in_the_source() {
    // The source's chance (0.8 × f², at reduced × 0.4) and times give about 22 in 100 at full and 10 in 100 at reduced
    // flipped at a time: a simulation of its `update` at 60 Hz.
    let shares = [(Level::Full, REACH_FULL, 0.22), (Level::Reduced, REACH_REDUCED, 0.10)].map(|(level, reach, source)| {
        let share = warm_share(level, reach);
        eprintln!("{level:?}: {share:.3} of the cells within reach flipped under a still pointer (source {source})");
        (level, share, source)
    });
    for (level, share, source) in shares {
        assert!((source * 0.75..=source * 1.25).contains(&share), "{level:?}: {share:.3} flipped, the source {source}");
    }
}

#[test]
fn a_change_planned_within_its_longest_wait_is_not_taken_for_a_clock_that_went_back() {
    for longest in [REFLIP.1, FLICKER.1, RETRY, TWINKLE_NEXT] {
        assert!(longest <= LONGEST_NEXT, "{longest} s is longer than {LONGEST_NEXT} s");
        assert!(!due(1.0, 1.0 + longest - 1e-6, LONGEST_NEXT));
    }
}

#[test]
fn an_input_time_past_any_window_is_ignored() {
    for now in [1.0e10, 7.6e13, 1.0e300, f64::MAX, -1.0e10, f64::MIN] {
        let mut field = emblem_field();
        field.update(now, 1, Some(pos2(300.0, 300.0)), Level::Full, true);
        field.update(now + 1.0, 2, Some(pos2(300.0, 300.0)), Level::Full, true);
        assert!(field.warm.is_empty() && field.next_change(now).is_none(), "{now}: the field moves");
    }
}

#[test]
fn a_cell_waits_to_roll_again_only_while_it_is_within_reach() {
    let (mut field, _) = flipped_at_1();
    assert!(!field.waiting.is_empty());
    // The pointer off the field but in the window: nothing waits, and nothing asks for a frame for it.
    field.update(1.0 + FRAME, 11, Some(pos2(5000.0, 300.0)), Level::Full, false);
    assert!(field.waiting.is_empty(), "{} cells out of reach wait", field.waiting.len());
}
