// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A screener answer (M11) at the three levels, frame by frame on New senders, frames 10 ms apart: the answer in the
//! frame of its press, the answered card's edge, its way out of its slot, the next card stepping up, a key during it,
//! and rest.

use egui::accesskit::Role;
use egui::{Color32, Key, Modifiers, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::Mailbox;
use crate::motion::Level;
use crate::theme::{Mode, TextScale, Theme};
use crate::{InboxApp, sample};

/// The window on New senders at `level`, at rest, frames 10 ms apart.
fn queue(level: Level) -> Harness<'static, InboxApp> {
    let clock = || Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()));
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = crate::component::tests::builder()
        .with_size(vec2(1300.0, 900.0))
        .with_step_dt(0.01)
        .with_max_steps(100)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().set_motion_level(level);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run_steps(60);
    harness
}

/// The tops of the queue's Let in buttons, top down: the cards still asking.
fn let_in_tops(harness: &Harness<'static, InboxApp>) -> Vec<f32> {
    let mut tops: Vec<f32> = harness.query_all_by_role_and_label(Role::Button, "Let in").map(|node| node.rect().top()).collect();
    tops.sort_by(f32::total_cmp);
    tops
}

/// The answer's colour in either mode: `minus` for Keep out, `plus` for Let in.
fn answer(keep_out: bool) -> Vec<Color32> {
    [Mode::Light, Mode::Dark]
        .map(|mode| {
            let p = Theme::new(mode, TextScale::default()).palette;
            if keep_out { p.minus } else { p.plus }
        })
        .to_vec()
}

/// The rectangle edged in the answer's colour, as drawn (its colour premultiplied by its opacity), and that opacity.
fn answered_edge(harness: &Harness<'static, InboxApp>, keep_out: bool) -> Option<(Rect, f32)> {
    let colours = answer(keep_out);
    let mut found = None;
    let mut look = |shape: &egui::Shape| {
        if let egui::Shape::Rect(rect) = shape
            && rect.stroke.width > 0.0
            && rect.stroke.color.a() > 0
            && rect.rect.height() > 60.0
        {
            let seen = rect.stroke.color;
            let alpha = f32::from(seen.a()) / 255.0;
            let matches = colours.iter().any(|full| {
                [(seen.r(), full.r()), (seen.g(), full.g()), (seen.b(), full.b())]
                    .iter()
                    .all(|(seen, full)| (f32::from(*seen) - f32::from(*full) * alpha).abs() <= 2.0)
            });
            if matches {
                found = Some((rect.rect, alpha));
            }
        }
    };
    for clipped in &harness.output().shapes {
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut look),
            shape => look(shape),
        }
    }
    found
}

/// The first card's answer pressed: the frame of the press is the last drawn.
fn answer_first(harness: &mut Harness<'static, InboxApp>, keep_out: bool) {
    let name = if keep_out { "Keep out" } else { "Let in" };
    let top = |node: &egui_kittest::Node<'_>| node.rect().top();
    let first = harness.query_all_by_role_and_label(Role::Button, name).min_by(|a, b| top(a).total_cmp(&top(b))).unwrap();
    first.click();
    harness.step();
}

#[test]
fn at_off_the_next_card_is_there_in_the_frame_of_the_answer() {
    let mut harness = queue(Level::Off);
    let tops = let_in_tops(&harness);
    answer_first(&mut harness, true);
    assert!(harness.state().mailbox().can_undo(), "the answer acts at once");
    assert_eq!(let_in_tops(&harness), tops[..tops.len() - 1].to_vec(), "the owner never waits: the next card in its place in that frame");
    assert!(answered_edge(&harness, true).is_none(), "the answered card is gone");
    harness.state_mut().mailbox_mut().forget_undo();
    harness.run_steps(40);
    assert!(crate::component::tests::rests(&mut harness, 4));
}

#[test]
fn at_full_the_answered_card_takes_its_colour_leaves_24_px_in_300_ms_and_the_next_steps_up_in_180() {
    for keep_out in [true, false] {
        let mut harness = queue(Level::Full);
        let tops = let_in_tops(&harness);
        let (first, second) = (tops[0], tops[1]);
        answer_first(&mut harness, keep_out);
        assert!(harness.state().mailbox().can_undo(), "{keep_out}: the answer acts at once");
        let (at, alpha) = answered_edge(&harness, keep_out).expect("0 ms: the edge in the answer's colour in that frame");
        assert!(alpha > 0.95, "{keep_out}: 0 ms: whole: {alpha}");
        harness.run_steps(12);
        let (moved, alpha) = answered_edge(&harness, keep_out).expect("120 ms: still leaving");
        let by = moved.top() - at.top();
        let toward = if keep_out { by } else { -by };
        assert!(toward > 17.0 && toward <= 24.0, "{keep_out}: 120 ms: about 21 of 24 px {}: {by}", if keep_out { "down" } else { "up" });
        assert!(alpha > 0.5 && alpha < 0.7, "{keep_out}: 120 ms: 60 % there: {alpha}");
        assert_eq!(let_in_tops(&harness).last().copied(), Some(second), "{keep_out}: the slot stays open");
        harness.run_steps(17);
        assert_eq!(let_in_tops(&harness).last().copied(), Some(second), "{keep_out}: 290 ms: the slot still open");
        harness.run_steps(19);
        assert_eq!(let_in_tops(&harness).last().copied(), Some(first), "{keep_out}: 480 ms: the next card stepped up");
        assert!(answered_edge(&harness, keep_out).is_none(), "{keep_out}: the answered card is gone");
        harness.state_mut().mailbox_mut().forget_undo();
        harness.run_steps(40);
        assert!(crate::component::tests::rests(&mut harness, 4), "{keep_out}: and it rests");
    }
}

