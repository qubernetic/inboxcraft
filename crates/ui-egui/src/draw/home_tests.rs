// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Home, the page: its five frames at 100 % and 130 % in dark and light, narrower bodies and hostile rects, a hostile
//! model, what each press asks, the keyboard's order, rest, and the emblem that leaves whole.

use egui::accesskit::Role;
use egui::{Key, Rect, Vec2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Asked, BRIEF, Drawn, MOST_SIDE, Stage, emblem_place, show};
use crate::component::tests::{keyboard_walk, rests};
use crate::field::{Emblem, Field, Level};
use crate::home::{AgendaLine, Counted, DraftLine, Failing, Home, sample};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme, install_fonts};

/// The body of a 1440 × 872 window.
const BODY: Vec2 = vec2(1205.0, 826.0);

/// What the harness keeps: Home's model, its field, the last frame's parts and the last ask.
struct State {
    home: Home,
    theme: Theme,
    keymap: Keymap,
    field: Field,
    emblem: Option<Emblem>,
    level: Level,
    rect: Rect,
    drawn: Drawn,
    asked: Option<Asked>,
}

fn scale(percent: u16) -> TextScale {
    TextScale::from_percent(percent).unwrap()
}

/// Home drawn from `home` over `rect` in a window of `size`.
#[allow(clippy::too_many_arguments)]
fn harness_with(
    home: Home,
    mode: Mode,
    percent: u16,
    size: Vec2,
    rect: Rect,
    emblem: Option<Emblem>,
    level: Level,
    glyphless: bool,
) -> Harness<'static, State> {
    harness_at(1.0, home, mode, percent, size, rect, emblem, level, glyphless)
}

/// Home drawn from `home` over `rect` in a window of `size`, at `ppp` pixels per point.
#[allow(clippy::too_many_arguments)]
fn harness_at(
    ppp: f32,
    home: Home,
    mode: Mode,
    percent: u16,
    size: Vec2,
    rect: Rect,
    emblem: Option<Emblem>,
    level: Level,
    glyphless: bool,
) -> Harness<'static, State> {
    let field = emblem.clone().map_or_else(|| Field::new(0), |emblem| Field::new(0).with_emblem(emblem));
    let state = State {
        home,
        theme: Theme::new(mode, scale(percent)),
        keymap: crate::component::gallery::keymap(),
        field,
        emblem,
        level,
        rect,
        drawn: Drawn::default(),
        asked: None,
    };
    let builder = Harness::builder().with_size(size).with_pixels_per_point(ppp).with_max_steps(60).with_context_setup(|ctx| {
        install_fonts(ctx);
        crate::motion::set_reduced(ctx, true);
    });
    let builder = if glyphless { builder.allow_missing_glyphs() } else { builder };
    let mut harness = builder.build_ui_state(
        |ui, state: &mut State| {
            let stage = Stage { field: &mut state.field, emblem: state.emblem.as_ref(), level: state.level };
            let drawn = show(ui, &state.theme, &state.keymap, &state.home, state.rect, stage);
            if drawn.asked.is_some() {
                state.asked = drawn.asked;
            }
            state.drawn = drawn;
        },
        state,
    );
    harness.run_ok();
    harness
}

/// Home in `state` over the body of a 1440 × 872 window.
fn harness(state: &str, mode: Mode, percent: u16) -> Harness<'static, State> {
    harness_with(sample(state).unwrap(), mode, percent, BODY, Rect::from_min_size(pos2(0.0, 0.0), BODY), None, Level::Off, false)
}

/// `(x, y, width, height)` of `rect`, whole points.
fn at(rect: Option<Rect>) -> (f32, f32, f32, f32) {
    let rect = rect.unwrap();
    (rect.left().round(), rect.top().round(), rect.width().round(), rect.height().round())
}

/// The top and the height of `rect`.
fn tall(rect: Option<Rect>) -> (f32, f32) {
    let (_, y, _, h) = at(rect);
    (y, h)
}

#[test]
fn each_frame_stands_where_its_sheet_puts_it_in_dark_and_light() {
    for mode in [Mode::Dark, Mode::Light] {
        let today = harness("today", mode, 100);
        let p = today.state().drawn.parts;
        assert_eq!(at(p.brief), (0.0, 0.0, 600.0, 826.0), "{mode:?}");
        assert_eq!(tall(p.label), (36.0, 16.0), "{mode:?}");
        assert_eq!(at(p.headline), (56.0, 72.0, 544.0, 96.0), "{mode:?}");
        assert_eq!(at(p.lede), (56.0, 188.0, 544.0, 52.0), "{mode:?}");
        assert_eq!(at(p.counts), (56.0, 260.0, 544.0, 78.0), "{mode:?}");
        assert_eq!(at(p.agenda), (56.0, 358.0, 544.0, 149.0), "{mode:?}");
        assert_eq!(at(p.drafts), (56.0, 527.0, 544.0, 157.0), "{mode:?}");
        assert_eq!(tall(p.buttons), (704.0, 32.0), "{mode:?}");
        assert_eq!(at(p.stage), (600.0, 0.0, 605.0, 826.0), "{mode:?}");
        assert_eq!(p.emblem, None, "{mode:?}: no emblem is drawn");

        let p = harness("nothing-to-do", mode, 100).state().drawn.parts;
        assert_eq!(at(p.counts), (56.0, 260.0, 544.0, 94.0), "{mode:?}: the zero tiles' labels take two lines");
        assert_eq!(tall(p.agenda), (374.0, 111.0), "{mode:?}");
        assert_eq!(tall(p.drafts), (505.0, 79.0), "{mode:?}: home-drafts/empty");
        assert_eq!(tall(p.buttons), (604.0, 32.0), "{mode:?}");

        let p = harness("overflow-counts", mode, 100).state().drawn.parts;
        assert_eq!(tall(p.counts), (260.0, 78.0), "{mode:?}");
        assert_eq!(tall(p.agenda), (358.0, 189.0), "{mode:?}: three lines and the more line");
        assert_eq!(tall(p.drafts), (567.0, 157.0), "{mode:?}");
        assert_eq!(tall(p.buttons), (744.0, 32.0), "{mode:?}");

        let p = harness("first-run", mode, 100).state().drawn.parts;
        assert_eq!(at(p.lede), (56.0, 188.0, 544.0, 78.0), "{mode:?}: three lines");
        assert_eq!(tall(p.buttons), (286.0, 32.0), "{mode:?}");
        assert_eq!(at(p.steps), (56.0, 338.0, 544.0, 159.0), "{mode:?}");
        assert!(p.counts.is_none() && p.agenda.is_none() && p.drafts.is_none(), "{mode:?}");

        let p = harness("everything-failing", mode, 100).state().drawn.parts;
        assert!(p.lede.is_none(), "{mode:?}: the banner stands in the lede's place");
        let (top, high) = tall(p.banner);
        assert_eq!(top, 188.0, "{mode:?}");
        assert_eq!(tall(p.counts).0, top + high + 20.0, "{mode:?}");
    }
}

