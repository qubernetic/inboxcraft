// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Done, park, pin, snooze and follow-up reminders in the window, on the sample: "Show done" and the park pill under
//! the header, "Later" from the rail with its piles, the time picker of Snooze and of "Remind me if no reply", Pin,
//! the More entries, the state chips under the subject, "Done all" on a group's card and the pin switch of the Triage
//! page.

use egui::accesskit::{Role, Toggled};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::Appearance;
use inboxcraft_model::Role as FolderRole;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::keymap;
use crate::mailbox::{Act, GroupKind, InboxView, Mailbox, Pile, Place};
use crate::sample;
use crate::settings::Section;
use crate::theme::install_fonts;

const ROTA: &str = "Allotment rota for November";

/// Thursday 5 November 2026, 09:00 UTC.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn app() -> InboxApp {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app.mailbox_mut().set_inbox_view(InboxView::Simple);
    app
}

fn harness_of(app: InboxApp) -> Harness<'static, InboxApp> {
    let mut harness =
        Harness::builder().with_size(vec2(1300.0, 900.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn id_of(app: &InboxApp, subject: &str) -> u64 {
    app.mailbox().messages().iter().find(|row| row.subject == subject).map(|row| row.id).unwrap()
}

/// The sample window with the rota open in the reader.
fn reading(subject: &str) -> Harness<'static, InboxApp> {
    let mut app = app();
    let id = id_of(&app, subject);
    app.mailbox_mut().select(Some(id));
    harness_of(app)
}

fn toggled(harness: &Harness<'static, InboxApp>, name: &str) -> Option<Toggled> {
    harness.get_by_role_and_label(Role::Button, name).accesskit_node().toggled()
}

fn settle(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..3 {
        harness.run();
    }
}

#[test]
fn show_done_lists_the_done_mail_dimmed_in_its_old_place() {
    let mut app = app();
    let rota = id_of(&app, ROTA);
    app.mailbox_mut().act(rota, Act::Done);
    let mut harness = harness_of(app);
    assert!(harness.query_by_label_contains("Allotment rota for November, 08:35").is_none(), "done left the inbox");
    assert_eq!(toggled(&harness, "Show done"), Some(Toggled::False));
    harness.get_by_role_and_label(Role::Button, "Show done").click();
    settle(&mut harness);
    assert!(harness.state().mailbox().show_done());
    assert_eq!(toggled(&harness, "Show done"), Some(Toggled::True));
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, done");
    // Done on a done row brings it back.
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, done").hover();
    settle(&mut harness);
    harness.get_by_role_and_label(Role::Button, "Not done").click();
    settle(&mut harness);
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35");
    assert_eq!(harness.state().mailbox().undoable().map(|undo| undo.text.as_str()), Some("Back in its place"));
}

#[test]
fn with_show_done_the_d_key_leaves_the_row_in_place_and_dims_it() {
    let mut app = app();
    app.mailbox_mut().set_show_done(true);
    let rota = id_of(&app, ROTA);
    app.mailbox_mut().select(Some(rota));
    let mut harness = harness_of(app);
    harness.state_mut().run_action(&egui::Context::default(), keymap::DONE);
    settle(&mut harness);
    assert!(harness.state().mailbox().leaving().is_none(), "no slide: the row stays");
    assert!(harness.state().mailbox().messages().iter().any(|row| row.id == rota && row.done));
    harness.state_mut().run_action(&egui::Context::default(), keymap::DONE);
    assert!(harness.state().mailbox().messages().iter().any(|row| row.id == rota && !row.done), "D again: not done");
}

#[test]
fn the_reader_s_done_reads_not_done_on_a_done_message_and_brings_it_back() {
    let mut app = app();
    app.mailbox_mut().set_show_done(true);
    let rota = id_of(&app, ROTA);
    app.mailbox_mut().select(Some(rota));
    app.mailbox_mut().act(rota, Act::Done);
    let mut harness = harness_of(app);
    settle(&mut harness);
    assert!(harness.query_by_role_and_label(Role::Button, "Done").is_none(), "no Done on a done message");
    harness.get_by_role_and_label(Role::Button, "Not done").click();
    settle(&mut harness);
    assert!(harness.state().mailbox().messages().iter().any(|row| row.id == rota && !row.done));
    assert_eq!(harness.state().mailbox().undoable().map(|undo| undo.text.as_str()), Some("Back in its place"));
    harness.get_by_role_and_label(Role::Button, "Done");
}

#[test]
fn the_park_pill_shows_while_mail_is_parked_and_opens_the_pile() {
    let mut app = app();
    let rota = id_of(&app, ROTA);
    app.mailbox_mut().act(rota, Act::Park(true));
    let mut harness = harness_of(app);
    harness.get_by_role_and_label(Role::Button, "Parked (1)").click();
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().place(), Place::Later(Pile::Parked));
    assert_eq!(harness.state().mailbox().title(), "Later");
    assert_eq!(toggled(&harness, "Parked (1)"), Some(Toggled::True));
    assert_eq!(toggled(&harness, "Pins (0)"), Some(Toggled::False));
    harness.get_by_label_contains("Allotment rota for November");
    harness.get_by_role_and_label(Role::Button, "Snoozed (0)").click();
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().place(), Place::Later(Pile::Snoozed));
    harness.get_by_label("Snoozed mail waits here until its time comes.");
}

