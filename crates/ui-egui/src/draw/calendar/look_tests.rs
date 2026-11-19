// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Small looks of the calendar's frames: New event pressed while the editor hangs from it, the toolbar's note 20 after
//! the title, the day's float counting its events with the number alone.

use egui::accesskit::Role;
use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use crate::calendar::{Day, View};
use crate::mailbox::Mailbox;
use crate::theme::{Mode, Palette};
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn window(view: View) -> Harness<'static, InboxApp> {
    window_at(view, vec2(1440.0, 900.0), egui::Theme::Dark)
}

fn window_at(view: View, size: egui::Vec2, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.show_calendar();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.set_view(mailbox, view);
    let mut harness = Harness::builder().with_size(size).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// Every filled rect of the last frame and its fill.
fn rects(harness: &Harness<'static, InboxApp>) -> Vec<(Rect, Color32)> {
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

/// The texts of the last frame: where each starts, how wide it is, and what it says.
fn texts(harness: &Harness<'static, InboxApp>) -> Vec<(f32, f32, String)> {
    harness
        .output()
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some((text.pos.x, text.galley.size().x, text.galley.text().to_owned())),
            _ => None,
        })
        .collect()
}

#[test]
fn new_event_is_pressed_while_the_editor_hangs_from_it() {
    let p = Palette::of(Mode::Dark);
    let mut harness = window(View::Week);
    let button = harness.get_by_role_and_label(Role::Button, "New event").rect();
    let pressed = |harness: &Harness<'static, InboxApp>| rects(harness).iter().any(|(rect, fill)| *fill == p.cyan_muted && *rect == button);
    assert!(!pressed(&harness), "at rest it is the cyan fill");
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    assert!(pressed(&harness), "open, the editor's button is pressed: cyan_muted");
    harness.state_mut().calendar_mut().0.close_editor();
    harness.run();
    assert!(!pressed(&harness));
}

#[test]
fn the_toolbar_s_note_stands_20_after_the_title() {
    let harness = window(View::Week);
    let found = texts(&harness);
    let title = found.iter().find(|(_, _, text)| text == "2 – 8 November 2026").unwrap();
    let note = found.iter().find(|(_, _, text)| text.starts_with("week ")).unwrap();
    assert!((note.0 - (title.0 + title.1) - 20.0).abs() < 0.01, "title {title:?}, note {note:?}");
}

