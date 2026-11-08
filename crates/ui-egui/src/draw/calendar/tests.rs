// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen behind the rail's Calendar: the way there and back, the mail's keys resting while it shows,
//! its empty state without calendars, the views and their steps, the mini month, the calendars' boxes and the
//! details of a picked event with the event's own zone.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _, by};
use inboxcraft_engine::Session;
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::calendar::{Day, View};
use crate::mailbox::Mailbox;
use crate::{InboxApp, Screen, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn build(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn harness(theme: egui::Theme) -> Harness<'static, InboxApp> {
    build(InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock())), theme)
}

/// The sample window on the calendar screen.
fn on_calendar(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = harness(theme);
    harness.get_by_role_and_label(Role::Button, "Calendar").click();
    harness.run();
    harness
}

fn toggled(harness: &Harness<'static, InboxApp>, name: &str) -> Option<Toggled> {
    harness.get_by_role_and_label(Role::Button, name).accesskit_node().toggled()
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

fn selected(harness: &Harness<'static, InboxApp>, name: &str) -> bool {
    toggled(harness, name) == Some(Toggled::True)
}

fn shows(harness: &Harness<'static, InboxApp>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

fn day(month: u8, date: u8) -> Day {
    Day::new(2026, month, date).unwrap()
}

#[test]
fn the_rail_s_calendar_opens_this_week_and_mail_comes_back() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness(theme);
        assert_eq!(harness.state().screen(), Screen::Mail);
        press(&mut harness, "Calendar");
        assert_eq!(harness.state().screen(), Screen::Calendar);
        assert_eq!(harness.state().calendar().calendars().len(), 3);
        harness.get_by_label("2 – 8 November 2026");
        assert!(shows(&harness, "Call with Petra about the logo, Thursday 5 November, 09:30 – 10:30"));
        assert_eq!(toggled(&harness, "Calendar"), Some(Toggled::True));
        assert_eq!(toggled(&harness, "Mail"), Some(Toggled::False));
        assert_eq!(harness.query_all_by_label_contains("Draft logo for the bakery").count(), 0, "the list gives the calendar its place");
        harness.check_accessibility();
        press(&mut harness, "Mail");
        assert_eq!(harness.state().screen(), Screen::Mail);
        assert!(harness.query_by_label("2 – 8 November 2026").is_none());
        assert!(harness.query_all_by_label_contains("Draft logo for the bakery").count() > 0);
    }
}

#[test]
fn without_calendars_the_screen_says_there_are_none_yet() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let store = Memory::new(StoreConfig::default());
        let app = InboxApp::new(Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock()));
        let mut harness = build(app, theme);
        press(&mut harness, "Calendar");
        assert!(harness.state().calendar().is_empty());
        harness.get_by_label("No calendars yet");
        assert!(harness.query_by_label("Week").is_none(), "no views without calendars");
        harness.check_accessibility();
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
    harness.get_by_label("2 – 8 November 2026");
}

#[test]
fn the_mail_s_keys_rest_while_the_calendar_shows() {
    let mut harness = harness(egui::Theme::Light);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let selected = harness.state().mailbox().selected();
    assert!(selected.is_some());
    press(&mut harness, "Calendar");
    let before = harness.state().mailbox().last_run();
    for key in [Key::ArrowDown, Key::D, Key::Delete, Key::Escape] {
        harness.key_press(key);
        harness.run();
    }
    assert_eq!(harness.state().mailbox().selected(), selected, "the arrows step nothing");
    assert_eq!(harness.state().mailbox().last_run(), before, "no act runs on a list that is not there");
    assert_eq!(harness.state().screen(), Screen::Calendar, "Escape leaves the calendar where it is");
}

#[test]
fn the_pills_switch_between_day_week_month_and_agenda() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        assert!(selected(&harness, "Week"));
        press(&mut harness, "Month");
        assert_eq!(harness.state().calendar().view(), View::Month);
        assert_eq!(harness.get_all_by_label("November 2026").count(), 2, "the title and the mini month's name");
        assert!(selected(&harness, "Month"));
        assert!(!selected(&harness, "Week"));
        assert!(shows(&harness, "Coast trip, Monday 16 November – Wednesday 18 November"));
        harness.check_accessibility();

        press(&mut harness, "Agenda");
        assert_eq!(harness.state().calendar().view(), View::Agenda);
        harness.get_by_label("From today");
        harness.get_by_label("Today · Thursday 5 November");
        assert!(shows(&harness, "Ferry to the island, Sunday 8 November"));
        harness.check_accessibility();

        press(&mut harness, "Day");
        assert_eq!(harness.state().calendar().view(), View::Day);
        assert_eq!(harness.get_all_by_label("Thursday 5 November 2026").count(), 2, "the title and the mini month's today");
        assert!(shows(&harness, "Dentist, Thursday 5 November, 10:00 – 11:30"));
        assert!(!shows(&harness, "Ferry to the island"), "the day shows its own events only");
        harness.check_accessibility();

        press(&mut harness, "Week");
        harness.get_by_label("2 – 8 November 2026");
    }
}

