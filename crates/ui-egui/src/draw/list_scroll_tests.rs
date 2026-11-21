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
