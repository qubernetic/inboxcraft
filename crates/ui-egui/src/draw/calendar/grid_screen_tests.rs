// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar's grids in whole pixels as the window paints them, at every scale and text scale; its keys; its
//! rest; and hostile events (long, bidi, control characters, no spaces; zero and negative lengths, across midnight,
//! 400 days, 500 at once, far-off dates) drawn without a panic and without text outside its cell.

use std::time::Duration;

use egui::accesskit::Role;
use egui::{Color32, Key, Rect, UiBuilder, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _, by};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::calendar::{Day, EventRow, Moment, View};
use crate::component::tests::{keyboard_walk, rests};
use crate::keymap::{self, Keymap};
use crate::mailbox::Mailbox;
use crate::theme::{Mode, Palette, TextScale, Theme};
use crate::{InboxApp, sample};

const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app() -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()))
}

/// The sample window at `ppp` and text scale `percent` on the calendar in `view`.
fn window(ppp: f32, percent: u16, view: View) -> Harness<'static, InboxApp> {
    window_with(ppp, percent, view, false)
}

/// [`window`], drawing a sign no bundled font holds (an emoji in an event's title) as a box when `tofu`.
fn window_with(ppp: f32, percent: u16, view: View, tofu: bool) -> Harness<'static, InboxApp> {
    let mut app = app();
    app.set_text_scale(TextScale::from_percent(percent).unwrap());
    app.show_calendar();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.set_view(mailbox, view);
    let mut builder = Harness::builder().with_size(vec2(1440.0, 900.0)).with_pixels_per_point(ppp).with_theme(egui::Theme::Dark);
    if tofu {
        builder = builder.allow_missing_glyphs();
    }
    let mut harness = builder.build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// Every filled rect of the last frame and its fill.