/// The texts of the last frame as drawn (a cut text shows its ellipsis): where each starts, how wide, what it shows.
fn drawn(harness: &Harness<'static, InboxApp>) -> Vec<(f32, f32, String)> {
    // The command line's texts stand above the body: they are not the screen's.
    let body = crate::draw::shell::laid(&harness.ctx).map_or(0.0, |laid| laid.body.top());
    harness
        .output()
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if text.pos.y >= body => {
                let shown = text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect();
                Some((text.pos.x, text.galley.size().x, shown))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn in_the_pictures_window_the_title_stays_readable_its_note_giving_way_first() {
    // The pictures' 1200 × 820 beside the index: the view is about 710 wide, its controls take all but about 140.
    let cases = [
        (View::Day, "Thursday 5 November 2026", "Thu 5 Nov", "Previous day"),
        (View::Week, "2 – 8 November 2026", "2 – 8 Nov", "Previous week"),
        (View::Month, "November 2026", "Nov 2026", "Previous month"),
        (View::Agenda, "From today", "From today", "Earlier days"),
    ];
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        for (view, full, short, back) in cases {
            let harness = window_at(view, vec2(1200.0, 820.0), theme);
            let back = harness.get_by_role_and_label(Role::Button, back).rect();
            let found = drawn(&harness);
            let title = found.iter().find(|(_, _, text)| text == full || text == short);
            let Some((left, wide, _)) = title else { panic!("{view:?}: the title whole or short: {found:?}") };
            assert!(left + wide <= back.left(), "{view:?}: the title {title:?} left of {back:?}");
            let cut = found.iter().find(|(_, _, text)| text.ends_with('…') && short.chars().next().is_some_and(|first| text.starts_with(first)));
            assert!(cut.is_none(), "{view:?}: nothing of the title cut: {cut:?}");
            if let Some(note) = found.iter().find(|(_, _, text)| text.starts_with("week ") || text.ends_with(" days")) {
                assert!(note.0 + note.1 <= back.left(), "{view:?}: a note shown is whole: {note:?}");
            }
        }
    }
    // At the frame's 1440 the whole title and its note stand as before.
    let found = drawn(&window(View::Week));
    assert!(found.iter().any(|(_, _, text)| text == "2 – 8 November 2026") && found.iter().any(|(_, _, text)| text == "week 45"), "{found:?}");
}

#[test]
fn where_even_the_short_title_has_no_room_the_side_panel_gives_way_first() {
    // 900: one pane, the week alone (the side panel swaps with it). At 1000 two panes, but the view beside the 252
    // side panel would cut the title: the panel gives way. From 1050 the narrow index leaves the title its room.
    for (view, full, back) in [(View::Week, "2 – 8 November 2026", "Previous week"), (View::Day, "Thursday 5 November 2026", "Previous day")] {
        for (width, side) in [(900.0, false), (1000.0, false), (1050.0, true), (1100.0, true), (1150.0, true), (1440.0, true)] {
            let harness = window_at(view, vec2(width, 820.0), egui::Theme::Light);
            let found = drawn(&harness);
            assert_eq!(found.iter().any(|(_, _, text)| text == "CALENDARS"), side, "{view:?} at {width}: the side panel shown {side}");
            let back = harness.get_by_role_and_label(Role::Button, back).rect();
            // The title 16 in from the body's left (the index 232 or 56 beside it).
            let start = crate::draw::shell::laid(&harness.ctx).unwrap().body.left() + 16.0;
            let title = found.iter().find(|(left, _, _)| (*left - start).abs() < 20.0 && *left < back.left());
            let Some((left, wide, text)) = title else { panic!("{view:?} at {width}: no title {found:?}") };
            assert!(!text.ends_with('…'), "{view:?} at {width}: the title {text:?} not cut");
            assert!(left + wide <= back.left(), "{view:?} at {width}: {text:?} left of {back:?}");
            if !side {
                assert_eq!(text, full, "{view:?} at {width}: without the side panel the whole title");
            }
        }
    }
}

#[test]
fn the_toolbar_s_groups_stand_24_apart_and_their_controls_8() {
    let harness = window(View::Week);
    let rect = |role, name| harness.get_by_role_and_label(role, name).rect();
    let back = rect(Role::Button, "Previous week");
    let today = rect(Role::Button, "Today");
    let on = rect(Role::Button, "Next week");
    let views = rect(Role::RadioGroup, "Day / Week / Month / Agenda");
    let new = rect(Role::Button, "New event");
    // As the frame: step back 391, Today 431..494, step on 502..534, the views 558..801, New event 825.
    assert_eq!(today.left() - back.right(), 8.0, "{back:?} {today:?}");
    assert_eq!(on.left() - today.right(), 8.0, "{today:?} {on:?}");
    assert_eq!(views.left() - on.right(), 24.0, "{on:?} {views:?}");
    assert_eq!(new.left() - views.right(), 24.0, "{views:?} {new:?}");
}

#[test]
fn one_line_stands_between_the_heads_or_the_all_day_lane_and_the_hours() {
    let p = Palette::of(Mode::Dark);
    for view in [View::Week, View::Day] {
        let harness = window(view);
        let found = rects(&harness);
        let across = |colour: Color32| -> Vec<Rect> {
            found.iter().filter(|(rect, fill)| *fill == colour && rect.width() > 400.0 && rect.height() <= 1.0).map(|(rect, _)| *rect).collect()
        };
        let lines = across(p.line);
        // The frame: the `line` under the heads (or the lane), then the hours' `surface`; the first hour line 48 lower.
        for under in &lines {
            let doubled = across(p.panel_line).into_iter().find(|hour| (hour.top() - under.bottom()).abs() < 0.01);
            assert!(doubled.is_none(), "{view:?}: {doubled:?} right under {under:?}");
        }
    }
}

#[test]
fn the_agenda_s_rows_run_the_view_s_whole_width_under_the_scroll_bar() {
    let p = Palette::of(Mode::Dark);
    let harness = window(View::Agenda);
    let found = rects(&harness);
    // The `line` under the toolbar runs the view's width; the frame's row hairlines run as far, the thumb over them.
    let toolbar = found
        .iter()
        .filter(|(rect, fill)| *fill == p.line && rect.height() <= 1.0 && rect.width() > 400.0 && (20.0..200.0).contains(&rect.top()))
        .map(|(rect, _)| *rect)
        .next()
        .unwrap();
    let rows: Vec<Rect> =
        found.iter().filter(|(rect, fill)| *fill == p.panel_line && rect.height() <= 1.0 && rect.width() > 400.0).map(|(rect, _)| *rect).collect();
    assert!(!rows.is_empty());
    for row in rows {
        assert_eq!((row.left(), row.right()), (toolbar.left(), toolbar.right()), "{row:?} under {toolbar:?}");
    }
}

#[test]
fn the_hours_draw_every_day_line_the_last_under_the_scroll_bar() {
    let p = Palette::of(Mode::Dark);
    for (view, days) in [(View::Week, 7), (View::Day, 1)] {
        let harness = window(view);
        // Down the hours: the gutter's line and one after each day, the last (the frame's x 940) too.
        let mut lefts: Vec<f32> = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                // What is in sight: the part of the line inside its clip.
                egui::Shape::Rect(rect) if rect.fill == p.panel_line => Some(rect.rect.intersect(clipped.clip_rect)),
                _ => None,
            })
            .filter(|rect| rect.width() > 0.0 && rect.width() <= 1.0 && rect.height() > 300.0)
            .map(|rect| rect.left())
            .collect();
        lefts.sort_by(f32::total_cmp);
        lefts.dedup();
        assert_eq!(lefts.len(), days + 1, "{view:?}: {lefts:?}");
    }
}

