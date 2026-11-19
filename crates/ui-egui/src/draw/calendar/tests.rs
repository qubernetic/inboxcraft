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
    let node = harness.query_by_role_and_label(Role::RadioButton, name).unwrap_or_else(|| harness.get_by_role_and_label(Role::Button, name));
    node.accesskit_node().toggled()
}

fn press(harness: &mut Harness<'static, InboxApp>, name: &str) {
    // A button, or a segmented control's option.
    harness.query_by_role_and_label(Role::RadioButton, name).unwrap_or_else(|| harness.get_by_role_and_label(Role::Button, name)).click();
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

/// Whether the details show each of `words` as a tag.
fn tags(harness: &Harness<'static, InboxApp>, words: &[&str]) -> bool {
    words.iter().all(|word| harness.query_all_by_label(word).next().is_some())
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
        harness.get_by_label("No calendars yet.");
        assert!(harness.query_by_label("Week").is_none(), "no views without calendars");
        harness.check_accessibility();
    }
}

#[test]
fn without_calendars_the_words_stand_on_the_whole_card_where_the_frame_has_them() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let p = crate::theme::Palette::of(if theme == egui::Theme::Dark { crate::theme::Mode::Dark } else { crate::theme::Mode::Light });
        let store = Memory::new(StoreConfig::default());
        let app = InboxApp::new(Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock()));
        let mut harness = build(app, theme);
        press(&mut harness, "Calendar");
        let block = harness.get_by_role_and_label(Role::Label, "No calendars yet.").rect();
        // The frame: the calendar's card over the whole body in `surface`, no box of its own round the words; the
        // block 440 wide in its middle, 304 under its top and 336 over its bottom (room for a button 32 high).
        let card = flat_rects(&harness)
            .into_iter()
            .filter(|(rect, fill)| *fill == p.surface && rect.contains_rect(block))
            .map(|(rect, _)| rect)
            .max_by(|a, b| a.area().total_cmp(&b.area()))
            .unwrap();
        assert!(card.width() > 900.0, "the card is the screen's, not a box round the words: {card:?}");
        assert_eq!(block.width(), 440.0, "{block:?}");
        assert!((block.center().x - card.center().x).abs() <= 0.5, "{block:?} in {card:?}");
        let (over, under) = (block.top() - card.top(), card.bottom() - block.bottom());
        assert!((under - over - 32.0).abs() <= 1.0, "over {over}, under {under}");
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
fn the_segmented_control_switches_between_day_week_month_and_agenda() {
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
        assert!(shows(&harness, "Thursday 5 November, today"), "today's head says so");
        assert!(shows(&harness, "Ferry to the island, Sunday 8 November"));
        harness.check_accessibility();

        press(&mut harness, "Day");
        assert_eq!(harness.state().calendar().view(), View::Day);
        assert_eq!(harness.get_all_by_label("Thursday 5 November 2026").count(), 1, "the title");
        assert_eq!(harness.get_all_by_label("Thursday 5 November 2026, today").count(), 2, "the day's head and the mini month's today");
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
    // The week's head comes first, the mini month's day after it.
    harness.get_all_by_label("Wednesday 18 November 2026").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().calendar().view(), View::Day, "a day's name in the week opens the day");
    assert_eq!(harness.get_all_by_label("Wednesday 18 November 2026").count(), 3, "the title, the day's head and the mini month's day");
    let (calendar, mailbox) = harness.state_mut().calendar_mut();
    calendar.set_view(mailbox, View::Month);
    harness.run();
    harness.get_all_by_label("Tuesday 17 November 2026").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().calendar().focus(), day(11, 17));
    assert_eq!(harness.state().calendar().view(), View::Day);
}

#[test]
fn an_event_is_coloured_by_its_account_s_bar_only() {
    let harness = on_calendar(egui::Theme::Light);
    let calendar = harness.state().calendar();
    let theme = crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::default());
    for row in calendar.calendars() {
        let place = calendar.account_index(row.account);
        assert_eq!(calendar.account_place(row.id), place, "{}: its account's place", row.name);
        assert_eq!(super::bar_colour(&theme, calendar, row.id), theme.account_colour(place), "{}: the account's colour", row.name);
    }
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
        assert!(tags(&harness, &["Personal", "busy"]));
        harness.check_accessibility();

        pick(&mut harness, "Call with Petra about the logo");
        assert!(!shows(&harness, "Event zone"), "an event on the computer's clock has no zone line");
        assert!(tags(&harness, &["Studio", "busy"]));
        harness.get_by_label("Petra Lindqvist, going");

        press(&mut harness, "Close the details");
        assert!(harness.state().calendar().picked().is_none());
        assert!(harness.query_by_label("Petra Lindqvist, going").is_none());
    }
}