#[test]
fn the_rail_s_later_runs_as_an_action_too() {
    let mut harness = harness_of(app());
    harness.state_mut().run_action(&egui::Context::default(), keymap::LATER);
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().place(), Place::Later(Pile::Pins));
    harness.get_by_label("Pinned mail stays here, wherever it is filed.");
    harness.get_by_role_and_label(Role::Button, "Parked (0)").click();
    settle(&mut harness);
    harness.state_mut().run_action(&egui::Context::default(), keymap::LATER);
    assert_eq!(harness.state().mailbox().place(), Place::Later(Pile::Parked), "Later again stays on the pile shown");
    harness.get_by_role_and_label(Role::Button, "Parked (0)").click();
    settle(&mut harness);
    harness.get_by_label("Parked mail waits here, out of the inbox, without a time.");
}

#[test]
fn snooze_opens_the_time_picker_and_a_typed_time_snoozes_the_message() {
    let mut harness = reading(ROTA);
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    settle(&mut harness);
    harness.get_by_label("Snooze until");
    for preset in ["Later today", "This evening", "Tomorrow morning"] {
        harness.get_by_label_contains(preset);
    }
    let field = harness.get_by_role_and_label(Role::TextInput, "Or type a time");
    field.focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Or type a time").type_text("sat 9");
    settle(&mut harness);
    harness.get_by_label("Saturday 7 November, 09:00");
    let snooze = harness.query_all_by_role_and_label(Role::Button, "Snooze").last().unwrap();
    snooze.click();
    settle(&mut harness);
    let app = harness.state();
    assert_eq!(app.mailbox().undoable().map(|undo| undo.text.as_str()), Some("Snoozed until Saturday 7 November, 09:00"));
    assert!(app.mailbox().messages().iter().all(|row| row.subject != ROTA), "it left the inbox");
    assert!(harness.query_by_label("Snooze until").is_none(), "the picker closed");
}

#[test]
fn a_preset_snoozes_at_once_and_cancel_snoozes_nothing() {
    let mut harness = reading(ROTA);
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    settle(&mut harness);
    harness.get_by_role_and_label(Role::Button, "Cancel").click();
    settle(&mut harness);
    assert!(harness.query_by_label("Snooze until").is_none());
    assert!(harness.state().mailbox().undoable().is_none());
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    settle(&mut harness);
    harness.get_by_label("Tomorrow morning, Fri 08:00").click();
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().undoable().map(|undo| undo.text.as_str()), Some("Snoozed until Friday 6 November, 08:00"));
}

#[test]
fn a_snoozed_message_shows_its_time_and_comes_back_on_request() {
    let mut app = app();
    let rota = id_of(&app, ROTA);
    app.mailbox_mut().act(rota, Act::Snooze(Timestamp::from_unix(NOW + 2 * 86_400, 0).unwrap()));
    app.mailbox_mut().open(Place::Later(Pile::Snoozed));
    app.mailbox_mut().select(Some(rota));
    app.mailbox_mut().forget_undo();
    let mut harness = harness_of(app);
    harness.get_by_label("Snoozed until Saturday 7 November, 09:00");
    harness.get_by_label_contains("Allotment rota for November, Sat 09:00");
    harness.get_by_role_and_label(Role::Button, "More").click();
    settle(&mut harness);
    harness.get_by_label("Bring back now").click();
    settle(&mut harness);
    let app = harness.state();
    assert_eq!(app.mailbox().undoable().map(|undo| undo.text.as_str()), Some("Back in the inbox"));
    assert!(app.mailbox().messages().is_empty(), "the snoozed pile is empty again");
}

#[test]
fn a_returned_message_says_so_in_the_list_and_the_reader() {
    let mut app = app();
    let rota = id_of(&app, ROTA);
    app.mailbox_mut().act(rota, Act::Snooze(Timestamp::from_unix(NOW + 3_600, 0).unwrap()));
    app.mailbox_mut().open(Place::Later(Pile::Snoozed));
    app.mailbox_mut().act(rota, Act::Unsnooze);
    app.mailbox_mut().open(Place::View(FolderRole::Inbox));
    let mut harness = harness_of(app);
    let row = harness.get_by_label_contains("Allotment rota for November, 08:35, unread, returned");
    row.click();
    settle(&mut harness);
    // Opened, it is read: the mark goes.
    assert!(harness.query_by_label("Back from a snooze").is_none());
}