/// The failing state as `home/everything-failing` draws it: the banner's words on two lines, 72 high, the tiles at 280
/// and the buttons at 724; at 130 % the lines grow with the text and stay two.
#[test]
fn the_failing_state_stands_with_its_two_line_banner_at_100_and_130_percent() {
    for mode in [Mode::Dark, Mode::Light] {
        for (percent, banner, counts, buttons) in [(100, (188.0, 72.0), 280.0, 724.0), (130, (222.0, 86.0), 328.0, 886.0)] {
            let p = harness("everything-failing", mode, percent).state().drawn.parts;
            let at = format!("{mode:?} {percent}: {:?} {:?} {:?}", p.banner, p.counts, p.buttons);
            assert_eq!(tall(p.banner), banner, "{at}");
            assert_eq!(tall(p.counts).0, counts, "{at}");
            assert_eq!(tall(p.buttons).0, buttons, "{at}");
        }
    }
}

/// A banner of any length takes at most two lines: 10 000 characters stand as high as the frame's sentence.
#[test]
fn a_failing_banner_of_any_length_takes_two_lines_at_most() {
    let mut home = sample("everything-failing").unwrap();
    let frame = harness("everything-failing", Mode::Dark, 100).state().drawn.parts.banner;
    home.failing = Some(crate::home::Failing { text: "Offline since 14:10 ".repeat(500), account: true, outbox: true });
    let long = harness_with(home, Mode::Dark, 100, BODY, Rect::from_min_size(pos2(0.0, 0.0), BODY), None, Level::Off, false);
    assert_eq!(tall(long.state().drawn.parts.banner), tall(frame));
    let mut home = sample("everything-failing").unwrap();
    home.failing = Some(crate::home::Failing { text: "Offline since 14:10.".into(), account: false, outbox: true });
    let short = harness_with(home, Mode::Dark, 100, BODY, Rect::from_min_size(pos2(0.0, 0.0), BODY), None, Level::Off, false);
    assert_eq!(tall(short.state().drawn.parts.banner), (188.0, 54.0), "words of one line take one");
}

#[test]
fn at_130_percent_heights_follow_the_text_and_widths_and_gaps_stay() {
    for mode in [Mode::Dark, Mode::Light] {
        for state in ["today", "nothing-to-do", "overflow-counts", "first-run", "everything-failing"] {
            let harness = harness(state, mode, 130);
            let theme = &harness.state().theme;
            let p = harness.state().drawn.parts;
            assert_eq!(tall(p.label).0, 36.0, "{state} {mode:?}");
            assert_eq!(at(p.headline).3, (theme.fonts.display.line * 2.0).ceil(), "{state} {mode:?}");
            let order = [p.label, p.headline, p.lede.or(p.banner), p.counts, p.agenda, p.drafts, p.buttons, p.steps];
            let shown: Vec<Rect> = order.into_iter().flatten().collect();
            for pair in shown.windows(2) {
                assert_eq!((pair[1].top() - pair[0].bottom()).round(), 20.0, "{state} {mode:?}: the parts stay 20 apart");
            }
            for rect in &shown {
                assert_eq!(rect.left().round(), 56.0, "{state} {mode:?}");
                assert!(rect.right() <= BRIEF + 0.5, "{state} {mode:?}: {rect:?} leaves the brief");
            }
        }
        let p = harness("today", mode, 130).state().drawn.parts;
        assert_eq!(at(p.stage).2, 605.0, "{mode:?}: the emblem's place keeps its width");
        assert!(at(p.counts).3 > 78.0, "{mode:?}: the tiles grow with the text");
    }
}

/// Every node of `harness` within `page`, and its parts top down without overlapping.
fn fits(harness: &Harness<'_, State>, page: Rect, what: &str) {
    for node in harness.root().children_recursive() {
        let Some(b) = node.accesskit_node().bounding_box() else { continue };
        let rect = Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32));
        if rect.width() > 0.0 && rect.top() < page.bottom() {
            assert!(
                rect.left() >= page.left() - 0.5 && rect.right() <= page.right() + 0.5,
                "{what}: {:?} {rect:?} leaves the page",
                node.accesskit_node().label()
            );
        }
    }
    let p = harness.state().drawn.parts;
    let shown: Vec<Rect> =
        [p.label, p.headline, p.lede.or(p.banner), p.counts, p.agenda, p.drafts, p.buttons, p.steps].into_iter().flatten().collect();
    for pair in shown.windows(2) {
        assert!(pair[1].top() >= pair[0].bottom(), "{what}: {:?} overlaps {:?}", pair[1], pair[0]);
    }
}

