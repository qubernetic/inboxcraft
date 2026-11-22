// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! "Unread only" in the list's filters' line, on the sample: on, the place lists its unread messages alone; a row read
//! meanwhile leaves at once, but the open message stays open (its row until the owner moves on, and the next message
//! is the next unread one); the choice is each place's own for the session; an empty state says nothing is unread;
//! its key, its palette row and `ui.list.unread` of an agent turn it as the pill does.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::Role as FolderRole;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::super::InboxApp;
use crate::keymap;
use crate::mailbox::{Act, InboxView, Mailbox, Pile, Place};
use crate::sample;
use crate::theme::install_fonts;

/// Thursday 5 November 2026, 09:00 UTC.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn app() -> InboxApp {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app.mailbox_mut().set_inbox_view(InboxView::List);
    app
}

fn harness_of(app: InboxApp) -> Harness<'static, InboxApp> {
    let mut harness =
        Harness::builder().with_size(vec2(1300.0, 900.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn settle(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..4 {
        harness.run();
    }
}

fn ids(mailbox: &Mailbox) -> Vec<u64> {
    mailbox.messages().iter().map(|row| row.id).collect()
}

fn unread(mailbox: &Mailbox) -> Vec<u64> {
    mailbox.messages().iter().filter(|row| !row.seen).map(|row| row.id).collect()
}

fn listed(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    harness.state().mailbox().messages().iter().any(|row| row.id == id)
}

fn seen(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    harness.state().mailbox().messages().iter().any(|row| row.id == id && row.seen)
}

fn toggled(harness: &Harness<'static, InboxApp>) -> Option<Toggled> {
    harness.get_by_role_and_label(Role::Button, "Unread only").accesskit_node().toggled()
}

#[test]
fn on_lists_the_unread_messages_alone_and_off_every_one_again() {
    let mut app = app();
    let all = ids(app.mailbox());
    let left = unread(app.mailbox());
    assert!(!left.is_empty() && left.len() < all.len(), "the sample's inbox holds read and unread mail");
    app.mailbox_mut().set_unread_only(true);
    assert!(app.mailbox().unread_only());
    assert_eq!(ids(app.mailbox()), left, "on: the unread ones alone, in their order");
    app.mailbox_mut().set_unread_only(false);
    assert_eq!(ids(app.mailbox()), all, "off: as before");
}

#[test]
fn the_pill_turns_it_on_and_off_and_says_which() {
    let mut harness = harness_of(app());
    assert_eq!(toggled(&harness), Some(Toggled::False));
    harness.get_by_role_and_label(Role::Button, "Unread only").click();
    settle(&mut harness);
    assert!(harness.state().mailbox().unread_only(), "the press turns it on");
    assert_eq!(toggled(&harness), Some(Toggled::True));
    assert!(harness.state().mailbox().messages().iter().all(|row| !row.seen));
    harness.get_by_role_and_label(Role::Button, "Unread only").click();
    settle(&mut harness);
    assert!(!harness.state().mailbox().unread_only(), "pressed again, off");
}

#[test]
fn a_row_marked_read_leaves_the_list_at_once() {
    let mut app = app();
    app.mailbox_mut().set_unread_only(true);
    let first = ids(app.mailbox())[0];
    app.mailbox_mut().start(first, Act::Read);
    assert!(!app.mailbox().messages().iter().any(|row| row.id == first), "read, it leaves");
    assert_eq!(app.mailbox().leaving().map(|(id, _)| id), Some(first), "it slides out as a done row does");
    app.mailbox_mut().refresh();
    assert!(!app.mailbox().messages().iter().any(|row| row.id == first), "and stays out");
}

#[test]
fn a_message_read_elsewhere_leaves_at_the_next_reading_of_the_list() {
    let mut app = app();
    app.mailbox_mut().set_unread_only(true);
    let first = ids(app.mailbox())[0];
    // Marked read under the list, as a sync from another device would.
    app.mailbox_mut().run("mail.seen", &json!({ "id": first, "seen": true })).unwrap();
    app.mailbox_mut().refresh();
    assert!(!app.mailbox().messages().iter().any(|row| row.id == first));
}

/// The window with "Unread only" on and the first unread message opened by the owner's press: its id and the next.
fn reading() -> (Harness<'static, InboxApp>, u64, u64) {
    let mut app = app();
    app.mailbox_mut().set_unread_only(true);
    let (first, next) = match ids(app.mailbox())[..] {
        [first, next, ..] => (first, next),
        _ => panic!("the sample has two unread messages"),
    };
    app.mailbox_mut().select(Some(first));
    let mut harness = harness_of(app);
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(first));
    (harness, first, next)
}

#[test]
fn the_open_message_stays_open_once_it_is_read() {
    let (mut harness, first, _) = reading();
    assert!(seen(&harness, first), "opening it read it");
    assert!(listed(&harness, first), "its row stays while it is open");
    harness.state_mut().mailbox_mut().refresh();
    settle(&mut harness);
    assert!(listed(&harness, first), "a fresh reading of the list keeps it too");
    assert_eq!(harness.state().reader().open_id(), Some(first), "the reader keeps it");
    assert_eq!(harness.state().mailbox().selected(), Some(first));
}

#[test]
fn the_next_message_is_the_next_unread_one_and_the_read_row_goes() {
    let (mut harness, first, next) = reading();
    harness.state_mut().mailbox_mut().step(true);
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), Some(next), "the next unread one");
    assert_eq!(harness.state().reader().open_id(), Some(next));
    assert!(!listed(&harness, first), "the read one left as the owner moved on");
}