fn rects<State>(harness: &Harness<'static, State>) -> Vec<(Rect, Color32)> {
    fn open(shape: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            egui::Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

/// The grid's lines of the last frame in physical pixels: the left (or top) pixel of each `panel_line` line across
/// (`vertical`) or down.
fn lines<State>(harness: &Harness<'static, State>, ppp: f32, vertical: bool) -> Vec<f32> {
    let p = Palette::of(Mode::Dark);
    rects(harness)
        .into_iter()
        .filter(|(rect, fill)| {
            *fill == p.panel_line
                && if vertical { rect.width() * ppp <= 2.5 && rect.height() > 20.0 } else { rect.height() * ppp <= 2.5 && rect.width() > 20.0 }
        })
        .map(|(rect, _)| if vertical { rect.left() * ppp } else { rect.top() * ppp })
        .collect()
}

fn whole(px: f32) -> bool {
    (px - px.round()).abs() < 0.01
}

/// The rect of the first event whose name holds `text`.
fn event_rect<State>(harness: &Harness<'static, State>, text: &str) -> Rect {
    harness.query_all(by().role(Role::Button).label_contains(text)).next().unwrap().rect()
}

/// The cells the last frame painted for events: a ground (`surface` at rest) with an account's bar of 3 points at its
/// top left, as tall as the ground.
fn painted_cells<State>(harness: &Harness<'static, State>, ppp: f32) -> Vec<Rect> {
    let p = Palette::of(Mode::Dark);
    let all = rects(harness);
    let bar = super::grid::pixels(3.0, ppp);
    all.iter()
        .filter(|(rect, fill)| *fill == p.surface && rect.width() > bar)
        .filter(|(rect, _)| {
            all.iter().any(|(other, fill)| {
                *fill != p.surface && other.min == rect.min && (other.width() - bar).abs() < 0.01 && (other.height() - rect.height()).abs() < 0.01
            })
        })
        .map(|(rect, _)| *rect)
        .collect()
}

/// The painted cell of the first event whose name holds `text`: the one its button stands on (within 2 px).
fn painted_rect<State>(harness: &Harness<'static, State>, text: &str, ppp: f32) -> Rect {
    let near = event_rect(harness, text);
    let cells = painted_cells(harness, ppp);
    let found = cells.iter().copied().find(|cell| (cell.min - near.min).length() * ppp <= 2.0 && (cell.max - near.max).length() * ppp <= 2.0);
    found.unwrap_or_else(|| panic!("{ppp}: no painted cell for {text:?} at {near:?} in {cells:?}"))
}

/// The pixels of one point's line at `ppp`.
fn line_px(ppp: f32) -> f32 {
    ppp.round().max(1.0)
}

#[test]
fn an_event_lies_on_the_grid_s_lines() {
    for ppp in SCALES {
        for percent in [100, 130] {
            let harness = window(ppp, percent, View::Week);
            // Monday 2 November, 09:00 to 10:00: its painted cell on the hour lines and the day lines.
            let rect = painted_rect(&harness, "Studio stand-up, Monday 2 November", ppp);
            let w = line_px(ppp);
            let (left, right, top, bottom) = (rect.left() * ppp, rect.right() * ppp, rect.top() * ppp, rect.bottom() * ppp);
            for edge in [left, right, top, bottom] {
                assert!(whole(edge), "{ppp} {percent}: an edge between pixels: {rect:?}");
            }
            let across = lines(&harness, ppp, true);
            let down = lines(&harness, ppp, false);
            let near = |found: &[f32], at: f32| found.iter().any(|x| (x - at).abs() < 0.01);
            assert!(near(&across, left), "{ppp} {percent}: its left is a day line's: {left} in {across:?}");
            assert!(near(&across, right - w), "{ppp} {percent}: its right is the next day line's");
            assert!(near(&down, top), "{ppp} {percent}: its top is the hour line's: {top}");
            assert!(near(&down, bottom - w), "{ppp} {percent}: its bottom is the next hour line's");
            let second = |found: &[f32], at: f32| found.iter().any(|x| (x - at).abs() > 0.01 && (x - at).abs() <= 2.0);
            assert!(!second(&across, left) && !second(&across, right - w), "{ppp} {percent}: no second day line within 2 px");
            assert!(!second(&down, top) && !second(&down, bottom - w), "{ppp} {percent}: no second hour line within 2 px");
        }
    }
}

/// The hours of the week alone in a view `width` points wide at `ppp`.
fn hours(width: f32, ppp: f32) -> Harness<'static, InboxApp> {
    let mut app = app();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.refresh(mailbox);
    calendar.set_view(mailbox, View::Week);
    let mut harness = Harness::builder()
        .with_size(vec2(width + 40.0, 700.0))
        .with_pixels_per_point(ppp)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(
            move |ui, app: &mut InboxApp| {
                let theme = Theme::new(Mode::Dark, TextScale::default());
                let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 680.0));
                ui.scope_builder(UiBuilder::new().max_rect(view), |ui| {
                    let mut asked = Vec::new();
                    super::hours::show(ui, &theme, &Keymap::default(), app.calendar(), 7, &mut asked);
                });
            },
            app,
        );
    harness.run();
    harness
}

#[test]
fn seven_day_columns_stand_on_whole_pixels() {
    for ppp in SCALES {
        for width in [955.0_f32, 956.0, 957.0] {
            let harness = hours(width, ppp);
            // The days' heads: eight lines from the gutter's to the last day's.
            let heads: Vec<f32> = {
                let p = Palette::of(Mode::Dark);
                let mut found: Vec<f32> = rects(&harness)
                    .into_iter()
                    .filter(|(rect, fill)| *fill == p.panel_line && rect.top() < 1.0 && rect.width() * ppp <= 2.5)
                    .map(|(rect, _)| rect.left() * ppp)
                    .collect();
                found.sort_by(f32::total_cmp);
                found.dedup_by(|a, b| (*a - *b).abs() < 0.01);
                found
            };
            assert_eq!(heads.len(), 8, "{ppp} {width}: {heads:?}");
            assert!(heads.iter().all(|x| whole(*x)), "{ppp} {width}: {heads:?}");
            let columns: Vec<f32> = heads.windows(2).map(|pair| pair[1] - pair[0]).collect();
            let (least, most) = columns.iter().fold((f32::MAX, f32::MIN), |(a, b), c| (a.min(*c), b.max(*c)));
            assert!(most - least <= 1.0 + 0.01, "{ppp} {width}: the columns differ by a pixel at most: {columns:?}");
            assert!((heads[0] - (56.0 * ppp).round()).abs() < 0.01, "{ppp} {width}: the gutter's line");
            assert!((heads[7] - ((width - 12.0) * ppp).round()).abs() < 0.01, "{ppp} {width}: the last day's line");
        }
    }
}