#[test]
fn the_bodies_of_narrower_windows_give_up_the_emblems_place_first() {
    // The body of a window 1300, 1000 (the index narrow) and 800 wide (one pane), and one narrower than the brief.
    for (width, stage) in [(1065.0, Some(465.0)), (941.0, Some(341.0)), (743.0, Some(143.0)), (520.0, None)] {
        for percent in [100, 130] {
            for state in ["today", "first-run", "everything-failing", "overflow-counts"] {
                let page = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, BODY.y));
                let harness = harness_with(sample(state).unwrap(), Mode::Dark, percent, page.size(), page, None, Level::Off, false);
                let what = format!("{state} at {width} and {percent} %");
                let p = harness.state().drawn.parts;
                assert_eq!(at(p.brief).2, width.min(BRIEF), "{what}");
                assert_eq!(p.stage.map(|stage| stage.width().round()), stage, "{what}");
                fits(&harness, page, &what);
            }
        }
    }
}

#[test]
fn a_rect_of_nothing_draws_nothing_and_a_huge_one_is_cut() {
    let window = vec2(400.0, 300.0);
    for rect in [
        Rect::from_min_size(pos2(0.0, 0.0), Vec2::ZERO),
        Rect::from_min_max(pos2(100.0, 100.0), pos2(-50.0, -20.0)),
        Rect::from_min_size(pos2(f32::NAN, 0.0), BODY),
        Rect::from_min_size(pos2(0.0, 0.0), vec2(f32::NAN, f32::INFINITY)),
    ] {
        let harness = harness_with(sample("today").unwrap(), Mode::Dark, 100, window, rect, None, Level::Full, false);
        assert_eq!(harness.state().drawn, Drawn::default(), "{rect:?} draws nothing");
    }
    let huge = Rect::from_min_size(pos2(0.0, 0.0), vec2(1.0e9, 1.0e9));
    let harness = harness_with(sample("today").unwrap(), Mode::Dark, 100, window, huge, None, Level::Full, false);
    let p = harness.state().drawn.parts;
    assert_eq!(at(p.brief).2, BRIEF);
    assert!(at(p.brief).3 <= MOST_SIDE && at(p.stage).2 <= MOST_SIDE, "cut to {MOST_SIDE}: {p:?}");
}

/// An emblem of `columns` × `rows` dense cells: a plain placeholder, not the mark.
fn placeholder(columns: usize, rows: usize) -> Emblem {
    let row = "#".repeat(columns);
    Emblem::from_rows(&vec![row.as_str(); rows])
}

/// `docs/design.md` speaks of the window: at the frame's sizes the emblem leaves whole at a window of 1381 and under,
/// and stands whole from 1382.
#[test]
fn the_emblem_leaves_whole_at_a_1381_window_and_under_and_stays_from_1382() {
    let emblem = placeholder(76, 40);
    let size = emblem.size();
    assert_eq!(size, vec2(547.0, 500.0), "the placeholder is the frame's 547 × 500");
    // The stage is the body less the brief; the body is the window less the index (232), two hairlines and the
    // window's two edges.
    for (window, whole) in [(1381.0, false), (1382.0, true), (1440.0, true)] {
        let body = Rect::from_min_size(pos2(0.0, 0.0), vec2(window - 235.0, BODY.y));
        let harness = harness_with(sample("today").unwrap(), Mode::Dark, 100, body.size(), body, Some(emblem.clone()), Level::Off, false);
        let p = harness.state().drawn.parts;
        match (whole, p.emblem) {
            (true, Some(rect)) => {
                assert_eq!(rect.size(), size, "{window}: never smaller");
                assert!(p.stage.unwrap().contains_rect(rect), "{window}: never cut");
            }
            (false, None) => {}
            (_, emblem) => panic!("{window}: the emblem stands at {emblem:?}"),
        }
    }
    let stage = Rect::from_min_size(pos2(600.0, 0.0), vec2(547.0, 499.0));
    assert_eq!(emblem_place(stage, &emblem, 0.0, 1.0), None, "a stage one point too low leaves no emblem");
}

/// The rects of the pictures (meshes with a texture of their own) painted this frame.
fn pictures(harness: &Harness<'_, State>) -> Vec<Rect> {
    harness
        .output()
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => Some(mesh.calc_bounds()),
            _ => None,
        })
        .collect()
}

/// `home/*` at 1440 × 900, in dark and light: the Qubernetic emblem (76 × 40 cells, 547 × 500) centred across the
/// emblem's place, at 629 of the body as the frames draw it; the names' box (394 × 96, `bg`) 12 out from its left and
/// 16 under it; in it, 12 in, the Qubernetic wordmark (88 × 14), the dot and the InboxCraft wordmark (102.17 × 13)
/// centred on a line of 20, the InboxCraft wordmark at 120 of the box's content. Down, the emblem stands 95 below the
/// place's top, as the frames draw it, and the names at 611.
#[test]
fn the_emblem_and_the_names_stand_where_the_frames_draw_them_in_dark_and_light() {
    for mode in [Mode::Dark, Mode::Light] {
        for state in ["today", "first-run"] {
            let body = Rect::from_min_size(pos2(0.0, 0.0), BODY);
            let harness = harness_with(sample(state).unwrap(), mode, 100, BODY, body, Some(crate::brand::emblem().clone()), Level::Off, false);
            let what = format!("{state}, {mode:?}");
            let p = harness.state().drawn.parts;
            assert_eq!(at(p.emblem), (629.0, 95.0, 547.0, 500.0), "{what}: the emblem");
            assert_eq!(at(p.names), (617.0, 611.0, 394.0, 96.0), "{what}: the names' box");
            let (qubernetic, inboxcraft) = p.wordmarks.unwrap();
            assert_eq!(qubernetic, Rect::from_min_size(pos2(629.0, 626.0), vec2(88.0, 14.0)), "{what}: the Qubernetic wordmark");
            assert_eq!(inboxcraft, Rect::from_min_size(pos2(749.0, 626.5), vec2(102.17, 13.0)), "{what}: the InboxCraft wordmark");
            // Each is painted as a picture from the whole pixel its box starts in.
            let painted = pictures(&harness);
            for (name, wordmark) in [("Qubernetic", qubernetic), ("InboxCraft", inboxcraft)] {
                assert!(
                    painted.iter().any(|rect| wordmark.expand(1.0).contains_rect(*rect) && rect.width() >= wordmark.width().floor()),
                    "{what}: {name} painted: {painted:?}"
                );
            }
        }
    }
}

