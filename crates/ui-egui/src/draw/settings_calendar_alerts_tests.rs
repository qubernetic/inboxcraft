// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Settings › Calendar: the calendar alerts switch, on and off, through `notify.set` on the owner's press only, its
//! help in each state, its keyboard and the page at rest.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use crate::mailbox::Mailbox;
use crate::settings::Section;
use crate::{InboxApp, sample};

const ON: &str = "An event's alert shows as a desktop notification at its time.";
const OFF: &str = "Off: no event notifies. The alerts stay on their events and come back with the switch.";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn on_calendar() -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app.settings_mut().open_at(Section::Calendar);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_max_steps(40)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn toggled(harness: &Harness<'static, InboxApp>) -> Option<Toggled> {
    harness.get_by_role_and_label(Role::Switch, "Calendar alerts").accesskit_node().toggled()
}

/// Whether the engine tells the owner of calendar alerts, as `notify.settings` answers.
fn alerts(harness: &mut Harness<'static, InboxApp>) -> bool {
    harness.state_mut().mailbox_mut().run("notify.settings", &json!({})).unwrap()["alerts"].as_bool().unwrap()
}

#[test]
fn the_switch_turns_calendar_alerts_off_and_on_through_the_engine() {
    let mut harness = on_calendar();
    assert_eq!(toggled(&harness), Some(Toggled::True));
    assert!(harness.query_all_by_label_contains(ON).next().is_some());
    harness.get_by_role_and_label(Role::Switch, "Calendar alerts").click();
    harness.run();
    assert!(!alerts(&mut harness), "the owner's press turns them off");
    assert_eq!(toggled(&harness), Some(Toggled::False));
    assert!(harness.query_all_by_label_contains(OFF).next().is_some());
    assert!(harness.query_all_by_label_contains(ON).next().is_none(), "the help says what off means");
    harness.get_by_role_and_label(Role::Switch, "Calendar alerts").click();
    harness.run();
    assert!(alerts(&mut harness));
    assert!(harness.query_all_by_label_contains(ON).next().is_some());
}

#[test]
fn a_focus_or_a_hover_changes_nothing_and_space_on_the_focused_switch_does() {
    let mut harness = on_calendar();
    harness.get_by_role_and_label(Role::Switch, "Calendar alerts").hover();
    harness.run();
    harness.get_by_role_and_label(Role::Switch, "Calendar alerts").focus();
    harness.run();
    assert!(alerts(&mut harness), "a focus or a hover changes nothing");
    harness.key_press_modifiers(Modifiers::NONE, Key::Space);
    harness.run();
    assert!(!alerts(&mut harness), "the owner's key on the focused switch does");
}

#[test]
fn the_switch_is_the_page_s_first_stop_after_the_sections() {
    let mut harness = on_calendar();
    harness.get_by_label("Plugins, settings page").focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::NONE, Key::Tab);
    harness.run();
    let focused = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().label());
    assert_eq!(focused.flatten().as_deref(), Some("Calendar alerts"));
}

#[test]
fn the_page_rests_with_alerts_on_and_off() {
    let mut harness = on_calendar();
    assert!(crate::component::tests::rests(&mut harness, 4));
    harness.get_by_role_and_label(Role::Switch, "Calendar alerts").click();
    harness.run();
    assert!(crate::component::tests::rests(&mut harness, 4));
}
