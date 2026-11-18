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

/// The pixels of one point's line at `ppp`.
fn line_px(ppp: f32) -> f32 {
    ppp.round().max(1.0)
}

#[test]
fn an_event_lies_on_the_grid_s_lines() {
    for ppp in SCALES {
        for percent in [100, 130] {
            let harness = window(ppp, percent, View::Week);
            // Monday 2 November, 09:00 to 10:00: on the hour lines and the day lines.
            let rect = event_rect(&harness, "Studio stand-up, Monday 2 November");
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
        let call = event_rect(&harness, "Call with Petra about the logo");
        let dentist = event_rect(&harness, "Dentist");
        let w = line_px(ppp) / ppp;
        assert!((call.right() - w - dentist.left()).abs() < 0.01, "{ppp}: the line between them is one: {call:?} {dentist:?}");
        assert!(call.left() < dentist.left());
        let across = lines(&harness, ppp, true);
        assert!(across.iter().any(|x| (x - call.left() * ppp).abs() < 0.01), "{ppp}: the first stands on the day's line");
        assert!(across.iter().any(|x| (x - (dentist.right() - w) * ppp).abs() < 0.01), "{ppp}: the second ends on the next day's line");
        // Two bands one above the other share one line too.
        let p = Palette::of(Mode::Dark);
        let band_line = (super::grid::lane_pitch(24.0, ppp) * ppp).round();
        assert_eq!(band_line, (24.0 * ppp).round() - line_px(ppp), "{ppp}: a lane is a band less its line");
        let _ = p;
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
    // The editor is modal: Tab goes round its own controls, and reaches every one of them.
    let missed = keyboard_walk(&mut harness);
    let own = ["Title", "Starts", "Ends", "All day", "Private", "Repeat", "Busy", "Free", "Guests", "Cancel", "Save"];
    let missed_own: Vec<&String> = missed.iter().filter(|label| own.contains(&label.as_str())).collect();
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
            // The crowd lays out a bounded number of cells.
            let crowd = harness.query_all(by().role(Role::Button).label_contains("Crowd")).count();
            assert!(crowd <= 64, "{view:?} {ppp}: {crowd} cells for 500 events");
            // "+N more" opens the day's events in a float, bounded too.
            let more =
                harness.query_all(by().role(Role::Button)).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with('+')));
            if let Some(more) = more {
                more.click();
                harness.run();
                let rows = harness.query_all(by().role(Role::Button).label_contains("Crowd")).count();
                assert!(rows <= 64 + super::more::MOST_ROWS, "{view:?}: the float lists at most its rows");
            }
        }
    }
}