/// The thumbs painted in `muted` inside the bar over the view's right edge, x over 900: within the thumb's place on
/// its strip ([`crate::component::scroll::over_thumb`]), painted on whole pixels.
fn thumbs_at_the_right(harness: &Harness<'static, InboxApp>) -> Vec<Rect> {
    let p = Palette::of(Mode::Dark);
    let strips: Vec<Rect> =
        crate::component::scroll::over_thumb(&harness.ctx).into_iter().filter(|strip| strip.left() > 900.0).map(|strip| strip.expand(1.0)).collect();
    rects(harness)
        .into_iter()
        .filter(|(rect, fill)| *fill == p.muted && rect.width() <= 8.0 && rect.height() >= 20.0)
        .map(|(rect, _)| rect)
        .filter(|rect| strips.iter().any(|strip| strip.contains_rect(*rect)))
        .collect()
}

#[test]
fn the_agenda_and_the_hours_paint_their_thumb_over_the_content() {
    for view in [View::Agenda, View::Week, View::Day] {
        let harness = window(view);
        assert_eq!(thumbs_at_the_right(&harness).len(), 1, "{view:?}: one thumb in the strip at the right");
    }
}

/// The week with one event more, "At the edge", on its last day, Sunday 8 November, 13:00 to 14:00.
fn edged() -> Harness<'static, InboxApp> {
    use crate::calendar::{EventRow, Moment};
    let mut harness = window(View::Week);
    let sunday = Day::new(2026, 11, 8).unwrap();
    let mut events: Vec<EventRow> = harness.state().calendar().events().cloned().collect();
    let mut event = events.iter().find(|event| !event.all_day).cloned().unwrap();
    event.id = 41_000;
    event.summary = "At the edge".to_owned();
    (event.start, event.end) = (Moment { day: sunday, minute: 780 }, Moment { day: sunday, minute: 840 });
    (event.first, event.last, event.all_day, event.repeat) = (sunday, sunday, false, None);
    events.push(event);
    harness.state_mut().calendar_mut().0.set_events(events);
    harness.run();
    harness
}

#[test]
fn a_press_on_the_right_edge_of_an_event_in_the_last_day_picks_it_not_the_scroll_strip() {
    let block = |harness: &Harness<'static, InboxApp>| {
        harness.query_all(egui_kittest::kittest::by().role(Role::Button).label_contains("At the edge")).next().unwrap().rect()
    };
    for back in [11.0, 6.0, 1.0] {
        let mut harness = edged();
        let block = block(&harness);
        let at = egui::pos2(block.right() - back, block.center().y);
        harness.event(egui::Event::PointerMoved(at));
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            harness.step();
        }
        harness.run();
        let picked = harness.state().calendar().picked().map(|event| event.summary.clone());
        assert_eq!(picked.as_deref(), Some("At the edge"), "a press at {at:?} on {block:?}");
    }
}