#[test]
fn overlapping_events_share_one_line() {
    for ppp in SCALES {
        let harness = window(ppp, 100, View::Week);
        // Thursday 5 November: the call (09:30 – 10:30) and the dentist (10:00 – 11:30) side by side.
        let call = painted_rect(&harness, "Call with Petra about the logo", ppp);
        let dentist = painted_rect(&harness, "Dentist", ppp);
        let w = line_px(ppp) / ppp;
        assert!((call.right() - w - dentist.left()).abs() < 0.01, "{ppp}: the line between them is one: {call:?} {dentist:?}");
        assert!(call.left() < dentist.left());
        let across = lines(&harness, ppp, true);
        assert!(across.iter().any(|x| (x - call.left() * ppp).abs() < 0.01), "{ppp}: the first stands on the day's line");
        assert!(across.iter().any(|x| (x - (dentist.right() - w) * ppp).abs() < 0.01), "{ppp}: the second ends on the next day's line");
        // Two bands one above the other share one line too: the lower one's top is the upper one's bottom line.
        let band_line = (super::grid::lane_pitch(24.0, ppp) * ppp).round();
        assert_eq!(band_line, (24.0 * ppp).round() - line_px(ppp), "{ppp}: a lane is a band less its line");
        let mut harness = window(ppp, 100, View::Week);
        let mut events: Vec<EventRow> = harness.state().calendar().events().cloned().collect();
        let fair = events.iter().find(|event| event.summary == "Small press book fair").cloned().unwrap();
        events.push(EventRow { id: 9200, summary: "Second fair".to_owned(), ..fair });
        harness.state_mut().calendar_mut().0.set_events(events);
        harness.run();
        let upper = painted_rect(&harness, "Small press book fair", ppp);
        let lower = painted_rect(&harness, "Second fair", ppp);
        assert!(whole(upper.top() * ppp) && whole(lower.top() * ppp) && whole(lower.bottom() * ppp), "{ppp}: bands on whole pixels");
        assert!((upper.height() * ppp - (24.0 * ppp).round()).abs() < 0.01, "{ppp}: a band is 24 points in whole pixels: {upper:?}");
        assert!(((lower.top() - (upper.bottom() - w)) * ppp).abs() < 0.01, "{ppp}: one line between the bands: {upper:?} {lower:?}");
        assert_eq!((upper.left(), upper.right()), (lower.left(), lower.right()));
    }
}

#[test]
fn arrows_move_the_picked_day() {
    let mut harness = window(1.0, 100, View::Month);
    let day = |date: u8| Day::new(2026, 11, date).unwrap();
    let presses = [(Key::ArrowRight, 6), (Key::ArrowDown, 13), (Key::ArrowLeft, 12), (Key::ArrowUp, 5)];
    for (key, date) in presses {
        harness.key_press(key);
        harness.run();
        assert_eq!(harness.state().calendar().picked_day(), Some(day(date)), "{key:?}");
    }
    // The keyboard is on the picked day: a screen reader hears it.
    let focused = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label());
    assert_eq!(focused.as_deref(), Some("Thursday 5 November 2026, today"));
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().calendar().view(), View::Day, "Enter opens the picked day");
    assert_eq!(harness.state().calendar().focus(), day(5));
}