/// The names come with the emblem: where the emblem and the names do not fit whole together, neither stands, at a
/// window of 1381 across and in a place too low for both (95 + 500 + 16 + 96 = 707 high stands it at 95, 612 pulls it
/// up to the top, 611 leaves none); at 130 % text the names' lines grow and both still stand.
#[test]
fn the_names_leave_with_the_emblem() {
    let emblem = crate::brand::emblem().clone();
    for (width, high, percent, whole) in [
        (1146.0, 826.0, 100, false),
        (1147.0, 826.0, 100, true),
        (1205.0, 611.0, 100, false),
        (1205.0, 612.0, 100, true),
        (1205.0, 700.0, 100, true),
        (1205.0, 707.0, 100, true),
        (1205.0, 826.0, 130, true),
    ] {
        let body = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, high));
        let harness = harness_with(sample("today").unwrap(), Mode::Dark, percent, body.size(), body, Some(emblem.clone()), Level::Off, false);
        let p = harness.state().drawn.parts;
        let what = format!("{width} × {high} at {percent} %");
        assert_eq!(p.emblem.is_some(), whole, "{what}: the emblem");
        assert_eq!(p.names.is_some(), whole, "{what}: the names");
        if let (Some(emblem), Some(names)) = (p.emblem, p.names) {
            let stage = p.stage.unwrap();
            assert!(stage.contains_rect(emblem), "{what}: the emblem never cut");
            assert!(names.bottom() <= stage.bottom() && names.top() == emblem.bottom() + 16.0, "{what}: the names 16 under it, inside");
        }
    }
}

/// The emblem stands 95 below the place's top in a place as high as the frames' and in a taller one (it is not
/// centred down), and a place too short for that pulls it up just enough: the names end at the place's foot.
#[test]
fn the_emblem_stands_95_down_and_a_short_place_pulls_it_up() {
    let emblem = crate::brand::emblem().clone();
    for (high, percent, top) in
        [(826.0, 100, 95.0), (1100.0, 100, 95.0), (707.0, 100, 95.0), (700.0, 100, 88.0), (612.0, 100, 0.0), (826.0, 130, 95.0)]
    {
        let body = Rect::from_min_size(pos2(0.0, 0.0), vec2(BODY.x, high));
        let harness = harness_with(sample("today").unwrap(), Mode::Dark, percent, body.size(), body, Some(emblem.clone()), Level::Off, false);
        let p = harness.state().drawn.parts;
        let what = format!("{high} high at {percent} %");
        assert_eq!(at(p.emblem), (629.0, top, 547.0, 500.0), "{what}: the emblem");
        assert_eq!(p.names.unwrap().top(), top + 516.0, "{what}: the names 16 under it");
        assert_eq!(harness.state().field.emblem_at(), p.emblem.map(|rect| rect.min), "{what}: the field draws it there");
        if high < 707.0 {
            assert_eq!(p.names.unwrap().bottom(), high, "{what}: pulled up just enough");
        }
    }
}

/// At 125 % and 150 % the emblem stands on the whole pixel the field draws it on, and the names 16 points (20 and 24
/// pixels) under it, on the same pixel grid, from a place whose corner is a whole point or a part of one.
#[test]
fn at_125_and_150_percent_the_names_sit_on_the_emblem_s_pixel_grid() {
    let emblem = crate::brand::emblem().clone();
    for ppp in [1.25_f32, 1.5] {
        for corner in [pos2(0.0, 0.0), pos2(0.3, 0.7)] {
            let body = Rect::from_min_size(corner, BODY);
            let harness =
                harness_at(ppp, sample("today").unwrap(), Mode::Dark, 100, BODY + vec2(1.0, 1.0), body, Some(emblem.clone()), Level::Off, false);
            let p = harness.state().drawn.parts;
            let what = format!("{ppp} from {corner:?}");
            let (Some(placed), Some(names)) = (p.emblem, p.names) else { panic!("{what}: none: {p:?}") };
            assert_eq!(harness.state().field.emblem_at(), Some(placed.min), "{what}: where the field draws it");
            let whole = |points: f32| ((points * ppp).round() - points * ppp).abs() < 1e-3;
            assert!(whole(placed.left()) && whole(placed.top()), "{what}: the emblem at {placed:?} on whole pixels");
            assert!(whole(names.left()) && whole(names.top()), "{what}: the names at {names:?} on whole pixels");
            assert!(((names.top() - placed.bottom()) - 16.0).abs() < 1e-3, "{what}: 16 points under it");
        }
    }
}