#[test]
fn the_hours_thumb_drags_the_content_and_a_press_beside_it_scrolls_nothing() {
    let p = Palette::of(Mode::Dark);
    let top = |harness: &Harness<'static, InboxApp>| {
        harness.query_all(egui_kittest::kittest::by().role(Role::Button).label_contains("At the edge")).next().unwrap().rect().top()
    };
    let mut harness = edged();
    let before = top(&harness);
    let thumb = thumbs_at_the_right(&harness)[0];
    let from = thumb.center();
    harness.event(egui::Event::PointerMoved(from));
    harness.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    for step in 1..=4 {
        harness.event(egui::Event::PointerMoved(from + vec2(0.0, 10.0 * step as f32)));
        harness.step();
    }
    let held = rects(&harness).iter().any(|(rect, fill)| *fill == p.ink && rect.width() <= 8.0 && rect.height() >= 20.0 && rect.left() > 900.0);
    harness.event(egui::Event::PointerButton {
        pos: from + vec2(0.0, 40.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    assert!(held, "held, the thumb is ink");
    let dragged = top(&harness);
    assert!(dragged < before - 20.0, "40 down the travel scrolls the content: {before} then {dragged}");
    // In the strip but past the thumb's end the press goes to what is under it; nothing scrolls.
    let thumb = thumbs_at_the_right(&harness)[0];
    let below = egui::pos2(thumb.center().x, thumb.bottom() + 8.0);
    harness.event(egui::Event::PointerMoved(below));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: below, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    harness.run();
    assert_eq!(top(&harness), dragged, "a press beside the thumb at {below:?}");
}

#[test]
fn today_s_head_has_a_1_px_cyan_edge_under_it() {
    let p = Palette::of(Mode::Dark);
    let harness = window(View::Week);
    // The frame's head: a `cyan_edge` bottom of 1 px, as wide as the day's column, over the heads' `line`.
    let under: Vec<Rect> = rects(&harness)
        .into_iter()
        .filter(|(rect, fill)| *fill == p.cyan_edge && rect.top() < 150.0 && rect.height() <= 2.0 && (100.0..200.0).contains(&rect.width()))
        .map(|(rect, _)| rect)
        .collect();
    assert_eq!(under.len(), 1, "{under:?}");
    assert!(under.iter().all(|rect| rect.height() == 1.0), "{under:?}");
}

#[test]
fn the_day_s_float_counts_its_events_with_the_number_alone() {
    let mut harness = window(View::Month);
    let thursday = Day::new(2026, 11, 5).unwrap();
    let held = harness.state().calendar().events().filter(|event| event.covers(thursday)).count();
    harness.state_mut().calendar_mut().0.set_more(Some(thursday));
    harness.run();
    let found = texts(&harness);
    assert!(found.iter().any(|(_, _, text)| *text == held.to_string()), "the count as the design writes it: {held}");
    assert!(!found.iter().any(|(_, _, text)| text.ends_with(" events") && text.starts_with(&held.to_string())));
}

#[test]
fn the_side_panel_s_content_stands_16_in_and_the_mini_month_is_220_wide() {
    let p = Palette::of(Mode::Dark);
    let harness = window(View::Week);
    // The panel 252 wide at the right of the card, after its hairline; its scrolling part 15 in, its content 1 further,
    // as the frame pads it.
    let found = rects(&harness);
    // The leftmost such line: the card's own right edge, a band of the same look, stands further right.
    let (edge, _) = found
        .iter()
        .copied()
        .filter(|(rect, fill)| *fill == p.line && rect.width() == 1.0 && rect.height() > 500.0 && rect.left() > 1000.0)
        .min_by(|a, b| a.0.left().total_cmp(&b.0.left()))
        .unwrap();
    let mini = found.into_iter().find(|(rect, fill)| *fill == p.surface && rect.left() > edge.right() && (rect.height() - 240.0).abs() < 41.0);
    let (mini, _) = mini.unwrap();
    assert_eq!(mini.left(), edge.right() + 16.0, "{mini:?}");
    assert_eq!(mini.width(), 220.0, "{mini:?}");
    assert_eq!(mini.top(), edge.top() + 16.0, "{mini:?}");
}

#[test]
fn the_mini_month_s_arrows_stand_where_the_sheet_draws_them() {
    let p = Palette::of(Mode::Dark);
    let harness = window(View::Week);
    let mini = rects(&harness)
        .into_iter()
        .find(|(rect, fill)| *fill == p.surface && rect.left() > 1100.0 && rect.width() == 220.0)
        .map(|(rect, _)| rect)
        .unwrap();
    // The sheet: the head 12 in, "‹" an 8-wide box at +168 and "›" at +188, so their middles at +184 and +204.
    let found = texts(&harness);
    for (sign, middle) in [("‹", 184.0), ("›", 204.0)] {
        let (x, wide, _) = found.iter().find(|(_, _, text)| text == sign).unwrap();
        let at = x + wide / 2.0 - mini.left();
        assert!((at - middle).abs() <= 0.5, "{sign} stands at +{at}, the sheet's +{middle}");
    }
}
