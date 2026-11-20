// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The curve and an event's glide: it moves only while it runs, lands on its cell, and stays put with motion reduced.

use egui::{Context, RawInput, Rect, pos2, vec2};

use super::glide;
use crate::calendar::{Day, EventRow, Moment};
use crate::motion::{css_ease, ease};

fn event(repeat: Option<&str>) -> EventRow {
    let day = Day::new(2026, 11, 5).unwrap();
    EventRow {
        id: 7,
        calendar: 1,
        summary: "Kiln".to_owned(),
        location: String::new(),
        description: String::new(),
        start: Moment { day, minute: 540 },
        end: Moment { day, minute: 600 },
        first: day,
        last: day,
        all_day: false,
        busy: true,
        private: false,
        repeat: repeat.map(str::to_owned),
        zone: None,
        guests: Vec::new(),
        alert: crate::calendar::Alert::Default,
        untitled: false,
    }
}

#[test]
fn the_curve_starts_quick_and_lands_soft() {
    assert!(ease(0.0).abs() < 1e-3);
    assert!((ease(1.0) - 1.0).abs() < 1e-3);
    assert!(ease(0.25) > 0.5, "a quick start: {}", ease(0.25));
    let mut last = 0.0;
    for step in 0..=100 {
        let at = ease(step as f32 / 100.0);
        assert!(at + 1e-4 >= last, "never back");
        assert!(at <= 1.0 + 1e-4, "never past the end");
        last = at;
    }
    assert!((ease(f32::NAN) - 1.0).abs() < 1e-3);
}

/// Draws the event at `rect` at `time`; where it stood and whether a repaint was asked for.
fn frame(ctx: &Context, event: &EventRow, rect: Rect, time: f64) -> (Rect, bool) {
    let span = (Day::new(2026, 11, 2).unwrap(), Day::new(2026, 11, 8).unwrap());
    let mut shown = rect;
    let mut output = ctx.run_ui(RawInput { time: Some(time), ..RawInput::default() }, |ui| shown = glide(ui.ctx(), event, (span, 3), rect));
    output.textures_delta.clear();
    let repaint = output.viewport_output.values().any(|viewport| viewport.repaint_delay.is_zero());
    (shown, repaint)
}

#[test]
fn a_moved_event_glides_to_its_new_cell_and_then_rests() {
    let ctx = Context::default();
    let before = Rect::from_min_size(pos2(56.0, 96.0), vec2(127.0, 49.0));
    let after = before.translate(vec2(0.0, 96.0));
    assert_eq!(frame(&ctx, &event(None), before, 0.0).0, before, "the first frame is where it is");
    assert_eq!(frame(&ctx, &event(None), before, 0.016).0, before);
    let (moving, repaint) = frame(&ctx, &event(None), after, 0.032);
    assert!((moving.top() - before.top()).abs() < 0.5, "it starts from where it stood: {moving:?}");
    assert!(repaint, "a repaint while it moves");
    let (half, _) = frame(&ctx, &event(None), after, 0.032 + 0.1);
    assert!(half.top() > before.top() && half.top() < after.top(), "on its way: {half:?}");
    let (landed, _) = frame(&ctx, &event(None), after, 0.032 + 0.31);
    assert_eq!(landed, after);
    let (still, repaint) = frame(&ctx, &event(None), after, 0.032 + 0.4);
    assert_eq!(still, after);
    assert!(!repaint, "at rest once landed");
}

#[test]
fn a_repeating_event_and_reduced_motion_never_glide() {
    let ctx = Context::default();
    let before = Rect::from_min_size(pos2(56.0, 96.0), vec2(127.0, 49.0));
    let after = before.translate(vec2(0.0, 48.0));
    frame(&ctx, &event(Some("weekly")), before, 0.0);
    assert_eq!(frame(&ctx, &event(Some("weekly")), after, 0.016).0, after);
    let ctx = Context::default();
    crate::motion::set_reduced(&ctx, true);
    frame(&ctx, &event(None), before, 0.0);
    assert_eq!(frame(&ctx, &event(None), after, 0.016).0, after);
}

#[test]
fn the_ground_s_curve_is_css_s_ease() {
    // cubic-bezier(.25, .1, .25, 1): a slow start, most of the way by the middle.
    assert!(css_ease(0.0).abs() < 1e-3 && (css_ease(1.0) - 1.0).abs() < 1e-3);
    assert!((css_ease(0.5) - 0.8024).abs() < 2e-3, "{}", css_ease(0.5));
    assert!((css_ease(0.1) - 0.0951).abs() < 2e-3, "{}", css_ease(0.1));
    // The block's fade runs on it: at half of `fast` the ground is that far.
    let ctx = Context::default();
    let id = egui::Id::unique("ground");
    let at = |time: f64, on: bool| {
        let mut value = 0.0;
        let mut output = ctx
            .run_ui(RawInput { time: Some(time), ..RawInput::default() }, |ui| value = crate::motion::fade_eased(ui.ctx(), id, on, 0.125, css_ease));
        output.textures_delta.clear();
        value
    };
    assert!(at(0.0, false).abs() < 1e-3);
    at(0.0, true);
    let half = at(0.0625, true);
    assert!((half - css_ease(0.5)).abs() < 0.05, "half way through: {half}");
}

#[test]
fn a_glide_moves_the_top_and_the_height_is_at_once_the_new_one() {
    let ctx = Context::default();
    let before = Rect::from_min_size(pos2(56.0, 96.0), vec2(127.0, 49.0));
    let after = Rect::from_min_size(pos2(56.0, 144.0), vec2(127.0, 97.0));
    frame(&ctx, &event(None), before, 0.0);
    frame(&ctx, &event(None), before, 0.016);
    frame(&ctx, &event(None), after, 0.032);
    let (half, _) = frame(&ctx, &event(None), after, 0.032 + 0.1);
    assert!(half.top() > before.top() && half.top() < after.top(), "the top glides: {half:?}");
    assert!((half.height() - after.height()).abs() < 1e-3, "the height is the new one at once, as the design's top transition: {half:?}");
    assert_eq!(half.x_range(), after.x_range());
}