/// The two wordmarks are drawn into pictures once and kept: a frame after that draws none again.
#[test]
fn the_wordmarks_are_drawn_once_and_kept() {
    let body = Rect::from_min_size(pos2(0.0, 0.0), BODY);
    let mut harness = harness_with(sample("today").unwrap(), Mode::Dark, 100, BODY, body, Some(crate::brand::emblem().clone()), Level::Off, false);
    assert_eq!(crate::brand::rasterised(&harness.ctx), 2, "the Qubernetic and the InboxCraft wordmarks");
    for _ in 0..3 {
        harness.step();
    }
    assert_eq!(crate::brand::rasterised(&harness.ctx), 2, "kept: no frame draws them again");
}

/// At 125 % and 150 % each wordmark is painted from the whole pixel its box starts in, as wide as its box and the
/// part of a pixel it is shifted by.
#[test]
fn at_125_and_150_percent_the_wordmarks_start_on_whole_pixels() {
    for ppp in [1.25_f32, 1.5] {
        let body = Rect::from_min_size(pos2(0.0, 0.0), BODY);
        let harness = harness_at(ppp, sample("today").unwrap(), Mode::Dark, 100, BODY, body, Some(crate::brand::emblem().clone()), Level::Off, false);
        let (qubernetic, inboxcraft) = harness.state().drawn.parts.wordmarks.unwrap();
        let painted = pictures(&harness);
        for (name, wordmark) in [("Qubernetic", qubernetic), ("InboxCraft", inboxcraft)] {
            let (pixel, shift) = crate::brand::placed(wordmark.min, ppp);
            let wide = (wordmark.width() * ppp + shift.x).ceil();
            assert!(
                painted.iter().any(|rect| (rect.left() * ppp - pixel.x).abs() < 1e-3
                    && (rect.top() * ppp - pixel.y).abs() < 1e-3
                    && (rect.width() * ppp - wide).abs() < 1e-3),
                "{name} at {ppp}: from pixel {pixel:?}, {wide} wide, in {painted:?}"
            );
        }
    }
}

/// Home puts the stage's emblem on its field, and takes it off when the stage has none.
#[test]
fn home_puts_the_stage_s_emblem_on_its_field() {
    let body = Rect::from_min_size(pos2(0.0, 0.0), BODY);
    let mut harness = harness_with(sample("today").unwrap(), Mode::Dark, 100, BODY, body, None, Level::Off, false);
    assert_eq!(harness.state().field.emblem(), None);
    harness.state_mut().emblem = Some(crate::brand::emblem().clone());
    harness.run_ok();
    assert_eq!(harness.state().field.emblem(), Some(crate::brand::emblem()), "the emblem on the field");
    assert!(harness.state().drawn.parts.emblem.is_some());
    harness.state_mut().emblem = None;
    harness.run_ok();
    assert_eq!(harness.state().field.emblem(), None, "taken off");
    assert_eq!(harness.state().drawn.parts.names, None, "no names without the emblem");
}

/// A text from outside at its worst: 10 000 characters with CR, LF, NUL, bidi controls, Hebrew and Arabic.
/// Home's rect moving by parts of a point (as a sliding place does) keeps the emblem on the same cells of its field:
/// the cells are built once.
#[test]
fn home_moving_by_parts_of_a_point_builds_its_fields_cells_once() {
    let body = Rect::from_min_size(pos2(0.0, 0.0), BODY);
    let mut harness =
        harness_with(sample("today").unwrap(), Mode::Dark, 100, BODY + vec2(4.0, 4.0), body, Some(placeholder(76, 40)), Level::Off, false);
    assert!(harness.state().drawn.parts.emblem.is_some(), "no emblem at 1440");
    for step in 1..=40_u8 {
        let left = f32::from(step) * 0.07;
        harness.state_mut().rect = body.translate(vec2(left, left / 2.0));
        harness.run_ok();
    }
    assert_eq!(harness.state().field.builds(), 1, "a move alone built the field's cells again");
}

fn hostile(word: &str) -> String {
    let piece = format!("{word}\r\n\0\u{202e}\u{2067}שלום مرحبا\u{2069}\u{202c} ");
    piece.repeat(10_000 / piece.chars().count() + 1).chars().take(10_000).collect()
}

/// Home with hostile words everywhere, counts at their edges and 10 000 agenda lines and drafts.
fn hostile_home(counts: [u64; 4]) -> Home {
    let mut home = sample("everything-failing").unwrap();
    home.date = hostile("date");
    home.name = Some(hostile("name"));
    home.lede = hostile("lede");
    home.failing = Some(Failing { text: hostile("failing"), account: true, outbox: true });
    for (count, value) in home.counts.iter_mut().zip(counts) {
        count.count = Some(value);
    }
    home.agenda = (0..10_000).map(|n| AgendaLine { time: hostile("time"), title: hostile("title"), detail: hostile("detail"), account: n }).collect();
    home.agenda_total = u64::MAX;
    home.tomorrow = Some((hostile("time"), hostile("tomorrow")));
    home.drafts = (0..10_000).map(|id| DraftLine { id, subject: Some(hostile("subject")), to: hostile("to"), when: hostile("when") }).collect();
    home.drafts_total = 10_000;
    home
}