#[test]
fn closing_the_reader_lets_the_read_row_go() {
    let (mut harness, first, _) = reading();
    harness.state_mut().mailbox_mut().select(None);
    settle(&mut harness);
    assert!(!listed(&harness, first));
}

#[test]
fn each_place_keeps_its_own_choice() {
    let mut app = app();
    app.mailbox_mut().set_unread_only(true);
    app.mailbox_mut().open(Place::View(FolderRole::Sent));
    assert!(!app.mailbox().unread_only(), "Sent has its own, off");
    app.mailbox_mut().set_unread_only(true);
    app.mailbox_mut().open(Place::View(FolderRole::Archive));
    assert!(!app.mailbox().unread_only(), "the archive too");
    app.mailbox_mut().open(Place::View(FolderRole::Inbox));
    assert!(app.mailbox().unread_only(), "the inbox kept its own");
    assert!(app.mailbox().messages().iter().all(|row| !row.seen));
    app.mailbox_mut().open(Place::View(FolderRole::Sent));
    assert!(app.mailbox().unread_only(), "and Sent its own");
}

#[test]
fn places_without_it_have_no_pill_and_keep_their_rows() {
    let mut app = app();
    app.mailbox_mut().open(Place::Senders);
    let before = ids(app.mailbox());
    app.mailbox_mut().set_unread_only(true);
    assert!(!app.mailbox().can_unread_only() && !app.mailbox().unread_only());
    assert_eq!(ids(app.mailbox()), before);
    let harness = harness_of(app);
    assert!(harness.query_by_role_and_label(Role::Button, "Unread only").is_none());
}

#[test]
fn nothing_unread_says_so_in_place_of_a_blank_list() {
    let mut app = app();
    // The inbox shows the senders waiting aside in place of an empty state: Sent shows its own.
    app.mailbox_mut().open(Place::View(FolderRole::Sent));
    for id in unread(app.mailbox()) {
        app.mailbox_mut().run("mail.seen", &json!({ "id": id, "seen": true })).unwrap();
    }
    app.mailbox_mut().set_unread_only(true);
    assert!(app.mailbox().messages().is_empty());
    let harness = harness_of(app);
    harness.get_by_label("Nothing unread here.");
}

#[test]
fn its_key_turns_it_and_the_palette_lists_it() {
    let mut harness = harness_of(app());
    harness.key_press(Key::U);
    settle(&mut harness);
    assert!(harness.state().mailbox().unread_only(), "U turns it on");
    harness.key_press(Key::U);
    settle(&mut harness);
    assert!(!harness.state().mailbox().unread_only(), "and off");
    assert!(crate::palette::rows("unread only").iter().any(|row| row.id == keymap::UNREAD));
}

/// What the app does after a request of the control channel: the asks followed, an agent's at the frame's end.
fn follow(harness: &mut Harness<'static, InboxApp>) {
    harness.state_mut().follow_settings_asked();
    harness.run();
    harness.state_mut().act_on_agent_ask(false);
    settle(harness);
}

#[test]
fn an_agent_s_ui_list_unread_turns_the_place_shown() {
    let mut harness = harness_of(app());
    harness.state_mut().mailbox_mut().run_for_agent("ui.list.unread", &json!({ "on": true })).unwrap();
    follow(&mut harness);
    assert!(harness.state().mailbox().unread_only(), "on, as asked");
    harness.state_mut().mailbox_mut().run_for_agent("ui.list.unread", &json!({})).unwrap();
    follow(&mut harness);
    assert!(!harness.state().mailbox().unread_only(), "no `on`: the other of the two");
}