#[test]
fn pin_pins_the_open_message_and_the_list_shows_the_pin() {
    let mut harness = reading(ROTA);
    assert_eq!(toggled(&harness, "Pin"), Some(Toggled::False));
    harness.get_by_role_and_label(Role::Button, "Pin").click();
    settle(&mut harness);
    assert_eq!(toggled(&harness, "Take the pin off"), Some(Toggled::True));
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, pinned");
    assert!(super::tests::drew_icon(harness.output(), crate::iconset::IconId::Pin, crate::iconset::Style::Fill));
    harness.get_by_role_and_label(Role::Button, "Take the pin off").click();
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().undoable().map(|undo| undo.text.as_str()), Some("Pin taken off"));
}

#[test]
fn more_parks_the_open_message() {
    let mut harness = reading(ROTA);
    harness.get_by_role_and_label(Role::Button, "More").click();
    settle(&mut harness);
    harness.get_by_label("Park").click();
    settle(&mut harness);
    // Out once the slide is over.
    settle(&mut harness);
    let app = harness.state();
    assert_eq!(app.mailbox().undoable().map(|undo| undo.text.as_str()), Some("Parked"));
    assert_eq!(app.mailbox().parked(), 1);
}

#[test]
fn sent_mail_gets_a_reminder_through_the_same_picker() {
    let mut app = app();
    app.mailbox_mut().open(Place::View(FolderRole::Sent));
    let plans = id_of(&app, "Weekend plans");
    app.mailbox_mut().select(Some(plans));
    let mut harness = harness_of(app);
    assert!(harness.query_by_role_and_label(Role::Button, "Snooze").is_none());
    harness.get_by_role_and_label(Role::Button, "Remind me if no reply").click();
    settle(&mut harness);
    harness.get_by_label("Remind me if no reply by");
    harness.get_by_role_and_label(Role::TextInput, "Or type a time").focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Or type a time").type_text("mon 9");
    settle(&mut harness);
    harness.get_by_role_and_label(Role::Button, "Remind me").click();
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().undoable().map(|undo| undo.text.as_str()), Some("Reminder set for Monday 9 November, 09:00"));
    harness.get_by_label("Reminder if no reply by Monday 9 November, 09:00");
    harness.get_by_role_and_label(Role::Button, "More").click();
    settle(&mut harness);
    harness.get_by_label("Take the reminder off").click();
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().undoable().map(|undo| undo.text.as_str()), Some("Reminder taken off"));
}

#[test]
fn done_all_on_a_group_card_files_the_group() {
    let mut app = app();
    app.mailbox_mut().set_inbox_view(InboxView::Cards);
    let updates: Vec<u64> = app.mailbox().messages().iter().filter(|row| row.group == Some(GroupKind::Updates)).map(|row| row.id).collect();
    let mut harness = harness_of(app);
    // People's card comes first, Updates' second.
    let done_all = harness.query_all_by_role_and_label(Role::Button, "Done all").nth(1).unwrap();
    done_all.click();
    settle(&mut harness);
    let app = harness.state();
    assert_eq!(app.mailbox().undoable().map(|undo| undo.text.as_str()), Some("Marked 3 as done"));
    assert!(app.mailbox().messages().iter().all(|row| !updates.contains(&row.id)));
}

#[test]
fn the_triage_page_keeps_pins_on_done_mail_by_a_switch() {
    let mut app = app();
    app.settings_mut().open_at(Section::Triage, &Appearance::default());
    let mut harness = harness_of(app);
    let node = harness.get_by_role_and_label(Role::Switch, "Keep pins on done mail");
    assert_eq!(node.accesskit_node().toggled(), Some(Toggled::False));
    node.click();
    settle(&mut harness);
    assert!(harness.state().mailbox().pins_outlast_done());
    assert_eq!(harness.get_by_role_and_label(Role::Switch, "Keep pins on done mail").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn the_snooze_key_opens_the_picker_over_the_open_message_only() {
    let mut harness = harness_of(app());
    assert!(!harness.state().can_run(keymap::SNOOZE), "no message open");
    harness.key_press(egui::Key::H);
    settle(&mut harness);
    assert!(harness.query_by_label("Snooze until").is_none());
    let mut harness = reading(ROTA);
    assert!(harness.state().can_run(keymap::SNOOZE));
    harness.key_press(egui::Key::H);
    settle(&mut harness);
    harness.get_by_label("Snooze until");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    assert!(harness.query_by_label("Snooze until").is_none(), "Escape closes it");
}
