// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! How far the list scrolls: after Done on the last row in sight, and when the window shrinks in the pass after a
//! step.

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::{InboxView, Mailbox};
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window in `view` with motion, a frame every sixtieth of a second, `height` high.
fn moving(view: InboxView, height: f32) -> Harness<'static, InboxApp> {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    if view == InboxView::List {
        mailbox.set_inbox_view(InboxView::List);
    } else {
        let invented = sample::showcase(&mailbox);
        mailbox.set_showcase(invented);
        mailbox.set_inbox_view(view);
    }
    let mut app = InboxApp::at_mail(mailbox);
    app.set_reduce_motion(false);
    let mut harness = crate::component::tests::builder()
        .with_size(vec2(1300.0, height))
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(240)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run_steps(4);
    harness
}

/// The rect of message `id`'s row as the last pass built it.
fn row(harness: &Harness<'static, InboxApp>, id: u64) -> Option<egui::Rect> {
    let mailbox = harness.state().mailbox();
    let row = mailbox.messages().iter().find(|row| row.id == id)?;
    harness.query_by_label(&mailbox.spoken(row)).map(|node| node.rect())
}

/// Done on the last row wholly in sight: the row the keyboard walks on to comes into sight by the least scroll that
/// shows it once the left row's room has closed. In the List view the next row slides up into the left row's place
/// and the list does not scroll at all (it scrolled 48 to 170 while the closing room was counted); in Grouped the
/// rule's row can stand in another group further down, and the list scrolls until it stands whole at the bottom, not
/// a row further.
#[test]
fn done_on_the_last_row_in_sight_scrolls_the_least_that_shows_the_next_row() {
    for (view, height) in [(InboxView::List, 640.0), (InboxView::List, 700.0), (InboxView::Grouped, 640.0), (InboxView::Grouped, 700.0)] {
        let mut harness = moving(view, height);
        harness.key_press(Key::Home);
        harness.run_steps(30);
        let sight = super::view_rect(&harness.ctx).unwrap();
        // J until the next row lies past the viewport's bottom: the keyed row is the last one whole in sight.
        let order = super::order(&harness.ctx);
        let mut at = 0;
        loop {
            let next = *order.get(at + 1).unwrap();
            if row(&harness, next).is_none_or(|rect| rect.bottom() > sight.bottom() + 0.01) {
                break;
            }
            harness.key_press(Key::J);
            harness.run_steps(30);
            at += 1;
        }
        let before = super::scrolled(&harness.ctx).unwrap();
        assert_eq!(harness.state().mailbox().selected(), order.get(at).copied(), "{view:?} {height}: the last row in sight is keyed");
        harness.key_press(Key::D);
        harness.run_steps(120);
        let scrolled = super::scrolled(&harness.ctx).unwrap() - before;
        let walked = harness.state().mailbox().selected().unwrap();
        let rect = row(&harness, walked).unwrap();
        let sight = super::view_rect(&harness.ctx).unwrap();
        assert!(sight.expand(0.01).contains_rect(rect), "{view:?} {height}: the walked-to row is in sight: {rect:?} in {sight:?}");
        assert!(
            scrolled.abs() < 0.01 || (rect.bottom() - sight.bottom()).abs() < 0.01,
            "{view:?} {height}: the list scrolled {scrolled}, past what shows the walked-to row: {rect:?} in {sight:?}"
        );
        if view == InboxView::List {
            assert!(scrolled.abs() < 0.01, "{view:?} {height}: the next row slides into the left row's place, the list stays: {scrolled}");
        }
    }
}