#[test]
fn undo_of_an_act_on_the_open_message_brings_it_back_open() {
    for (key, act) in [(Key::D, "done"), (Key::A, "archive"), (Key::Delete, "delete")] {
        let (mut harness, first, next) = reading();
        harness.key_press(key);
        settle(&mut harness);
        assert!(!listed(&harness, first), "{act}: it left");
        assert_eq!(harness.state().mailbox().selected(), Some(next), "{act}: the rule walked on");
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, keymap::UNDO);
        settle(&mut harness);
        assert!(listed(&harness, first), "{act}: Undo lists it again, read, in its place");
        assert_eq!(harness.state().mailbox().selected(), Some(first), "{act}: selected again");
        assert_eq!(harness.state().reader().open_id(), Some(first), "{act}: open again");
    }
}

/// An agent's `ui.list.unread` with `params`, held for the frame's end as the app holds it.
fn agent_holds(harness: &mut Harness<'static, InboxApp>, params: serde_json::Value) {
    harness.state_mut().mailbox_mut().run_for_agent("ui.list.unread", &params).unwrap();
    harness.state_mut().follow_settings_asked();
}

#[test]
fn an_agent_s_ask_waits_for_the_frame_s_end_and_yields_to_the_owner() {
    let mut harness = harness_of(app());
    agent_holds(&mut harness, json!({ "on": true }));
    assert!(harness.state().holds_agent_ask(), "held, not applied at once");
    assert!(!harness.state().mailbox().unread_only(), "the list as it was until the frame's end");
    // The owner's own key or press changed something in this frame: the agent's ask goes.
    harness.state_mut().act_on_agent_ask(true);
    settle(&mut harness);
    assert!(!harness.state().mailbox().unread_only(), "the owner acted: the ask is dropped");
    assert!(!harness.state().holds_agent_ask());
}

#[test]
fn the_owner_s_own_turn_in_the_frame_wins_over_an_agent_s_ask() {
    let mut harness = harness_of(app());
    agent_holds(&mut harness, json!({}));
    harness.key_press(Key::U);
    harness.run();
    assert!(harness.state().mailbox().unread_only(), "the owner's U turned it on");
    // The app's frame end: nothing of the settings or the place changed, but the owner turned the filter.
    harness.state_mut().act_on_agent_ask(false);
    settle(&mut harness);
    assert!(harness.state().mailbox().unread_only(), "the agent's toggle does not undo the owner's key");
}

#[test]
fn an_agent_s_ask_passes_over_the_command_line_s_results() {
    let mut harness = harness_of(app());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().command().results());
    agent_holds(&mut harness, json!({ "on": true }));
    harness.state_mut().act_on_agent_ask(false);
    harness.run();
    assert!(!harness.state().mailbox().unread_only(), "the owner holds the keys: passed over");
    assert!(harness.state().command().results(), "and the results stay");
}

#[test]
fn while_a_search_shows_neither_the_key_nor_an_agent_turns_it() {
    let mut harness = harness_of(app());
    harness.state_mut().search("Forgehub");
    settle(&mut harness);
    assert!(!harness.state().mailbox().query().is_empty());
    assert!(!harness.state().can_run(keymap::UNREAD), "the key rests and the palette offers it unavailable");
    harness.key_press(Key::U);
    settle(&mut harness);
    agent_holds(&mut harness, json!({ "on": true }));
    harness.state_mut().act_on_agent_ask(false);
    settle(&mut harness);
    harness.state_mut().search("");
    settle(&mut harness);
    assert!(!harness.state().mailbox().unread_only(), "nothing turned it behind the search");
}

#[test]
fn an_agent_s_ask_on_the_settings_leaves_the_hidden_list_alone() {
    let mut harness = harness_of(app());
    harness.state_mut().open_settings_on("appearance");
    settle(&mut harness);
    agent_holds(&mut harness, json!({ "on": true }));
    harness.state_mut().act_on_agent_ask(false);
    settle(&mut harness);
    harness.state_mut().leave_settings();
    settle(&mut harness);
    assert!(!harness.state().mailbox().unread_only(), "the list the owner did not see is as it was");
}

#[test]
fn places_without_it_refuse_the_key_and_the_palette() {
    // The outbox's own: `draw::outbox_tests`, which can send.
    for place in [Place::Senders, Place::Later(Pile::Pins), Place::View(FolderRole::Drafts)] {
        let mut app = app();
        app.mailbox_mut().open(place);
        assert_eq!(app.mailbox().place(), place, "the sample shows {place:?}");
        let mut harness = harness_of(app);
        assert!(!harness.state().can_run(keymap::UNREAD), "{place:?}: not here");
        harness.key_press(Key::U);
        settle(&mut harness);
        assert!(!harness.state().mailbox().unread_only(), "{place:?}");
    }
}