#[test]
fn the_calendar_s_keys_are_keymap_actions_that_rest_in_a_field_and_a_float() {
    // Rebound on the Shortcuts page: L moves the pick right.
    let mut harness = window(1.0, 100, View::Month);
    harness.state_mut().keymap_mut().bind(keymap::CALENDAR_RIGHT, egui::KeyboardShortcut::new(egui::Modifiers::NONE, Key::L));
    harness.key_press(Key::L);
    harness.run();
    assert_eq!(harness.state().calendar().picked_day(), Some(Day::new(2026, 11, 6).unwrap()));
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(harness.state().calendar().picked_day(), Some(Day::new(2026, 11, 6).unwrap()), "the old key does nothing");
    for id in keymap::CALENDAR_KEYS {
        assert!(keymap::action(id).is_some(), "{id}: on the Shortcuts page");
    }
    // In the week the arrows pick events; with the editor open (a float, a field focused) they rest.
    let mut harness = window(1.0, 100, View::Week);
    harness.key_press(Key::ArrowDown);
    harness.run();
    let first = harness.state().calendar().picked().map(|event| event.summary.clone());
    assert!(first.is_some(), "the first event is picked");
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().calendar().picked().map(|event| event.summary.clone()), first, "the field keeps its arrows");
    assert!(harness.state().calendar().editor().is_some());
}

#[test]
fn escape_in_the_editor_never_throws_typing_away_without_a_word() {
    let mut harness = window(1.0, 100, View::Week);
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text("Kiln");
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    let editor = harness.state().calendar().editor();
    assert!(editor.is_some_and(|editor| editor.form().summary == "Kiln"), "the first Esc keeps the typing");
    harness.get_by_label("What you typed is not saved. Save keeps it; Cancel, or Esc again, throws it away.");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().calendar().editor().is_none(), "the second Esc closes it");
}

#[test]
fn the_now_line_asks_for_a_repaint_once_a_minute_at_most() {
    for view in [View::Week, View::Day] {
        let mut harness = window(1.0, 100, view);
        let p = Palette::of(Mode::Dark);
        assert!(rects(&harness).iter().any(|(_, fill)| *fill == p.cyan_edge), "{view:?}: the now line shows");
        assert!(rests(&mut harness, 4), "{view:?}: no repaint sooner than a second");
        let delay = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap();
        assert!(delay <= Duration::from_secs(60), "{view:?}: the minute turns within a minute: {delay:?}");
    }
}

#[test]
fn every_scene_is_walked_by_the_keyboard_and_rests() {
    for view in View::ALL {
        let mut harness = window(1.0, 100, view);
        assert!(rests(&mut harness, 4), "{view:?} rests");
        let missed = keyboard_walk(&mut harness);
        assert!(missed.is_empty(), "{view:?}: Tab never reaches {missed:?}");
    }
    let mut harness = window(1.0, 100, View::Week);
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    // The editor is modal: Tab goes round its own controls, and reaches every one of them: every node in its float (the
    // area that holds its Title, under the window's root).
    let title = harness.get_by_role_and_label(Role::TextInput, "Title");
    let mut area = title.accesskit_node();
    while let Some(parent) = area.parent().filter(|parent| parent.parent().is_some()) {
        area = parent;
    }
    fn pressable(node: egui_kittest::kittest::AccessKitNode<'_>, out: &mut Vec<String>) {
        let data = node.data();
        if data.supports_action(egui::accesskit::Action::Click) || data.supports_action(egui::accesskit::Action::Focus) {
            out.extend(node.label());
        }
        node.children().for_each(|child| pressable(child, out));
    }
    let mut inside = Vec::new();
    pressable(area, &mut inside);
    let own = ["Title", "Starts", "Ends", "All day", "Private", "Repeat", "Busy", "Free", "Guests", "Cancel", "Save"];
    assert!(own.iter().all(|name| inside.iter().any(|label| label == name)), "the float holds {inside:?}");
    // A name the window also has outside the float (the rail's Calendar) may be missed as often as it is outside.
    let mut all = Vec::new();
    pressable(harness.root().accesskit_node(), &mut all);
    let missed = keyboard_walk(&mut harness);
    let count = |list: &[String], name: &String| list.iter().filter(|label| *label == name).count();
    let missed_own: Vec<&String> =
        inside.iter().filter(|name| count(&missed, name) > count(&all, name).saturating_sub(count(&inside, name))).collect();
    assert!(missed_own.is_empty(), "the editor: Tab never reaches {missed_own:?}");
}