#[test]
fn at_reduced_the_answered_card_fades_in_125_ms_and_the_next_steps_up_in_180() {
    let mut harness = queue(Level::Reduced);
    let tops = let_in_tops(&harness);
    let (first, second) = (tops[0], tops[1]);
    answer_first(&mut harness, true);
    let (at, alpha) = answered_edge(&harness, true).expect("0 ms: the edge is minus in that frame");
    assert!(alpha > 0.95, "0 ms: whole: {alpha}");
    harness.run_steps(6);
    let (still, alpha) = answered_edge(&harness, true).expect("60 ms: fading");
    assert_eq!(still.top(), at.top(), "60 ms: in its place");
    assert!(alpha > 0.35 && alpha < 0.65, "60 ms: half way out: {alpha}");
    harness.run_steps(6);
    assert_eq!(let_in_tops(&harness).last().copied(), Some(second), "120 ms: its slot still open");
    harness.run_steps(19);
    assert_eq!(let_in_tops(&harness).last().copied(), Some(first), "310 ms: the next card stepped up");
    harness.state_mut().mailbox_mut().forget_undo();
    harness.run_steps(40);
    assert!(crate::component::tests::rests(&mut harness, 4));
}

#[test]
fn a_key_while_a_card_leaves_acts_in_its_frame_and_the_next_card_is_in_place() {
    for level in [Level::Full, Level::Reduced] {
        let mut harness = queue(level);
        let tops = let_in_tops(&harness);
        answer_first(&mut harness, true);
        harness.run_steps(4);
        let focused = harness.query_by(|node| node.is_focused()).map(|node| (node.rect().min, node.accesskit_node().label()));
        harness.event(egui::Event::Key { key: Key::Tab, pressed: true, modifiers: Modifiers::NONE, repeat: false, physical_key: None });
        harness.step();
        assert_ne!(
            harness.query_by(|node| node.is_focused()).map(|node| (node.rect().min, node.accesskit_node().label())),
            focused,
            "{level:?}: Tab moved on in its frame"
        );
        assert_eq!(let_in_tops(&harness), tops[..tops.len() - 1].to_vec(), "{level:?}: the next card in its place in that frame");
        assert!(answered_edge(&harness, true).is_none(), "{level:?}: the answered card is gone");
    }
}

/// A pointer press at `at`, held `hold` frames, then let go (a click spread over frames, as a hand makes it).
pub(crate) fn hold_click(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2, hold: usize) {
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    harness.event(egui::Event::PointerMoved(at));
    harness.step();
    harness.event(button(true));
    harness.step();
    harness.run_steps(hold);
    harness.event(button(false));
    harness.step();
}

#[test]
fn a_click_on_the_next_card_held_across_the_slots_close_answers_it() {
    // Each level at both moments the answered card changes its form: its slot starts closing, then it is let go. The
    // press comes before the moment and the release after it.
    let cases = [(Level::Reduced, 8, 6), (Level::Reduced, 28, 6), (Level::Full, 25, 8), (Level::Full, 45, 8)];
    let mut lost = Vec::new();
    for (level, wait, hold) in cases {
        let mut harness = queue(level);
        let asking = let_in_tops(&harness).len();
        answer_first(&mut harness, true);
        harness.run_steps(wait);
        let mut lets: Vec<_> = harness.query_all_by_role_and_label(Role::Button, "Let in").map(|node| node.rect()).collect();
        lets.sort_by(|a, b| a.top().total_cmp(&b.top()));
        // While the answered card still stands in its slot, its own (dead) answers are the first.
        let next = usize::from(answered_edge(&harness, true).is_some());
        let at = lets.get(next).map(|rect| rect.center()).expect("the next card's Let in");
        hold_click(&mut harness, at, hold);
        harness.run_steps(80);
        let left = let_in_tops(&harness).len();
        if left + 2 != asking {
            lost.push((level, wait, asking, left));
        }
    }
    assert!(lost.is_empty(), "the second answer was lost: {lost:?}");
}