fn bidi(text: &str) -> bool {
    text.chars().any(|c| c.is_control() || ('\u{202a}'..='\u{202e}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c))
}

#[test]
fn a_hostile_home_stays_in_its_boxes_and_draws_only_what_it_shows() {
    for counts in [[0, 9_999, 10_000, u64::MAX], [u64::MAX, 0, 1, 9_999]] {
        for percent in [100, 130] {
            let page = Rect::from_min_size(pos2(0.0, 0.0), BODY);
            let mut harness = harness_with(hostile_home(counts), Mode::Dark, percent, BODY, page, None, Level::Full, true);
            harness.run_steps(2);
            fits(&harness, page, &format!("{counts:?} at {percent} %"));
            assert_eq!(harness.query_all_by_role(Role::ListItem).count(), 2, "two drafts drawn of 10 000");
            let lines = harness
                .root()
                .children_recursive()
                .filter(|node| {
                    let node = node.accesskit_node();
                    node.label().or_else(|| node.value()).is_some_and(|text| text.starts_with("time") && text.contains("title"))
                })
                .count();
            assert_eq!(lines, 3, "three agenda lines drawn of 10 000");
            for node in harness.root().children_recursive() {
                let label = node.accesskit_node().label().unwrap_or_default();
                assert!(label.chars().count() <= 3 * crate::home::MOST + 64, "a name of {} characters", label.chars().count());
                assert!(!bidi(&label), "a control or bidi character in {:?}", label.chars().take(40).collect::<String>());
            }
            assert!(harness.query_all_by_label_contains("9\u{202f}999+").count() >= 1, "{counts:?}: a count caps at 9 999+");
            assert!(harness.query_all_by_label_contains("all drafts (9\u{202f}999+)").count() == 1, "the drafts' link caps its count");
        }
    }
}

/// Presses the node labelled `label` and says what Home was asked.
fn asked_by(harness: &mut Harness<'static, State>, label: &str) -> Option<Asked> {
    harness.state_mut().asked = None;
    harness.get_by_label(label).click();
    harness.run_steps(2);
    harness.state().asked
}

#[test]
fn each_press_asks_what_it_says_and_runs_nothing() {
    let mut today = harness("today", Mode::Dark, 100);
    for (label, asked) in [
        ("7 To read", Asked::Count(Counted::ToRead)),
        ("3 To screen", Asked::Count(Counted::ToScreen)),
        ("2 Back today", Asked::Count(Counted::BackToday)),
        ("1 In the outbox", Asked::Count(Counted::Outbox)),
        ("open the week →", Asked::Week),
        ("Price list for the winter workshops to Sunniva Aalto edited 12:10", Asked::OpenDraft(1)),
        ("all drafts (2) →", Asked::AllDrafts),
        ("Price list for the winter workshops to Sunniva Aalto edited 12:10", Asked::OpenDraft(1)),
        ("Read mail", Asked::ReadMail),
        ("Write", Asked::Write),
    ] {
        assert_eq!(asked_by(&mut today, label), Some(asked), "{label}");
    }
    let mut quiet = harness("nothing-to-do", Mode::Light, 100);
    assert_eq!(asked_by(&mut quiet, "Open the calendar"), Some(Asked::Calendar));
    assert_eq!(asked_by(&mut quiet, "Write"), Some(Asked::Write));
    let mut more = harness("overflow-counts", Mode::Dark, 100);
    assert_eq!(asked_by(&mut more, "8 more today →"), Some(Asked::Today));
    let mut first = harness("first-run", Mode::Dark, 100);
    assert_eq!(asked_by(&mut first, "Add an account"), Some(Asked::AddAccount));
    // Coming soon: the window has no way to the sample mail yet (#303's fix; it asked for what nothing handled).
    assert_eq!(asked_by(&mut first, "Look around with sample mail"), None);
    assert!(first.get_by_label("Look around with sample mail").accesskit_node().is_disabled());
    let mut failing = harness("everything-failing", Mode::Dark, 100);
    assert_eq!(asked_by(&mut failing, "open account"), Some(Asked::OpenAccount));
    assert_eq!(asked_by(&mut failing, "open outbox"), Some(Asked::OpenOutbox));
    assert_eq!(asked_by(&mut failing, "1 Not sent"), Some(Asked::Count(Counted::NotSent)));
}

#[test]
fn a_hovered_draft_row_asks_to_open_or_discard_it() {
    let mut today = harness("today", Mode::Dark, 100);
    let row = today.get_by_label("Re: Kiln service: two dates to choose from to Halvard Engen edited yesterday").rect();
    today.hover_at(row.center());
    today.run_steps(2);
    assert_eq!(asked_by(&mut today, "discard"), Some(Asked::DiscardDraft(2)));
    today.hover_at(row.center());
    today.run_steps(2);
    assert_eq!(asked_by(&mut today, "open"), Some(Asked::OpenDraft(2)));
}

/// The label of the focused node.
fn focused(harness: &Harness<'_, State>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
}

#[test]
fn tab_goes_through_the_page_in_the_drawn_order_and_enter_and_space_press() {
    const FIRST: &str = "Price list for the winter workshops to Sunniva Aalto edited 12:10";
    const SECOND: &str = "Re: Kiln service: two dates to choose from to Halvard Engen edited yesterday";
    let mut today = harness("today", Mode::Dark, 100);
    let mut walked = Vec::new();
    for _ in 0..14 {
        today.key_press(Key::Tab);
        today.run_steps(2);
        walked.push(focused(&today).unwrap_or_default());
    }
    // A draft row the keyboard is on shows its two links, and Tab goes through them before the next row.
    let order = [
        "7 To read",
        "3 To screen",
        "2 Back today",
        "1 In the outbox",
        "open the week →",
        FIRST,
        "open",
        "discard",
        SECOND,
        "open",
        "discard",
        "all drafts (2) →",
        "Read mail",
        "Write",
    ];
    assert_eq!(walked, order);
    today.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    today.run_steps(2);
    assert_eq!(focused(&today).as_deref(), Some("Read mail"), "Shift Tab goes back");
    today.state_mut().asked = None;
    today.key_press(Key::Enter);
    today.run_steps(2);
    assert_eq!(today.state().asked, Some(Asked::ReadMail), "Enter presses");
    let mut back = Vec::new();
    for _ in 0..7 {
        today.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
        today.run_steps(2);
        back.push(focused(&today).unwrap_or_default());
    }
    assert_eq!(back, ["all drafts (2) →", "discard", "open", SECOND, "discard", "open", FIRST], "Shift Tab: the drawn order backwards");
    today.state_mut().asked = None;
    today.key_press(Key::Space);
    today.run_steps(2);
    assert_eq!(today.state().asked, Some(Asked::OpenDraft(1)), "Space presses");
    for _ in 0..2 {
        today.key_press(Key::Tab);
        today.run_steps(2);
    }
    assert_eq!(focused(&today).as_deref(), Some("discard"));
    today.state_mut().asked = None;
    today.key_press(Key::Enter);
    today.run_steps(2);
    assert_eq!(today.state().asked, Some(Asked::DiscardDraft(1)), "discard by the keyboard only asks");
    // With the keyboard on the first draft its links are in the tree, and Tab reaches them.
    let mut lit = harness("today", Mode::Light, 100);
    lit.get_by_label(FIRST).focus();
    lit.run_steps(2);
    assert!(lit.query_by_label("discard").is_some(), "the keyed row shows its links");
    let missed = keyboard_walk(&mut lit);
    assert!(missed.is_empty(), "Tab never reaches {missed:?}");
    for state in ["today", "nothing-to-do", "overflow-counts", "first-run", "everything-failing"] {
        let mut harness = harness(state, Mode::Light, 100);
        let missed = keyboard_walk(&mut harness);
        assert!(missed.is_empty(), "{state}: Tab never reaches {missed:?}");
    }
}

/// With the pointer away Home rests at off; at full and reduced its emblem twinkles on by itself, and the frames that
/// asks for are bounded: those its twinkles need, about 30 a second, well below the display's 60.
#[test]
fn home_with_the_pointer_away_rests_at_off_and_twinkles_at_a_bounded_pace() {
    for level in [Level::Off, Level::Reduced, Level::Full] {
        for state in ["today", "first-run", "everything-failing"] {
            let page = Rect::from_min_size(pos2(0.0, 0.0), BODY);
            let mut harness = harness_with(sample(state).unwrap(), Mode::Dark, 100, BODY, page, Some(placeholder(20, 10)), level, false);
            let rests = rests(&mut harness, 8);
            assert_eq!(rests, level == Level::Off, "{state} at {level:?}: rests {rests}");
            let mut field = harness.state().field.clone();
            let drawn = crate::field::tests::frames_drawn(&mut field, 10.0, None, level);
            let most = if level == Level::Off { 1 } else { 400 };
            assert!(drawn <= most, "{state} at {level:?}: {drawn} frames in 10 s with the pointer away");
        }
    }
}

/// Every control Home's page draws, in each state, that can be pressed asks for something the window handles: no press
/// is a no-op. What cannot be pressed yet reads as disabled ("Look around with sample mail", coming soon).
#[test]
fn no_pressable_control_on_home_is_a_no_op() {
    for state in ["today", "nothing-to-do", "overflow-counts", "first-run", "everything-failing"] {
        let labels: Vec<String> = {
            let harness = harness(state, Mode::Dark, 100);
            harness
                .root()
                .children_recursive()
                .filter(|node| matches!(node.accesskit_node().role(), Role::Button | Role::Link | Role::ListItem))
                .filter(|node| !node.accesskit_node().is_disabled())
                .filter_map(|node| node.accesskit_node().label())
                .collect()
        };
        assert!(!labels.is_empty(), "{state}");
        for label in labels {
            let mut harness = harness(state, Mode::Dark, 100);
            let asked = asked_by(&mut harness, &label);
            // `Asked::SampleMail` is the one ask the window drops (`InboxApp::apply_home`).
            assert!(asked.is_some_and(|asked| asked != Asked::SampleMail), "{state}: {label:?} asked {asked:?}");
        }
    }
}

/// Home "today" at motion `level`, frames 10 ms apart; `shown` false draws nothing (the page is left).
fn headline_harness(level: crate::motion::Level) -> Harness<'static, (Home, bool)> {
    let theme = Theme::new(Mode::Dark, scale(100));
    let keymap = Keymap::default();
    let mut field = Field::new(0);
    let mut harness = crate::component::tests::builder().with_size(BODY).with_step_dt(0.01).build_ui_state(
        move |ui, (home, shown): &mut (Home, bool)| {
            crate::motion::set_level(ui.ctx(), level);
            if *shown {
                let stage = Stage { field: &mut field, emblem: None, level: Level::Off };
                show(ui, &theme, &keymap, home, Rect::from_min_size(pos2(0.0, 0.0), BODY), stage);
            }
        },
        (sample("today").unwrap(), false),
    );
    // The harness's first frames run before the page opens.
    harness.state_mut().1 = true;
    harness.step();
    harness
}

/// The headline's two lines as painted, the greeting and the name.
fn headline_lines(harness: &Harness<'static, (Home, bool)>) -> (String, String) {
    let (first, second) = harness.state().0.headline();
    let texts = crate::component::tests::texts(harness);
    let width = |text: &str| text.chars().count();
    let first = texts.iter().find(|text| width(text) == width(first) && !text.starts_with('[')).cloned().unwrap_or_default();
    let name = format!("{}.", second.unwrap_or_default());
    let second = texts.iter().find(|text| width(text) == width(&name) && **text != first).cloned().unwrap_or_default();
    (first, second)
}

#[test]
fn at_full_the_headline_resolves_from_the_left_over_700_ms_each_time_the_page_opens() {
    let mut harness = headline_harness(crate::motion::Level::Full);
    let greeting = harness.state().0.headline().0.to_owned();
    let (first, _) = headline_lines(&harness);
    assert!(first.chars().zip(greeting.chars()).all(|(a, b)| a != b || b == ' '), "0 ms: only glyphs, {first:?}");
    harness.run_steps(42);
    let (first, second) = headline_lines(&harness);
    let word = greeting.split(' ').next().unwrap();
    assert!(first.starts_with(word), "420 ms: the first word reads, {first:?}");
    assert_ne!(second, format!("{}.", harness.state().0.headline().1.unwrap()), "the name not yet");
    harness.run_steps(28);
    assert_eq!(headline_lines(&harness).0, greeting, "700 ms: it reads");
    assert!(rests(&mut harness, 2), "and rests");
    // Left and opened again: it plays again.
    harness.state_mut().1 = false;
    harness.step();
    harness.state_mut().1 = true;
    harness.step();
    assert_ne!(headline_lines(&harness).0, greeting, "opened again, it resolves again");
}

#[test]
fn at_reduced_the_headline_plays_once_per_launch_and_at_off_never() {
    let mut harness = headline_harness(crate::motion::Level::Reduced);
    let greeting = harness.state().0.headline().0.to_owned();
    assert_ne!(headline_lines(&harness).0, greeting, "the first time: glyphs");
    harness.run_steps(70);
    assert_eq!(headline_lines(&harness).0, greeting);
    harness.state_mut().1 = false;
    harness.step();
    harness.state_mut().1 = true;
    harness.step();
    assert_eq!(headline_lines(&harness).0, greeting, "once per launch: not again");
    assert!(rests(&mut harness, 2));
    let mut harness = headline_harness(crate::motion::Level::Off);
    assert_eq!(headline_lines(&harness).0, greeting, "off: it reads at once");
    assert!(rests(&mut harness, 2));
}

#[test]
fn a_key_during_the_headline_ends_it_in_that_frame() {
    let mut harness = headline_harness(crate::motion::Level::Full);
    let greeting = harness.state().0.headline().0.to_owned();
    harness.run_steps(5);
    harness.key_press(Key::Tab);
    harness.step();
    assert_eq!(headline_lines(&harness).0, greeting, "the key acts and the headline reads at once");
}

/// The drafts' block of the sample `state` in its frame of its own (`home-drafts/*`), in `mode`: the block's rect and
/// the draft rows the harness holds.
fn drafts_frame(state: &str, mode: Mode) -> (Rect, usize) {
    let theme = Theme::new(mode, scale(100));
    let keymap = crate::component::gallery::keymap();
    let home = sample(state).unwrap();
    let mut harness = Harness::builder().with_size(vec2(600.0, 300.0)).with_context_setup(install_fonts).build_ui_state(
        |ui, block: &mut Rect| {
            *block = super::drafts_frame(ui, &theme, &keymap, &home, Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 300.0)));
        },
        Rect::NOTHING,
    );
    harness.run_ok();
    let rows = ["Price list for the winter workshops", "Re: Kiln service", "to Marit Solheim"]
        .into_iter()
        .filter(|words| harness.query_by(|node| node.label().is_some_and(|label| label.contains(words))).is_some())
        .count();
    (*harness.state(), rows)
}