/// An event of the sample's first calendar with `summary`, from `start` to `end`.
fn event(id: u64, summary: &str, (first, start): (Day, u16), (last, end): (Day, u16), all_day: bool) -> EventRow {
    EventRow {
        id,
        calendar: 1,
        summary: crate::mailbox::shown(summary),
        location: crate::mailbox::shown(summary),
        description: crate::mailbox::shown(summary),
        start: Moment { day: first, minute: start },
        end: Moment { day: last, minute: end },
        first,
        last: last.max(first),
        all_day,
        busy: true,
        private: false,
        repeat: None,
        zone: None,
        guests: Vec::new(),
        untitled: false,
    }
}

#[test]
fn hostile_events_never_panic_and_their_text_stays_in_its_cell() {
    let thursday = Day::new(2026, 11, 5).unwrap();
    let long = "Quarterly\u{202E}planning\u{0000}\u{0007}🙂🙂 with the bakery".repeat(40) + &"x".repeat(2000);
    let mut hostile = vec![
        event(9001, &long, (thursday, 600), (thursday, 600), false),
        event(9002, "Backwards", (thursday, 900), (thursday, 300), false),
        event(9003, "Over midnight", (thursday, 1380), (thursday.plus(1), 120), false),
        event(9004, "Four hundred days", (thursday.plus(-200), 0), (thursday.plus(200), 0), true),
        event(9005, "Far off", (Day::new(9999, 12, 31).unwrap_or(thursday), 0), (Day::new(9999, 12, 31).unwrap_or(thursday), 1439), false),
        event(9006, "Before time", (Day::EPOCH.plus(i64::MIN / 2), u16::MAX), (Day::EPOCH, u16::MAX), true),
    ];
    for index in 0..500_u64 {
        hostile.push(event(10_000 + index, &format!("Crowd {index}"), (thursday, 540), (thursday, 600), false));
    }
    for ppp in [1.0, 1.5] {
        for view in View::ALL {
            let mut harness = window_with(ppp, 100, view, true);
            harness.state_mut().calendar_mut().0.set_events(hostile.clone());
            harness.run();
            for clipped in harness.output().shapes.iter() {
                if let egui::Shape::Text(text) = &clipped.shape {
                    let wide = text.galley.size().x;
                    if text.galley.text().starts_with("Quarterly") {
                        assert!(wide <= clipped.clip_rect.width() + 1.0, "{view:?} {ppp}: the long title stays in its cell");
                        assert!(text.galley.elided, "{view:?} {ppp}: cut with one ellipsis");
                    }
                }
            }
            // The crowd lays out exactly its bound: four columns in the hours, the day's 50 rows in the agenda (the four
            // other events of Thursday come first), and in the month the cell's room, which the four others fill.
            let crowd = harness.query_all(by().role(Role::Button).label_contains("Crowd")).count();
            let bound = match view {
                View::Day | View::Week => super::hours::MOST_COLUMNS,
                View::Agenda => super::agenda::MOST_ROWS - 4,
                View::Month => 0,
            };
            assert_eq!(crowd, bound, "{view:?} {ppp}: {crowd} cells for 500 events");
            let link =
                harness.query_all(by().role(Role::Button)).any(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with('+')));
            assert!(link, "{view:?} {ppp}: the rest are counted in a +N more");
            // The day's float lists exactly its rows and counts the rest.
            harness.state_mut().calendar_mut().0.set_more(Some(thursday));
            harness.run();
            let held = hostile.iter().filter(|event| event.covers(thursday)).count();
            let rest = format!("and {} more in the day", held - super::more::MOST_ROWS);
            assert!(harness.query_by_label(&rest).is_some(), "{view:?} {ppp}: the float says {rest:?}");
            harness.state_mut().calendar_mut().0.set_more(None);
        }
    }
}