#[test]
fn the_steps_move_by_the_view_and_today_comes_back() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "Next week");
    harness.get_by_label("9 – 15 November 2026");
    press(&mut harness, "Previous week");
    press(&mut harness, "Previous week");
    harness.get_by_label("26 October – 1 November 2026");
    press(&mut harness, "Today");
    harness.get_by_label("2 – 8 November 2026");
    press(&mut harness, "Month");
    press(&mut harness, "Next month");
    assert_eq!(harness.get_all_by_label("December 2026").count(), 2, "the title and the mini month's name");
    press(&mut harness, "Day");
    press(&mut harness, "Previous day");
    assert_eq!(harness.state().calendar().focus(), day(11, 30));
    press(&mut harness, "Agenda");
    press(&mut harness, "Later days");
    assert_eq!(harness.state().calendar().focus(), day(12, 30));
    press(&mut harness, "Earlier days");
    assert_eq!(harness.state().calendar().focus(), day(11, 30));
}

#[test]
fn a_day_of_the_month_or_the_mini_month_opens_its_days() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "Month after");
    assert_eq!(harness.state().calendar().month(), day(12, 1));
    assert_eq!(harness.state().calendar().focus(), day(11, 5), "the view stays");
    press(&mut harness, "Month before");
    // The week header and the mini month both name Monday 16 November; the mini month's is the first.
    harness.get_all_by_label("Monday 16 November 2026").next().unwrap().click();
    harness.run();
    harness.get_by_label("16 – 22 November 2026");
    harness.get_all_by_label("Wednesday 18 November 2026").last().unwrap().click();
    harness.run();
    assert_eq!(harness.state().calendar().view(), View::Day, "a day's name in the week opens the day");
    assert_eq!(harness.get_all_by_label("Wednesday 18 November 2026").count(), 2, "the title and the mini month's day");
    let (calendar, mailbox) = harness.state_mut().calendar_mut();
    calendar.set_view(mailbox, View::Month);
    harness.run();
    harness.get_all_by_label("Tuesday 17 November 2026").last().unwrap().click();
    harness.run();
    assert_eq!(harness.state().calendar().focus(), day(11, 17));
    assert_eq!(harness.state().calendar().view(), View::Day);
}

#[test]
fn a_calendar_s_box_hides_its_events() {
    let mut harness = on_calendar(egui::Theme::Light);
    assert!(shows(&harness, "Dentist"));
    harness.get_by_role_and_label(Role::CheckBox, "Personal").click();
    harness.run();
    assert!(!harness.state().calendar().is_shown(1));
    assert!(!shows(&harness, "Dentist"));
    assert!(shows(&harness, "Call with Petra about the logo"), "the Studio calendar stays");
    harness.get_by_role_and_label(Role::CheckBox, "Personal").click();
    harness.run();
    assert!(shows(&harness, "Dentist"));
}

#[test]
fn a_picked_event_shows_both_dates_and_its_own_zone_labelled() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        assert!(!shows(&harness, "Event zone"));
        pick(&mut harness, "Ferry to the island");
        assert_eq!(harness.state().calendar().picked().map(|event| event.summary.as_str()), Some("Ferry to the island"));
        harness.get_by_label("Event zone Atlantic/Azores: 07:00 – 10:00");
        harness.get_by_label("Sunday 8 November, 08:00 – 11:00");
        harness.get_by_label("Grey Gull Ferries · Booking GG-2291");
        harness.get_by_label("Personal · busy");
        harness.check_accessibility();

        pick(&mut harness, "Call with Petra about the logo");
        assert!(!shows(&harness, "Event zone"), "an event on the computer's clock has no zone line");
        harness.get_by_label("Studio · busy");
        harness.get_by_label("Petra Lindqvist · going");

        press(&mut harness, "Close the details");
        assert!(harness.state().calendar().picked().is_none());
        assert!(harness.query_by_label("Petra Lindqvist · going").is_none());
    }
}

#[test]
fn a_block_over_several_days_shows_both_dates_and_a_second_press_lets_it_go() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "Month");
    pick(&mut harness, "Coast trip");
    harness.get_by_label("Monday 16 November – Wednesday 18 November");
    harness.get_by_label("Personal · busy");
    pick(&mut harness, "Coast trip");
    assert!(harness.state().calendar().picked().is_none());
    pick(&mut harness, "Studio stand-up");
    harness.get_by_label("Studio · busy · repeats weekly");
    pick(&mut harness, "Dentist");
    harness.get_by_label("Personal · busy · private");
}

#[test]
fn a_click_on_an_alert_opens_the_calendar_on_the_event_with_its_details() {
    let mut harness = harness(egui::Theme::Light);
    let id = {
        let app = harness.state_mut();
        let (calendar, mailbox) = app.calendar_mut();
        calendar.refresh(mailbox);
        calendar.events().find(|event| event.summary == "Studio stand-up").map(|event| event.id).unwrap()
    };
    // Next Monday's stand-up, 09:00 UTC.
    harness.state_mut().open_event(id, Timestamp::from_unix(1_793_869_200 + 4 * 86_400, 0).unwrap());
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Calendar);
    assert_eq!(harness.state().calendar().focus(), Day::new(2026, 11, 9).unwrap());
    assert_eq!(harness.state().calendar().picked().map(|event| event.id), Some(id));
}