/// Turns the wheel over the list by `points` (down when positive) and lets the scrolling settle.
fn wheel(harness: &mut Harness<'static, InboxApp>, points: f32) {
    harness.hover_at(egui::pos2(400.0, 400.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -points),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(30);
}

/// Done on a keyed row the owner has scrolled partly out at the top, the list scrolled more than 50: the first message
/// in sight stays where it stands while the left row's room closes, so nothing in sight jumps down, not even for a
/// frame (it jumped by the cut in Done's pass);
/// the row walked on to is the one under the left row, whole in sight, and the keyboard is on it. In List and Grouped,
/// with motion and without.
#[test]
fn done_on_a_keyed_row_cut_at_the_top_never_jumps_the_list() {
    for view in [InboxView::List, InboxView::Grouped] {
        for motion in [true, false] {
            let at = format!("{view:?} motion {motion}");
            // A short window, so the sample's rows scroll well past the keyed one.
            let mut harness = moving(view, 440.0);
            harness.state_mut().set_reduce_motion(!motion);
            harness.key_press(Key::Home);
            harness.run_steps(30);
            // J to the fourth row in the list's order whose next row stands right under it.
            let order = super::order(&harness.ctx);
            let mut keyed = 0;
            for _ in 0..3 {
                harness.key_press(Key::J);
                harness.run_steps(30);
                keyed += 1;
            }
            while row(&harness, order[keyed + 1]).unwrap().top() != row(&harness, order[keyed]).unwrap().bottom() {
                harness.key_press(Key::J);
                harness.run_steps(30);
                keyed += 1;
            }
            let (left, next) = (order[keyed], order[keyed + 1]);
            assert_eq!(harness.state().mailbox().selected(), Some(left), "{at}: keyed");
            // The wheel until the keyed row stands 20 above the viewport's top.
            let sight = super::view_rect(&harness.ctx).unwrap();
            for _ in 0..8 {
                let rect = row(&harness, left).unwrap();
                let short = rect.top() - sight.top() + 20.0;
                if short.abs() < 0.01 {
                    break;
                }
                wheel(&mut harness, short);
            }
            let rect = row(&harness, left).unwrap();
            assert!(super::scrolled(&harness.ctx).unwrap() > 50.0, "{at}: the list scrolled more than 50");
            assert!((sight.top() - rect.top() - 20.0).abs() < 0.01, "{at}: the keyed row cut 20 at the top: {rect:?} in {sight:?}");
            assert_eq!(harness.state().mailbox().selected(), Some(left), "{at}: still keyed after the wheel");
            // Every row whole in sight under the keyed one, where it stands.
            let shown: Vec<(u64, egui::Rect)> = order
                .iter()
                .skip(keyed + 1)
                .filter_map(|id| row(&harness, *id).map(|rect| (*id, rect)))
                .filter(|(_, rect)| sight.contains_rect(*rect))
                .collect();
            assert!(shown.len() >= 3, "{at}: rows in sight under the keyed one");
            harness.key_press(Key::D);
            // The left row dissolves, then its room closes: the part of it above the viewport goes with the offset,
            // the part in sight closes there and the rows under it rise, never down, never in Done's own pass. With
            // motion off the row goes at once and the rows under it rise in that pass.
            let mut last = shown.clone();
            for pass in 0..60 {
                harness.run_steps(1);
                for (id, was) in &mut last {
                    let now = row(&harness, *id).unwrap();
                    assert!(now.top() <= was.top() + 0.01, "{at}: pass {pass} after Done, row {id} jumped down from {was:?} to {now:?}");
                    if pass == 0 && motion {
                        assert!((now.top() - was.top()).abs() < 0.01, "{at}: Done's own pass moved row {id} from {was:?} to {now:?}");
                    }
                    *was = now;
                }
            }
            harness.run_steps(60);
            assert_eq!(harness.state().mailbox().selected(), Some(next), "{at}: the row under the left one is walked on to");
            let sight = super::view_rect(&harness.ctx).unwrap();
            let walked = row(&harness, next).unwrap();
            assert!(sight.expand(0.01).contains_rect(walked), "{at}: the walked-to row whole in sight: {walked:?} in {sight:?}");
            let focus = harness.ctx.memory(|memory| memory.focused()).and_then(|id| harness.ctx.read_response(id)).map(|response| response.rect);
            let focus = focus.unwrap_or_else(|| panic!("{at}: the keyboard is on a row"));
            assert!(sight.expand(0.01).contains_rect(focus), "{at}: the keyboard on a row in sight: {focus:?} in {sight:?}");
        }
    }
}

/// Done on the last row wholly in sight, and the window made 40 shorter while the left row's room still closes: the
/// row walked on to, still sliding up into the left row's place, counts as whole in the last viewport, so the shorter
/// one brings it into sight, and it ends whole, not cut at the bottom. In List and Grouped (where the row walked on to
/// may stand in another group).
#[test]
fn a_window_made_shorter_while_done_closes_keeps_the_walked_to_row_whole() {
    for view in [InboxView::List, InboxView::Grouped] {
        let height = 640.0;
        let mut harness = moving(view, height);
        harness.key_press(Key::Home);
        harness.run_steps(30);
        let sight = super::view_rect(&harness.ctx).unwrap();
        // J until the next row lies past the viewport's bottom and stands right under the keyed one.
        let order = super::order(&harness.ctx);
        let mut at = 0;
        loop {
            let next = *order.get(at + 1).unwrap();
            let (keyed, below) = (row(&harness, order[at]), row(&harness, next));
            let under = keyed.zip(below).is_some_and(|(keyed, below)| (below.top() - keyed.bottom()).abs() < 0.01);
            if below.is_none_or(|rect| rect.bottom() > sight.bottom() + 0.01) && under {
                break;
            }
            harness.key_press(Key::J);
            harness.run_steps(30);
            at += 1;
        }
        let (left, next) = (order[at], order[at + 1]);
        let rect = row(&harness, left).unwrap();
        assert!(sight.bottom() - rect.bottom() < 40.0, "{view:?}: the keyed row ends less than 40 above the bottom: {rect:?} in {sight:?}");
        harness.key_press(Key::D);
        harness.run_steps(8);
        harness.set_size(vec2(1300.0, height - 40.0));
        harness.run_steps(120);
        // In Grouped the next-message rule may walk on to a row of another group.
        let walked = harness.state().mailbox().selected().unwrap();
        if view == InboxView::List {
            assert_eq!(walked, next, "{view:?}: the row under the left one is walked on to");
        }
        let sight = super::view_rect(&harness.ctx).unwrap();
        let walked = row(&harness, walked).unwrap();
        assert!(sight.expand(0.01).contains_rect(walked), "{view:?}: the walked-to row whole in the shorter list: {walked:?} in {sight:?}");
    }
}