#[test]
fn a_key_shared_with_another_screen_s_action_works_on_each_screen() {
    // ArrowDown is also the key of an action of the mail screen's list: the calendar keeps it, the mail keeps its own.
    let mut harness = window(1.0, 100, View::Week);
    let down = egui::KeyboardShortcut::new(egui::Modifiers::NONE, Key::ArrowDown);
    harness.state_mut().keymap_mut().bind("ui.list.down", down);
    assert_eq!(harness.state().keymap().shortcut(keymap::CALENDAR_DOWN), Some(down));
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert!(harness.state().calendar().picked().is_some(), "on the calendar ArrowDown picks an event");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::Num1);
    harness.run();
    // The keyboard was on the calendar's event, gone with its screen.
    harness.ctx.memory_mut(|memory| memory.focused().map(|focused| memory.surrender_focus(focused)));
    harness.run();
    let before = harness.state().mailbox().selected();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_ne!(harness.state().mailbox().selected(), before, "on the mail ArrowDown moves the list");
}

#[test]
fn the_details_show_the_whole_text_and_scroll_to_the_rest() {
    let thursday = Day::new(2026, 11, 5).unwrap();
    let words = "Dial in from the studio, then the agenda: glaze tests, kiln schedule, the open day's stand. ";
    let mut long = event(9100, &words.repeat(3), (thursday, 600), (thursday, 660), false);
    let paragraphs: Vec<String> = (1..=30).map(|line| format!("Agenda point {line}: {words}")).collect();
    long.description = paragraphs.join("\n");
    long.location = format!("Room 3, {}", words.repeat(2));
    let mut harness = window(1.0, 100, View::Week);
    let mut events: Vec<EventRow> = harness.state().calendar().events().cloned().collect();
    events.push(long.clone());
    harness.state_mut().calendar_mut().0.set_events(events);
    harness.state_mut().calendar_mut().0.pick(Some(long.key()));
    harness.run();
    // Each part's text, row by row as painted (each row is painted on its own, and only the rows in sight): the panel
    // is scrolled through the part, and every row is kept by where it stands in the part.
    let side = 1440.0 - 252.0;
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    for whole in [long.summary.as_str(), long.location.as_str(), long.description.as_str()] {
        let spoken = whole.replace('\n', " ");
        let part = |harness: &Harness<'static, InboxApp>| harness.get_by_role_and_label(Role::Label, &spoken).rect();
        harness.get_by_role_and_label(Role::Label, &spoken).scroll_to_me();
        harness.run();
        // The part's top in sight first.
        let wheel = |harness: &mut Harness<'static, InboxApp>, by: f32| {
            harness.hover_at(egui::pos2(side + 100.0, 600.0));
            harness.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, by),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
            });
            harness.run();
        };
        for _ in 0..60 {
            if part(&harness).top() >= 23.0 {
                break;
            }
            wheel(&mut harness, 300.0);
        }
        let mut rows: std::collections::BTreeMap<i64, (String, bool)> = std::collections::BTreeMap::new();
        for _ in 0..60 {
            let rect = part(&harness);
            for clipped in &harness.output().shapes {
                if let egui::Shape::Text(text) = &clipped.shape
                    && text.pos.x >= side
                    && rect.y_range().contains(text.pos.y)
                    // In the panel's scrolling body (from 23 to 851), not its foot.
                    && clipped.clip_rect.top() >= 23.0
                    && clipped.clip_rect.y_range().contains(text.pos.y)
                    // An icon (the head's Close) is no text.
                    && !text.galley.text().chars().all(|c| ('\u{e000}'..='\u{f8ff}').contains(&c))
                {
                    rows.insert((text.pos.y - rect.top()).round() as i64, (text.galley.text().to_owned(), text.galley.elided));
                }
            }
            if rect.bottom() <= 851.0 {
                break;
            }
            wheel(&mut harness, -300.0);
        }
        assert!(rows.values().all(|(_, elided)| !elided), "nothing in the details is cut: {whole:.40}");
        let shown = words(&rows.values().map(|(text, _)| text.as_str()).collect::<Vec<_>>().join(" "));
        assert_eq!(shown, words(whole), "the details show the whole text: {whole:.40}");
    }
    let edit = harness.get_by_role_and_label(Role::Button, "Edit");
    edit.scroll_to_me();
    harness.run();
    let edit = harness.get_by_role_and_label(Role::Button, "Edit").rect();
    assert!(edit.bottom() <= 900.0 && edit.top() >= 0.0, "Edit scrolls into sight: {edit:?}");
}

