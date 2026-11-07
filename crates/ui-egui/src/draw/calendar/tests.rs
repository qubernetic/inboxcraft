// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen behind the rail's Calendar: its empty state, the way there and back, and the mail's keys
//! resting while it shows.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::Mailbox;
use crate::{InboxApp, Screen, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn harness(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn toggled(harness: &Harness<'static, InboxApp>, name: &str) -> Option<Toggled> {
    harness.get_by_role_and_label(Role::Button, name).accesskit_node().toggled()
}

#[test]
fn the_rail_s_calendar_opens_the_calendar_s_empty_state_and_mail_comes_back() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness(theme);
        assert_eq!(harness.state().screen(), Screen::Mail);
        assert!(!harness.get_by_role_and_label(Role::Button, "Calendar").accesskit_node().is_disabled());
        harness.get_by_role_and_label(Role::Button, "Calendar").click();
        harness.run();
        assert_eq!(harness.state().screen(), Screen::Calendar);
        assert!(harness.state().calendar().is_empty());
        harness.get_by_label("No calendars yet");
        assert_eq!(toggled(&harness, "Calendar"), Some(Toggled::True));
        assert_eq!(toggled(&harness, "Mail"), Some(Toggled::False));
        assert_eq!(harness.query_all_by_label_contains("Draft logo for the bakery").count(), 0, "the list gives the calendar its place");
        harness.check_accessibility();
        harness.get_by_role_and_label(Role::Button, "Mail").click();
        harness.run();
        assert_eq!(harness.state().screen(), Screen::Mail);
        assert!(harness.query_by_label("No calendars yet").is_none());
        assert!(harness.query_all_by_label_contains("Draft logo for the bakery").count() > 0);
    }
}

#[test]
fn command_2_shows_the_calendar_and_leaves_the_settings_sheet() {
    let mut harness = harness(egui::Theme::Light);
    harness.state_mut().open_settings();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num2);
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Calendar);
    assert!(!harness.state().settings().is_open());
    harness.get_by_label("No calendars yet");
}

#[test]
fn the_mail_s_keys_rest_while_the_calendar_shows() {
    let mut harness = harness(egui::Theme::Light);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let selected = harness.state().mailbox().selected();
    assert!(selected.is_some());
    harness.get_by_role_and_label(Role::Button, "Calendar").click();
    harness.run();
    let before = harness.state().mailbox().last_run();
    for key in [Key::ArrowDown, Key::D, Key::Delete, Key::Escape] {
        harness.key_press(key);
        harness.run();
    }
    assert_eq!(harness.state().mailbox().selected(), selected, "the arrows step nothing");
    assert_eq!(harness.state().mailbox().last_run(), before, "no act runs on a list that is not there");
    assert_eq!(harness.state().screen(), Screen::Calendar, "Escape leaves the calendar where it is");
}
