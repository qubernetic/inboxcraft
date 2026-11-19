// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The character field and the emblem: the fixed pattern, whole pixels, built once, hostile sizes, M1 and M2 at their
//! key frames at every level, rest, and that the field takes no click, key or focus.

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
}

impl Stage {
    fn new(level: Level, opts: Opts) -> Self {
        Self {
            field: Field::new(RECT, 7, TextScale::default()),
            rect: RECT,
            level,
            opts,
            mode: Mode::Dark,
            scale: TextScale::default(),
            over: 0,
            under: 0,
            keys: 0,
            buttons: false,
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
                    if ui.put(under, egui::Button::new("Under")).clicked() {
                        stage.under += 1;
                    }
                }
                show(ui, &theme, &mut stage.field, stage.rect, stage.level, stage.opts);
                if ui.input(|input| input.key_pressed(Key::K)) {
                    stage.keys += 1;
                }
                // Over the field: drawn after it.
                if stage.buttons {
                    let over = Rect::from_min_size(pos2(100.0, 100.0), vec2(120.0, 32.0));
                    if ui.put(over, egui::Button::new("Over")).clicked() {
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

/// The cells a field over `rect` holds with `seed` at `scale`, one pixel a point.
fn built(rect: Rect, seed: u32, scale: TextScale) -> Cells {
    cells(rect, scale, None, 1.0, seed, None)
}

/// The warm cells: their spot, their glyph and whether they are cooling.
fn warm(harness: &Harness<'static, Stage>) -> Vec<(Spot, char, bool)> {
    harness.state().field.warm.iter().map(|(spot, warm)| (*spot, glyph(warm.glyph), warm.left.is_some())).collect()
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
    stage.field = Field::new(RECT, 7, TextScale::default()).with_emblem(block());
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

#[test]
fn seed_0_is_the_design_files_pattern() {
    // The first row of the character field of `screen/home/today`: dots at columns 0, 14, 28, 57 and 72.
    let cells = built(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 825.0)), 0, TextScale::default());
    let row: Vec<usize> = (0..80).filter(|column| cells.held[*column] == DOT).collect();
    assert_eq!(row, [0, 14, 28, 57, 72]);
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

#[test]
fn a_4k_field_builds_without_a_spike() {
    use inboxcraft_time::{Clock as _, SystemClock};
    let clock = SystemClock::new();
    let started = clock.monotonic();
    let cells = cells(Rect::from_min_size(Pos2::ZERO, vec2(3840.0, 2160.0)), TextScale::default(), None, 1.0, 7, None);
    assert_eq!(cells.held.len(), 480 * 144);
    // Some 69 000 cells, a hash each: far under a frame even unoptimised.
    let took = clock.monotonic().saturating_sub(started);
    assert!(took < std::time::Duration::from_millis(50), "{took:?}");
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
    for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(cells(RECT, TextScale::default(), None, ppp, 7, None).held.is_empty());
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
    // 0 ms: the cells within reach flip at once; none beyond.
    harness.hover_at(at);
    harness.step();
    let first = warm(&harness);
    let spots = field_spots(&first);
    if reach <= 0.0 {
        assert!(first.is_empty(), "{level:?}: the field answered the pointer");
        assert!(!repaints(&harness) || rests(&mut harness, 4), "{level:?}: the still field does not rest");
        return;
    }
    let near = within(&harness, at, reach);
    assert!(spots.len() * 4 >= near && spots.len() * 4 <= near * 3, "{level:?}: {} of {near} cells within {reach} flipped", spots.len());
    for spot in &spots {
        assert!(centre(&harness, *spot).distance(at) <= reach, "{level:?}: {spot:?} flipped beyond {reach}");
    }
    assert!(first.iter().all(|(.., cooling)| !cooling));
    assert!(first.iter().any(|(_, glyph, _)| *glyph != '·'));
    // The pointer stays: the cells hold their glyphs and nothing asks for a frame.
    harness.step();
    assert_eq!(warm(&harness), first, "{level:?}: a held cell changed");
    assert!(rests(&mut harness, 3), "{level:?}: the field under a still pointer does not rest");
    // The pointer moves on: the cells it left cool, each at its own time, keeping their glyphs.
    harness.hover_at(away);
    harness.step();
    assert!(repaints(&harness), "{level:?}: no frame asked for while cells cool");
    harness.run_steps(MS_100 - 1);
    assert!(repaints(&harness), "{level:?}: no frame asked for while the trail cools, the pointer still");
    let mid = warm(&harness);
    let left: Vec<_> = mid.iter().filter(|(spot, ..)| spots.contains(spot)).collect();
    assert!(left.iter().all(|(.., cooling)| *cooling));
    for (spot, glyph, _) in &left {
        assert_eq!(first.iter().find(|(was, ..)| was == spot).map(|(_, glyph, _)| glyph), Some(glyph), "{spot:?} changed its glyph");
    }
    assert!(left.len() * 4 >= spots.len() && left.len() * 4 <= spots.len() * 3, "{level:?}: {} of {} still warm at 100 ms", left.len(), spots.len());
    let held = field_spots(&warm(&harness).into_iter().filter(|(.., cooling)| !cooling).collect::<Vec<_>>());
    assert!(!held.is_empty() && held.iter().all(|spot| centre(&harness, *spot).distance(away) <= reach));
    let glyphs: Vec<_> = warm(&harness).into_iter().filter(|(.., cooling)| !cooling).collect();
    // 200 ms after it moved on: the trail has cooled; the cells under the still pointer are the same glyphs.
    harness.run_steps(MS_200 - MS_100 + 1);
    let last = warm(&harness);
    assert!(last.iter().all(|(spot, ..)| !spots.contains(spot)), "{level:?}: the trail is still warm at 200 ms");
    assert_eq!(last, glyphs, "{level:?}: the cells under the still pointer changed");
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
fn the_pointer_leaving_the_window_cools_everything_and_the_field_rests() {
    for level in [Level::Full, Level::Reduced] {
        let mut stage = with_emblem(level);
        stage.opts.on_home = true;
        let mut harness = harness(stage);
        harness.hover_at(pos2(380.0 + 290.0 - 30.0, 300.0));
        harness.step();
        assert!(!warm(&harness).is_empty());
        harness.event(Event::PointerGone);
        harness.step();
        assert!(warm(&harness).iter().all(|(.., cooling)| *cooling));
        assert!(repaints(&harness));
        harness.run_steps(MS_200);
        assert!(warm(&harness).is_empty(), "{level:?}: still warm after the pointer left");
        assert!(rests(&mut harness, 3), "{level:?}: no rest after the pointer left");
    }
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
        assert!(emblem_spots(&harness).iter().all(|(.., cooling)| *cooling));
        harness.run_steps(MS_200);
        assert!(emblem_spots(&harness).is_empty(), "{level:?}: the mark did not settle back in 200 ms");
        assert!(rests(&mut harness, 3));
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