/// The picked event's start day and title.
fn picked(harness: &Harness<'static, InboxApp>) -> Option<(Day, String)> {
    harness.state().calendar().picked().map(|event| (event.start.day, event.summary.clone()))
}

/// Presses `key` and lets the frame run.
fn press(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.key_press(key);
    harness.run();
}

/// The name of the node the keyboard is on.
fn focused_name(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
}

#[test]
fn arrows_move_the_picked_event_in_the_week() {
    let mut harness = window(1.0, 100, View::Week);
    let order: Vec<(Day, String)> = harness.state().calendar().events().map(|event| (event.start.day, event.summary.clone())).collect();
    press(&mut harness, Key::ArrowDown);
    assert_eq!(picked(&harness).as_ref(), order.first(), "the first event of the week");
    press(&mut harness, Key::ArrowDown);
    assert_eq!(picked(&harness).as_ref(), order.get(1), "the next one");
    press(&mut harness, Key::ArrowUp);
    assert_eq!(picked(&harness).as_ref(), order.first(), "back");
    // Right goes to the first event of a later day, Left back to the last of an earlier one.
    let from = picked(&harness).map(|(day, _)| day).unwrap();
    press(&mut harness, Key::ArrowRight);
    let to = picked(&harness).map(|(day, _)| day).unwrap();
    assert!(to > from, "Right goes to a later day: {from:?} to {to:?}");
    let expected = order.iter().find(|(day, _)| *day >= from.plus(1));
    assert_eq!(picked(&harness).as_ref(), expected);
    press(&mut harness, Key::ArrowLeft);
    let back = picked(&harness).map(|(day, _)| day).unwrap();
    assert!(back < to, "Left goes to an earlier day");
    // The keyboard is on the picked event: a screen reader hears it.
    let name = focused_name(&harness).unwrap_or_default();
    assert!(name.contains(&picked(&harness).unwrap().1), "focus on the pick: {name}");
    press(&mut harness, Key::Enter);
    assert_eq!(focused_name(&harness).as_deref(), Some("Close the details"), "Enter takes the keyboard to the details");
}

#[test]
fn arrows_step_the_day_and_move_the_picked_event_in_the_day_view() {
    let mut harness = window(1.0, 100, View::Day);
    let thursday = Day::new(2026, 11, 5).unwrap();
    assert_eq!(harness.state().calendar().focus(), thursday);
    press(&mut harness, Key::ArrowDown);
    let first = picked(&harness);
    assert!(first.as_ref().is_some_and(|(day, _)| *day == thursday), "the day's first event: {first:?}");
    press(&mut harness, Key::ArrowDown);
    assert_ne!(picked(&harness), first, "the next event of the day");
    press(&mut harness, Key::ArrowRight);
    assert_eq!(harness.state().calendar().focus(), thursday.plus(1), "Right steps a day on");
    press(&mut harness, Key::ArrowLeft);
    press(&mut harness, Key::ArrowLeft);
    assert_eq!(harness.state().calendar().focus(), thursday.plus(-1), "Left steps a day back");
}

