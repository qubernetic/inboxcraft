// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor on the calendar screen: New event, Edit and Delete, the form's controls, the notify-guests prompt
//! only for an event with guests, and the toast whose Undo (or Ctrl+Z) takes a change back.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _, by};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window on the calendar screen.
fn on_calendar(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    press(&mut harness, "Calendar");
    harness
}

fn press(harness: &mut Harness<'static, InboxApp>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

/// Presses the first event whose name holds `text`.
fn pick(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.query_all(by().role(Role::Button).label_contains(text)).next().unwrap().click();
    harness.run();
}

fn shows(harness: &Harness<'static, InboxApp>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

fn editing(harness: &Harness<'static, InboxApp>) -> bool {
    harness.state().calendar().editor().is_some()
}

fn type_into(harness: &mut Harness<'static, InboxApp>, field: &str, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, field).focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, field).type_text(text);
    harness.run();
}

#[test]
fn new_event_opens_the_editor_and_saving_adds_the_event_with_an_undo_toast() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        press(&mut harness, "New event");
        assert!(editing(&harness));
        for field in ["Title", "Starts", "Ends", "Guests"] {
            harness.get_by_role_and_label(Role::TextInput, field);
        }
        assert!(!shows(&harness, "Tell the"), "a new event asks nothing of its guests");
        harness.check_accessibility();
        type_into(&mut harness, "Title", "Pottery class");
        type_into(&mut harness, "Guests", "kai@example.org");
        assert!(!shows(&harness, "Tell the"), "not even with guests typed");
        press(&mut harness, "Save");
        assert!(!editing(&harness), "the editor closes");
        assert!(shows(&harness, "Pottery class, Thursday 5 November, 09:00 – 10:00"));
        harness.get_by_label("Event added");
        harness.check_accessibility();
        press(&mut harness, "Undo");
        assert!(!shows(&harness, "Pottery class"), "undo takes the new event away");
    }
}

#[test]
fn the_notify_prompt_shows_only_when_the_event_has_guests() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        pick(&mut harness, "Dentist");
        press(&mut harness, "Edit");
        assert!(editing(&harness));
        assert!(!shows(&harness, "Tell the"), "the dentist has no guests");
        press(&mut harness, "Cancel");
        assert!(!editing(&harness));
        pick(&mut harness, "Allotment work party");
        press(&mut harness, "Edit");
        harness.get_by_label("Tell the 1 guest about this change?");
        let quiet = harness.get_by_role_and_label(Role::RadioButton, "Don't tell them");
        assert_eq!(quiet.accesskit_node().toggled(), Some(Toggled::True), "the default is not to tell them");
        harness.check_accessibility();
        harness.get_by_role_and_label(Role::RadioButton, "Send them the update").click();
        harness.run();
        assert!(harness.state().calendar().editor().is_some_and(|editor| editor.notify()));
        harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().guests.clear();
        harness.run();
        assert!(!shows(&harness, "Tell the"), "without guests the prompt goes");
    }
}

#[test]
fn an_edit_with_its_guests_told_says_so_in_the_toast() {
    let mut harness = on_calendar(egui::Theme::Light);
    pick(&mut harness, "Allotment work party");
    press(&mut harness, "Edit");
    type_into(&mut harness, "Title", " and soup");
    harness.get_by_role_and_label(Role::RadioButton, "Send them the update").click();
    harness.run();
    press(&mut harness, "Save");
    harness.get_by_label("Event saved; the guests are told");
    assert!(shows(&harness, "Allotment work party and soup"));
}

#[test]
fn the_form_s_pills_and_boxes_change_the_event() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    harness.get_by_role_and_label(Role::Button, "Studio").click();
    harness.run();
    for name in ["Weekly", "Free"] {
        press(&mut harness, name);
    }
    for name in ["All day", "Private"] {
        harness.get_by_role_and_label(Role::CheckBox, name).click();
        harness.run();
    }
    let form = harness.state().calendar().editor().unwrap().form().clone();
    assert_eq!((form.calendar, form.repeat.as_str(), form.busy, form.all_day, form.private), (2, "weekly", false, true, true));
    assert_eq!(form.start, "2026-11-05");
    harness.get_by_role_and_label(Role::CheckBox, "All day").click();
    harness.run();
    assert_eq!(harness.state().calendar().editor().unwrap().form().start, "2026-11-05 09:00");
    press(&mut harness, "Busy");
    press(&mut harness, "Never");
    let form = harness.state().calendar().editor().unwrap().form().clone();
    assert_eq!((form.repeat.as_str(), form.busy), ("none", true));
}

#[test]
fn a_custom_repeat_shows_as_custom_and_cannot_be_picked() {
    let mut harness = on_calendar(egui::Theme::Light);
    pick(&mut harness, "Dentist");
    press(&mut harness, "Edit");
    assert!(harness.query_by_label("Custom").is_none(), "only a repeat the presets do not name shows Custom");
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().repeat = "custom".to_owned();
    harness.run();
    press(&mut harness, "Custom");
    assert_eq!(harness.state().calendar().editor().unwrap().form().repeat, "custom");
    assert!(harness.query_by_label("Studio").is_some_and(|node| node.accesskit_node().role() == Role::CheckBox), "an edit picks no calendar");
}

#[test]
fn what_cannot_be_saved_is_said_in_the_editor() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().start = "soon".to_owned();
    harness.run();
    press(&mut harness, "Save");
    assert!(editing(&harness));
    harness.get_by_label("Starts is no date and time like 2026-11-06 13:00");
    harness.check_accessibility();
}

#[test]
fn escape_closes_the_editor_and_leaves_the_calendar() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!editing(&harness));
    harness.get_by_label("2 – 8 November 2026");
}

#[test]
fn delete_shows_a_toast_and_ctrl_z_brings_the_event_back() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        pick(&mut harness, "Dentist");
        press(&mut harness, "Delete");
        assert!(!shows(&harness, "Dentist"));
        harness.get_by_label("Event deleted");
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        harness.run();
        assert!(shows(&harness, "Dentist"), "the toast's key brings it back on the calendar screen");
    }
}

#[test]
fn after_the_toast_ctrl_z_still_takes_an_event_s_change_back() {
    let mut harness = on_calendar(egui::Theme::Light);
    pick(&mut harness, "Dentist");
    press(&mut harness, "Delete");
    harness.state_mut().mailbox_mut().forget_undo();
    harness.run();
    assert!(harness.query_by_label("Event deleted").is_none());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(shows(&harness, "Dentist"), "the history's Undo reads the calendar again");
}