/// `home-drafts/default` draws three drafts of five, the block 494 × 201 25 in its frame; `home-drafts/empty` the empty
/// line, 79 high; the page's frames draw two, and the page gives the block no more (`today`, `overflow-counts` pinned
/// above at 157).
#[test]
fn the_drafts_block_draws_three_where_its_frame_draws_three() {
    for mode in [Mode::Dark, Mode::Light] {
        let (block, rows) = drafts_frame("drafts", mode);
        assert_eq!(at(Some(block)), (25.0, 25.0, 494.0, 201.0), "{mode:?}: home-drafts/default");
        assert_eq!(rows, 3, "{mode:?}: three rows");
        let (block, rows) = drafts_frame("drafts-empty", mode);
        assert_eq!(at(Some(block)), (25.0, 25.0, 494.0, 79.0), "{mode:?}: home-drafts/empty");
        assert_eq!(rows, 0, "{mode:?}");
    }
    let page = sample("drafts").unwrap();
    let harness = harness_with(page, Mode::Dark, 100, BODY, Rect::from_min_size(pos2(0.0, 0.0), BODY), None, Level::Off, false);
    assert_eq!(tall(harness.state().drawn.parts.drafts).1, 157.0, "the page draws two of three");
}

/// Every text the last frame painted inside `within`, joined in the order painted.
fn painted_in(harness: &Harness<'_, State>, within: Rect) -> String {
    fn walk(shape: &egui::Shape, within: Rect, found: &mut Vec<String>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, within, found)),
            egui::Shape::Text(text) if within.expand(1.0).contains_rect(Rect::from_min_size(text.pos, text.galley.size())) => {
                found.push(text.galley.job.text.clone());
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, within, &mut found));
    found.join(" ")
}

/// The longest account name the banner's sentence holds leaves its end in sight: "refused the sign-in" shows on the
/// banner's two lines at a 600 point brief and at 130 %, alone and after "Offline since …".
#[test]
fn a_long_account_name_leaves_refused_the_sign_in_in_sight() {
    let name = crate::home::named(&"W".repeat(200));
    assert_eq!(name.chars().count(), crate::home::NAME_MOST + 1, "cut, with one …");
    for since in [None, Some("14:10")] {
        for percent in [100, 130] {
            for width in [600.0, BRIEF] {
                let mut home = sample("everything-failing").unwrap();
                let text = crate::home::failing_text(since, std::slice::from_ref(&name), 1, 1, Some("Odette Marchand"));
                home.failing = Some(Failing { text, account: true, outbox: true });
                let page = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, BODY.y));
                let harness = harness_with(home, Mode::Dark, percent, page.size(), page, None, Level::Off, false);
                let banner = harness.state().drawn.parts.banner.unwrap();
                let shown = painted_in(&harness, banner);
                assert!(shown.contains("refused the sign-in"), "{since:?} at {width} and {percent} %: {shown}");
            }
        }
    }
}
