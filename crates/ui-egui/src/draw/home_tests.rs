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
    let builder = Harness::builder().with_size(size).with_max_steps(60).with_context_setup(|ctx| {
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

#[test]
fn the_emblem_leaves_whole_under_1381() {
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
    assert_eq!(emblem_place(stage, &emblem), None, "a stage one point too low leaves no emblem");
}

/// A text from outside at its worst: 10 000 characters with CR, LF, NUL, bidi controls, Hebrew and Arabic.
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
    assert_eq!(asked_by(&mut first, "Look around with sample mail"), Some(Asked::SampleMail));
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
    let mut today = harness("today", Mode::Dark, 100);
    let mut walked = Vec::new();
    for _ in 0..10 {
        today.key_press(Key::Tab);
        today.run_steps(2);
        walked.push(focused(&today).unwrap_or_default());
    }
    let order = [
        "7 To read",
        "3 To screen",
        "2 Back today",
        "1 In the outbox",
        "open the week →",
        "Price list for the winter workshops to Sunniva Aalto edited 12:10",
        "Re: Kiln service: two dates to choose from to Halvard Engen edited yesterday",
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
    for _ in 0..3 {
        today.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
        today.run_steps(2);
    }
    assert_eq!(focused(&today).as_deref(), Some("Price list for the winter workshops to Sunniva Aalto edited 12:10"));
    today.state_mut().asked = None;
    today.key_press(Key::Space);
    today.run_steps(2);
    assert_eq!(today.state().asked, Some(Asked::OpenDraft(1)), "Space presses");
    for state in ["today", "nothing-to-do", "overflow-counts", "first-run", "everything-failing"] {
        let mut harness = harness(state, Mode::Light, 100);
        let missed = keyboard_walk(&mut harness);
        assert!(missed.is_empty(), "{state}: Tab never reaches {missed:?}");
    }
}

#[test]
fn home_rests_at_every_level_with_the_pointer_away() {
    for level in [Level::Off, Level::Reduced, Level::Full] {
        for state in ["today", "first-run", "everything-failing"] {
            let page = Rect::from_min_size(pos2(0.0, 0.0), BODY);
            let mut harness = harness_with(sample(state).unwrap(), Mode::Dark, 100, BODY, page, Some(placeholder(20, 10)), level, false);
            assert!(rests(&mut harness, 8), "{state} at {level:?} keeps asking for frames");
        }
    }
}