#[test]
fn a_block_over_several_days_shows_both_dates_and_a_second_press_lets_it_go() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "Month");
    pick(&mut harness, "Coast trip");
    harness.get_by_label("Monday 16 November – Wednesday 18 November");
    assert!(tags(&harness, &["Personal", "busy"]));
    pick(&mut harness, "Coast trip");
    assert!(harness.state().calendar().picked().is_none());
    pick(&mut harness, "Studio stand-up");
    assert!(tags(&harness, &["Studio", "busy", "repeats weekly"]));
    pick(&mut harness, "Dentist");
    assert!(tags(&harness, &["Personal", "busy", "private"]));
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

#[test]
fn today_is_a_cyan_edge_box_and_the_picked_day_the_cyan_fill_with_on_cyan() {
    let p = crate::theme::Palette::of(crate::theme::Mode::Light);
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "Month");
    let rects = |harness: &Harness<'static, InboxApp>| flat_rects(harness).into_iter().map(|(_, fill)| fill).collect::<Vec<_>>();
    let cyan = |harness: &Harness<'static, InboxApp>, colour| rects(harness).into_iter().filter(|fill| *fill == colour).count();
    assert!(cyan(&harness, p.cyan_edge) >= 8, "today in the month and the mini month in a cyan_edge box, four lines each");
    let before = cyan(&harness, p.cyan);
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(harness.state().calendar().picked_day(), Some(day(11, 6)));
    assert_eq!(cyan(&harness, p.cyan), before + 2, "the picked day on cyan, in the month and the mini month");
    assert!(texts(&harness, "6").contains(&p.on_cyan), "its number in on_cyan");
    assert!(crate::theme::contrast(p.on_cyan, p.cyan) >= 4.5);
}

/// Every filled rect painted in the last frame, with its fill.
fn flat_rects(harness: &Harness<'static, InboxApp>) -> Vec<(egui::Rect, egui::Color32)> {
    fn open(shape: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            egui::Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

/// The colours the texts `text` were painted in, in the last frame.
fn texts(harness: &Harness<'static, InboxApp>, text: &str) -> Vec<egui::Color32> {
    fn open(shape: &egui::Shape, text: &str, out: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, text, out)),
            egui::Shape::Text(shape) if shape.galley.text() == text => {
                out.extend(shape.galley.rows.iter().flat_map(|row| row.visuals.mesh.vertices.iter()).map(|vertex| vertex.color).next());
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, text, &mut out));
    out
}

#[test]
fn a_long_title_in_the_details_pushes_the_times_down_and_the_day_s_times_stand_under_the_day() {
    let mut harness = on_calendar(egui::Theme::Dark);
    pick(&mut harness, "Call with Petra about the logo");
    let title = harness.get_by_label("Call with Petra about the logo").rect();
    let when = harness.get_by_label("Thursday 5 November, 09:30 – 10:30").rect();
    assert!(when.top() >= title.bottom(), "the times under the title: {title:?} {when:?}");
    assert!(when.height() >= 2.0 * 22.0 - 0.5, "the times on a line of their own: {when:?}");
}

#[test]
fn the_week_drawn_follows_the_rule_set_in_the_session_with_today_in_it() {
    for (rule, title, note, heads) in [
        (r#"{"first_day": "monday"}"#, "2 – 8 November 2026", "week 45", ["Monday 2 November 2026", "Sunday 8 November 2026"]),
        (r#"{"first_day": "sunday"}"#, "1 – 7 November 2026", "week 44", ["Sunday 1 November 2026", "Saturday 7 November 2026"]),
        (
            r#"{"first_day": "saturday", "first_week": "week_of_1_january"}"#,
            "31 October – 6 November 2026",
            "week 45",
            ["Saturday 31 October 2026", "Friday 6 November 2026"],
        ),
    ] {
        let mut harness = on_calendar(egui::Theme::Light);
        {
            let (calendar, mailbox) = harness.state_mut().calendar_mut();
            mailbox.run("week.set", &serde_json::from_str(rule).unwrap()).unwrap();
            // What the window does once a frame.
            calendar.follow_week(mailbox);
        }
        harness.run();
        harness.get_by_label(title);
        harness.get_by_label(note);
        assert_eq!(harness.get_all_by_label("Thursday 5 November 2026, today").count(), 2, "{rule}: today's head and the mini month's");
        // A day's head is the widest of the buttons named after it (the mini month's are 28 wide).
        let head = |name: &str| harness.get_all_by_label(name).map(|node| node.rect()).max_by(|a, b| a.width().total_cmp(&b.width())).unwrap();
        let [first, last] = heads.map(head);
        assert!(first.width() > 60.0 && last.width() > 60.0, "{rule}: the heads over the hours");
        assert!(first.left() < last.left(), "{rule}: the week's first day at the left");
    }
}
