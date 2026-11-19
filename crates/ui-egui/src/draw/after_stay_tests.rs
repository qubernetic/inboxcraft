// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What follows an act, the owner's choice on Settings › Triage: `Stay` leaves the keyboard on the row that took the
//! open message's place and opens nothing (nothing read, the reader empty), and still never floats a docked composer.
//! The row on the page sets it through `after.set` only on the owner's press.

use egui::accesskit::Role;
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::{After, Mailbox};
use crate::settings::Section;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window, two panes, motion off.
fn window() -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    let mut harness = Harness::builder().with_size(vec2(1300.0, 820.0)).with_max_steps(40).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn seen(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    harness.state().mailbox().messages().iter().find(|row| row.id == id).is_some_and(|row| row.seen)
}

/// Opens the first row with an unread row below it by the owner's press; the open message and the row below it.
fn open_one(harness: &mut Harness<'static, InboxApp>) -> (u64, u64) {
    let order = super::list::order(&harness.ctx);
    let (open, below) = order
        .windows(2)
        .find_map(|pair| match pair {
            [open, below] if !seen(harness, *below) => Some((*open, *below)),
            _ => None,
        })
        .expect("the sample has an unread row under another");
    harness.state_mut().mailbox_mut().select(Some(open));
    for _ in 0..3 {
        harness.run();
    }
    assert_eq!(harness.state().reader().open_id(), Some(open));
    (open, below)
}

#[test]
fn stay_leaves_the_keyboard_on_the_next_row_and_opens_nothing() {
    let mut harness = window();
    harness.state_mut().mailbox_mut().set_after(After::Stay);
    assert_eq!(harness.state().mailbox().after(), After::Stay);
    let (open, below) = open_one(&mut harness);
    harness.key_press(Key::D);
    for _ in 0..4 {
        harness.run();
    }
    assert!(!harness.state().mailbox().messages().iter().any(|row| row.id == open), "done: it left the list");
    assert_eq!(harness.state().reader().open_id(), None, "nothing opens");
    assert_eq!(harness.state().mailbox().selected(), None, "nothing is selected");
    assert!(!seen(&harness, below), "the next row is not read");
    assert_eq!(super::list::focused_row(&harness.ctx), Some(below), "the keyboard is on the row that took its place");
}

#[test]
fn next_still_opens_the_next_row() {
    let mut harness = window();
    let (_, below) = open_one(&mut harness);
    harness.key_press(Key::D);
    for _ in 0..4 {
        harness.run();
    }
    assert_eq!(harness.state().reader().open_id(), Some(below), "the default walks on");
}

#[test]
fn stay_never_floats_a_docked_composer() {
    let mut harness = window();
    harness.state_mut().mailbox_mut().set_after(After::Stay);
    let (open, _) = open_one(&mut harness);
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    assert!(!harness.state().composer().unwrap().floating(), "docked in the reader's place");
    harness.state_mut().mailbox_mut().act(open, crate::mailbox::Act::Done);
    for _ in 0..4 {
        harness.run();
    }
    assert!(!harness.state().composer().unwrap().floating(), "nothing opened floats the docked composer");
    assert_eq!(harness.state().mailbox().selected(), None);
}

#[test]
fn the_triage_page_sets_what_follows_an_act_only_on_the_owner_s_press() {
    let mut harness = window();
    super::tests::at_settings(harness.state_mut(), Section::Triage);
    harness.run();
    let stay = harness.get_by_role_and_label(Role::RadioButton, "Stay for what follows an act");
    stay.hover();
    harness.run();
    harness.get_by_role_and_label(Role::RadioButton, "Stay for what follows an act").focus();
    harness.run();
    assert_eq!(harness.state().mailbox().after(), After::Next, "a hover or a focus changes nothing");
    harness.get_by_role_and_label(Role::RadioButton, "Stay for what follows an act").click();
    harness.run();
    assert_eq!(harness.state().mailbox().after(), After::Stay);
    harness.get_by_role_and_label(Role::RadioButton, "List for what follows an act").click();
    harness.run();
    assert_eq!(harness.state().mailbox().after(), After::List);
}

/// Presses D and lets the act and its landing settle.
fn done_by_key(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press(Key::D);
    for _ in 0..4 {
        harness.run();
    }
}

/// Under Stay, D three times in a row from `open`: each takes the row the keyboard is on, so three messages leave and
/// none opens, none is read, the keyboard lands each time on the row that took the last one's place.
fn three_in_a_row(harness: &mut Harness<'static, InboxApp>, open: u64) {
    let before = harness.state().mailbox().messages().len();
    let unread: Vec<u64> = harness.state().mailbox().messages().iter().filter(|row| !row.seen && row.id != open).map(|row| row.id).collect();
    let mut gone = vec![open];
    done_by_key(harness);
    for _ in 0..2 {
        let keyed = super::list::focused_row(&harness.ctx).expect("the keyboard is on a row after an act under Stay");
        assert!(!gone.contains(&keyed), "the keyboard is on a row still listed");
        done_by_key(harness);
        assert!(!harness.state().mailbox().messages().iter().any(|row| row.id == keyed), "D took the keyed row {keyed}");
        gone.push(keyed);
    }
    assert_eq!(harness.state().mailbox().messages().len(), before.saturating_sub(3), "three messages left the list");
    assert_eq!(harness.state().reader().open_id(), None, "nothing opens");
    assert_eq!(harness.state().mailbox().selected(), None, "nothing is selected");
    for id in unread.iter().filter(|id| harness.state().mailbox().messages().iter().any(|row| row.id == **id)) {
        assert!(!seen(harness, *id), "row {id} is not read");
    }
    let keyed = super::list::focused_row(&harness.ctx).expect("the keyboard is still on a row");
    assert!(!gone.contains(&keyed), "the keyboard is on a row still listed");
}

#[test]
fn stay_takes_three_messages_in_a_row_by_key_and_opens_none() {
    let mut harness = window();
    harness.state_mut().mailbox_mut().set_after(After::Stay);
    let (open, _) = open_one(&mut harness);
    three_in_a_row(&mut harness, open);
}

#[test]
fn stay_takes_three_in_a_row_from_the_list_s_last_row() {
    let mut harness = window();
    harness.state_mut().mailbox_mut().set_after(After::Stay);
    let last = *super::list::order(&harness.ctx).last().expect("the sample lists rows");
    harness.state_mut().mailbox_mut().select(Some(last));
    for _ in 0..3 {
        harness.run();
    }
    assert_eq!(harness.state().reader().open_id(), Some(last));
    // At the list's end each act lands on the row above, the new last row, which the next D takes.
    three_in_a_row(&mut harness, last);
    let order = super::list::order(&harness.ctx);
    assert_eq!(super::list::focused_row(&harness.ctx), order.last().copied(), "the keyboard is on the list's last row");
}
