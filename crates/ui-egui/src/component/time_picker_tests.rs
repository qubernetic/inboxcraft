// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::Key;
use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use super::{Ask, Picked, Preset, TimePicker, show};
use crate::component::tests::{builder, theme};
use crate::motion;

/// A UTC time.
fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).and_then(|civil| civil.to_timestamp(Offset::UTC)).unwrap_or(Timestamp::MIN)
}

/// Friday 6 November 2026, 14:30 UTC.
fn friday() -> Timestamp {
    at(2026, 11, 6, 14, 30)
}

const PRESETS: [(&str, [u8; 3]); 4] =
    [("Later today", [6, 17, 30]), ("This morning", [6, 8, 0]), ("Tomorrow morning", [7, 8, 0]), ("Next week", [9, 8, 0])];

/// What the harness keeps: the picker, the time it is, whether it asks about an alert, and what was picked.
struct Probe {
    picker: TimePicker,
    now: Timestamp,
    alert: bool,
    picked: Vec<Picked>,
}

fn harness(alert: bool) -> Harness<'static, Probe> {
    let theme = theme();
    let mut harness = builder().with_size(egui::vec2(360.0, 420.0)).build_ui_state(
        move |ui, probe: &mut Probe| {
            motion::set_reduced(ui.ctx(), true);
            let presets: Vec<Preset<'_>> =
                PRESETS.iter().map(|(name, [day, hour, minute])| Preset { name, at: at(2026, 11, *day, *hour, *minute) }).collect();
            let ask =
                Ask { title: "Snooze until", confirm: "Snooze", alert: probe.alert.then_some("Notify me when it comes back"), presets: &presets };
            match show(ui, &theme, &mut probe.picker, &ask, probe.now, Offset::UTC) {
                Picked::Open => {}
                picked => probe.picked.push(picked),
            }
        },
        Probe { picker: TimePicker::default(), now: friday(), alert, picked: Vec::new() },
    );
    harness.run();
    harness
}

fn typed(harness: &mut Harness<'static, Probe>, text: &str) {
    let field = harness.get_by_role_and_label(Role::TextInput, "Or type a time");
    field.focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Or type a time").type_text(text);
    harness.run();
}

#[test]
fn the_presets_ahead_show_with_their_time_and_past_ones_are_hidden() {
    let harness = harness(true);
    assert!(harness.query_by_label("Snooze until").is_some());
    let later = harness.get_by_label("Later today, 17:30");
    assert_eq!(later.accesskit_node().role(), Role::MenuItem);
    assert!(harness.query_by_label("Tomorrow morning, Sat 08:00").is_some());
    assert!(harness.query_by_label("Next week, Mon 08:00").is_some());
    assert!(harness.query_by_label_contains("This morning").is_none(), "08:00 today is behind");
}

#[test]
fn a_preset_is_chosen_with_one_click_and_its_time_is_the_one_shown() {
    let mut harness = harness(false);
    harness.get_by_label("Tomorrow morning, Sat 08:00").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 7, 8, 0), alert: false }]);
}

#[test]
fn a_typed_time_is_used_without_enter() {
    let mut harness = harness(true);
    typed(&mut harness, "thu 9");
    assert!(harness.query_by_label("Thursday 12 November, 09:00").is_some(), "the parsed time shows as it is typed");
    assert!(harness.state().picked.is_empty(), "typing alone picks nothing");
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 12, 9, 0), alert: false }]);
}

#[test]
fn enter_takes_the_typed_time_too() {
    let mut harness = harness(false);
    typed(&mut harness, "tomorrow 9:30");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 7, 9, 30), alert: false }]);
}

#[test]
fn the_shown_time_follows_the_clock_and_is_the_one_used() {
    let mut harness = harness(false);
    typed(&mut harness, "17:30");
    assert!(harness.query_by_label("Friday 6 November, 17:30").is_some());
    harness.state_mut().now = at(2026, 11, 6, 17, 45);
    harness.run();
    assert!(harness.query_by_label("Friday 6 November, 17:30").is_none(), "a time behind is never shown");
    assert!(harness.query_by_label("Saturday 7 November, 17:30").is_some());
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 7, 17, 30), alert: false }]);
}

#[test]
fn text_that_is_no_time_says_so_and_cannot_be_confirmed() {
    let mut harness = harness(false);
    typed(&mut harness, "someday");
    assert!(harness.query_by_label("Not a time yet: try thu 9, 12 nov or in 3 days").is_some());
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().picked.is_empty());
    assert_eq!(harness.state().picker.chosen(friday(), Offset::UTC), None);
}

#[test]
fn nothing_typed_cannot_be_confirmed_either() {
    let mut harness = harness(false);
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    assert!(harness.state().picked.is_empty());
}

#[test]
fn cancel_and_escape_cancel() {
    let mut harness = harness(false);
    harness.get_by_role_and_label(Role::Button, "Cancel").click();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::Cancelled, Picked::Cancelled]);
}

#[test]
fn the_alert_switch_goes_with_the_time_and_remembers_its_last_value() {
    let mut harness = harness(true);
    let switch = harness.get_by_role_and_label(Role::Switch, "Notify me when it comes back");
    assert_eq!(switch.accesskit_node().toggled(), Some(Toggled::False));
    switch.click();
    harness.run();
    harness.get_by_label("Later today, 17:30").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 6, 17, 30), alert: true }]);
    // The next opening starts with nothing typed and the switch as it was left.
    typed(&mut harness, "thu 9");
    harness.state_mut().picker.open();
    harness.run();
    assert_eq!(harness.state().picker.typed(), "");
    assert!(harness.state().picker.alert());
    let switch = harness.get_by_role_and_label(Role::Switch, "Notify me when it comes back");
    assert_eq!(switch.accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn without_an_alert_question_there_is_no_switch_and_no_alert() {
    let mut harness = harness(false);
    harness.state_mut().picker.set_alert(true);
    harness.run();
    assert!(harness.query_by_role(Role::Switch).is_none());
    harness.get_by_label("Later today, 17:30").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 6, 17, 30), alert: false }]);
}

#[test]
fn typed_text_is_capped() {
    let mut harness = harness(false);
    typed(&mut harness, &"9".repeat(super::MAX_TYPED * 2));
    assert!(harness.state().picker.typed().len() <= super::MAX_TYPED);
}

#[test]
fn a_picker_made_with_an_alert_value_starts_with_it() {
    let picker = TimePicker::with_alert(true);
    assert!(picker.alert());
    assert_eq!(picker.typed(), "");
}

#[test]
fn set_typed_keeps_at_most_max_typed_bytes_at_a_character_boundary() {
    let mut picker = TimePicker::default();
    picker.set_typed("thu 9");
    assert_eq!(picker.typed(), "thu 9");
    picker.set_typed(&"é".repeat(super::MAX_TYPED));
    assert!(picker.typed().len() <= super::MAX_TYPED && picker.typed().chars().all(|c| c == 'é'), "{}", picker.typed());
    assert_eq!(picker.typed().chars().count(), super::MAX_TYPED / 2);
    picker.set_typed(&format!("a{}", "é".repeat(super::MAX_TYPED)));
    assert_eq!(picker.typed().len(), super::MAX_TYPED - 1, "the cut steps back to the last whole character");
}