#[test]
fn arrows_move_the_picked_event_in_the_agenda() {
    let mut harness = window(1.0, 100, View::Agenda);
    let order: Vec<(Day, String)> = harness.state().calendar().events().map(|event| (event.start.day, event.summary.clone())).collect();
    assert!(order.len() >= 3);
    press(&mut harness, Key::ArrowDown);
    press(&mut harness, Key::ArrowDown);
    press(&mut harness, Key::ArrowDown);
    assert_eq!(picked(&harness).as_ref(), order.get(2), "three down: the third event");
    press(&mut harness, Key::ArrowUp);
    assert_eq!(picked(&harness).as_ref(), order.get(1));
    press(&mut harness, Key::ArrowRight);
    assert_eq!(picked(&harness).as_ref(), order.get(2), "in the agenda Right is the next event");
    press(&mut harness, Key::ArrowLeft);
    assert_eq!(picked(&harness).as_ref(), order.get(1), "and Left the one before");
    let name = focused_name(&harness).unwrap_or_default();
    assert!(name.contains(&picked(&harness).unwrap().1), "focus on the pick: {name}");
    press(&mut harness, Key::Escape);
    assert!(picked(&harness).is_none(), "Esc lets the pick go");
}

#[test]
fn arrows_move_the_picked_day_in_the_mini_month() {
    let mut harness = window(1.0, 100, View::Week);
    let day = |date: u8| Day::new(2026, 11, date).unwrap();
    // The keyboard on today in the mini month, in the side panel (the week's head has a button of that name too).
    let side = 1440.0 - 252.0;
    harness.query_all(by().role(Role::Button).label("Thursday 5 November 2026, today")).find(|node| node.rect().left() >= side).unwrap().focus();
    harness.run();
    assert_eq!(harness.state().calendar().picked_day(), Some(day(5)), "Tab or focus on a day of the mini month picks it");
    for (key, date) in [(Key::ArrowRight, 6), (Key::ArrowDown, 13), (Key::ArrowLeft, 12), (Key::ArrowUp, 5), (Key::ArrowUp, 29)] {
        press(&mut harness, key);
        let want = if date == 29 { Day::new(2026, 10, 29).unwrap() } else { day(date) };
        assert_eq!(harness.state().calendar().picked_day(), Some(want), "{key:?}");
    }
    assert_eq!(harness.state().calendar().month(), Day::new(2026, 10, 1).unwrap(), "the mini month follows into October");
    assert_eq!(harness.state().calendar().view(), View::Week, "the view stays a week");
    assert_eq!(focused_name(&harness).as_deref(), Some("Thursday 29 October 2026"), "the keyboard follows the pick");
    press(&mut harness, Key::Enter);
    assert_eq!(harness.state().calendar().focus(), Day::new(2026, 10, 29).unwrap(), "Enter puts the view there");
}

#[test]
fn month_and_agenda_strokes_stand_on_whole_pixels() {
    let p = Palette::of(Mode::Dark);
    for ppp in SCALES {
        for percent in [100, 130] {
            for view in [View::Month, View::Agenda] {
                let mut harness = window(ppp, percent, view);
                // A picked event: the month's lit chip frame, the agenda's picked row; and today's box.
                let key = harness.state().calendar().events().find(|event| event.summary == "Call with Petra about the logo").map(EventRow::key);
                harness.state_mut().calendar_mut().0.pick(key);
                harness.run();
                let strokes: Vec<Rect> = rects(&harness)
                    .into_iter()
                    .filter(|(rect, fill)| {
                        // Under the toolbar: the days and the side panel (the toolbar's controls are the components').
                        rect.top() >= 48.0 * f32::from(percent) / 100.0
                            && [p.line, p.panel_line, p.cyan_edge].contains(fill)
                            && (rect.width() * ppp <= 2.5 || rect.height() * ppp <= 2.5)
                    })
                    .map(|(rect, _)| rect)
                    .collect();
                assert!(strokes.len() > 20, "{view:?} {ppp} {percent}: the strokes are found");
                let off: Vec<Rect> = strokes
                    .iter()
                    .copied()
                    .filter(|rect| ![rect.left(), rect.right(), rect.top(), rect.bottom()].iter().all(|edge| whole(edge * ppp)))
                    .collect();
                assert!(
                    off.is_empty(),
                    "{view:?} {ppp} {percent}: strokes between pixels: {:?}",
                    off.iter().map(|r| [r.left() * ppp, r.top() * ppp, r.right() * ppp, r.bottom() * ppp]).collect::<Vec<_>>()
                );
            }
        }
    }
}
