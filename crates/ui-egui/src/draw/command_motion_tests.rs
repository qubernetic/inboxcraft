// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command line (M8) at the three levels, frames 10 ms apart: the results unfold a line a `stagger` at full and
//! come together at reduced and off; the caret blinks at full and reduced and stands still at off; a key during the
//! unfolding acts in its frame with every line there; and the line rests but for the caret.

use std::time::Duration;

use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use crate::InboxApp;
use crate::command::field_id;
use crate::draw::list::rows_motion_tests::window;
use crate::motion::Level;

/// Ctrl K, at rest, then `>` typed as one event: the frame of the key, which lists the commands, is the last drawn.
fn open(harness: &mut Harness<'static, InboxApp>) {
    harness.event(egui::Event::Key { key: Key::K, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    harness.run_steps(20);
    harness.event(egui::Event::Text(">".to_owned()));
    harness.step();
}

/// The rows of the results drawn in the last frame.
fn rows(harness: &Harness<'static, InboxApp>) -> usize {
    harness.query_all_by_role(Role::MenuItem).count()
}

/// Whether the field's caret was painted in the last frame: a narrow block of `cyan_edge` in its box (the scope's chip is
/// wider).
fn caret(harness: &Harness<'static, InboxApp>) -> bool {
    let Some(field) = harness.ctx.data(|data| data.get_temp::<egui::Rect>(field_id().with("box"))) else { return false };
    let cyan = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::default()).palette.cyan_edge;
    harness.output().shapes.iter().any(|clipped| {
        matches!(&clipped.shape, egui::Shape::Rect(block) if block.fill == cyan && field.contains_rect(block.rect) && block.rect.width() < 10.0)
    })
}

/// Whether nothing but the caret's blink (at its next change) asks for a repaint: no movement asks for the next frame.
fn rests_but_the_caret(harness: &mut Harness<'static, InboxApp>) -> bool {
    harness.run_steps(2);
    let delay = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap_or(Duration::MAX);
    delay > Duration::ZERO
}

#[test]
fn at_full_the_results_unfold_a_line_every_30_ms_each_whole() {
    let mut harness = window(Level::Full);
    open(&mut harness);
    assert!(harness.state().command().results(), "0 ms: open in the frame of the key");
    assert_eq!(rows(&harness), 0, "0 ms: no line yet");
    harness.run_steps(6);
    assert_eq!(rows(&harness), 1, "60 ms: two lines, the head and one row");
    harness.run_steps(9);
    assert_eq!(rows(&harness), 4, "150 ms: five lines");
    harness.run_steps(25);
    let all = rows(&harness);
    assert!(all > 10, "400 ms: the list whole, {all}");
    assert!(rests_but_the_caret(&mut harness), "and rests but for the caret");
}

#[test]
fn at_reduced_and_off_the_results_come_together() {
    for level in [Level::Reduced, Level::Off] {
        let mut harness = window(level);
        open(&mut harness);
        let all = rows(&harness);
        assert!(all > 10, "{level:?} 0 ms: the list whole, {all}");
        assert!(rests_but_the_caret(&mut harness), "{level:?}: and rests but for the caret");
    }
}

#[test]
fn the_caret_blinks_at_full_and_reduced_and_stands_still_at_off() {
    for (level, blinks) in [(Level::Full, true), (Level::Reduced, true), (Level::Off, false)] {
        let mut harness = window(level);
        open(&mut harness);
        harness.run_steps(40);
        assert!(caret(&harness), "{level:?} 400 ms: the caret on");
        harness.run_steps(30);
        assert_eq!(caret(&harness), !blinks, "{level:?} 700 ms: off for the second half of the second, or still");
    }
    let mut harness = window(Level::Off);
    open(&mut harness);
    assert!(crate::component::tests::rests(&mut harness, 20), "Off: nothing asks for a repaint");
}

#[test]
fn a_key_while_the_results_unfold_acts_in_its_frame_and_every_line_is_there() {
    let mut harness = window(Level::Full);
    open(&mut harness);
    harness.run_steps(3);
    let all = {
        let mut whole = window(Level::Off);
        open(&mut whole);
        rows(&whole)
    };
    assert!(rows(&harness) < all, "30 ms: unfolding");
    harness.event(egui::Event::Key { key: Key::ArrowDown, pressed: true, modifiers: Modifiers::NONE, repeat: false, physical_key: None });
    harness.step();
    assert_eq!(rows(&harness), all, "the key ends it in its frame");
    assert_eq!(harness.state().command().picked(), 1, "and acts: the pick moved");
}
