// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window and the week rule (`week.set`): the calendar takes a new rule at the next frame, the rule is kept
//! between runs beside the notification choices, a kept file that cannot be read is the default, and a rule taken back
//! at the start reads no calendar the owner has not opened. The hour all-day alerts ring at is kept beside the rule.

use std::sync::{Arc, Mutex};

use egui::vec2;
use egui_kittest::Harness;
use inboxcraft_engine::Session;
use inboxcraft_engine::all_day_alert::AllDayAlert;
use inboxcraft_engine::appearance::Appearance;
use inboxcraft_engine::notify::{Level, Settings as NotifySettings};
use inboxcraft_model::AccountId;
use inboxcraft_time::{FakeClock, FirstWeek, Timestamp, WeekRule, Weekday};
use serde_json::{Value, json};

use super::InboxApp;
use crate::keep::Keep;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::theme::install_fonts;

/// Thursday 5 November 2026, 09:00 UTC.
const NOW: i64 = 1_793_869_200;

const SUNDAY: WeekRule = WeekRule { first_day: Weekday::Sunday, first_week: FirstWeek::WeekOf1January };

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn sunday() -> Value {
    json!({ "first_day": "sunday", "first_week": "week_of_1_january" })
}

/// The sample's mail in a session that is not the sample's, as the owner's own mail would be.
fn real_app() -> InboxApp {
    let session = Session::new(Box::new(sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap()), clock()).unwrap();
    let mut app = InboxApp::at_mail(Mailbox::new(session, clock()));
    app.set_reduce_motion(true);
    app
}

fn harness_of(app: InboxApp) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 900.0))
        .with_theme(egui::Theme::Light)
        .with_context_setup(install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// A keep that holds the week rule and the notification choices in memory and records what it was given.
#[derive(Clone, Default)]
struct Held {
    week: Arc<Mutex<Option<Value>>>,
    notify: Arc<Mutex<Option<NotifySettings>>>,
    stored: Arc<Mutex<Vec<Value>>>,
}

impl Keep for Held {
    fn load(&self) -> Option<Appearance> {
        None
    }

    fn store(&mut self, _appearance: &Appearance) -> Result<(), String> {
        Ok(())
    }

    fn load_notify(&self) -> Option<NotifySettings> {
        self.notify.lock().unwrap().clone()
    }

    fn load_week(&self) -> Option<Value> {
        self.week.lock().unwrap().clone()
    }

    fn store_week(&mut self, week: &Value) -> Result<(), String> {
        self.stored.lock().unwrap().push(week.clone());
        *self.week.lock().unwrap() = Some(week.clone());
        Ok(())
    }
}

#[test]
fn a_week_rule_set_through_the_session_shows_in_the_calendar_at_the_next_frame() {
    let mut app = real_app();
    app.show_calendar();
    let mut harness = harness_of(app);
    assert_eq!(harness.state().calendar.week_rule(), WeekRule::default());
    harness.state_mut().mailbox_mut().run("week.set", &sunday()).unwrap();
    harness.step();
    assert_eq!(harness.state().calendar.week_rule(), SUNDAY, "the calendar took the rule at the next frame");
}

#[test]
fn the_week_rule_is_kept_and_comes_back_at_the_next_start() {
    let keep = Held::default();
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())));
    assert!(keep.stored.lock().unwrap().is_empty(), "nothing changed yet");
    harness.state_mut().mailbox_mut().run("week.set", &sunday()).unwrap();
    harness.run();
    harness.run();
    assert_eq!(keep.stored.lock().unwrap().len(), 1, "kept once per change");
    let again = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(again.mailbox().week_rule(), SUNDAY, "the rule came back at the start");
}

#[test]
fn a_junk_kept_week_is_the_default_and_the_other_kept_choices_stay() {
    for junk in [json!("sunday"), json!({ "first_day": 7, "first_week": [] }), json!({ "first_day": "\u{202e}sunday" }), Value::Null] {
        let keep = Held::default();
        *keep.week.lock().unwrap() = Some(junk.clone());
        let mut notify = NotifySettings::default();
        notify.set_level(AccountId::new(1), Level::None);
        *keep.notify.lock().unwrap() = Some(notify);
        let app = real_app().with_keep(Box::new(keep.clone()));
        assert_eq!(app.mailbox().week_rule(), WeekRule::default(), "{junk}");
        assert_eq!(app.mailbox().notify_level(1), Level::None, "{junk}: the notification choices stay");
    }
}

/// Guards the screen gate: off the calendar the window never follows the rule (`follow_week` runs only behind
/// `calendar_before`'s check of the screen shown), so a kept rule reads nothing until the calendar is opened. The
/// calendar's own `read` guard (no rule followed before its first read) is bitten by the unit test
/// `calendar::week_tests::a_calendar_never_read_follows_no_rule_until_its_first_read`, not by this one.
#[test]
fn a_kept_rule_reads_no_calendar_the_owner_has_not_opened() {
    let keep = Held::default();
    *keep.week.lock().unwrap() = Some(sunday());
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())));
    let serial = harness.state().calendar.serial();
    for _ in 0..3 {
        harness.run();
    }
    assert_eq!(harness.state().calendar.serial(), serial, "the mail view read no calendar");
    assert!(harness.state().mailbox().error().is_none());
    harness.state_mut().show_calendar();
    harness.run();
    assert_eq!(harness.state().calendar.week_rule(), SUNDAY, "opened, the calendar counts by the kept rule");
}

#[test]
fn with_nothing_kept_the_week_follows_the_locale_and_junk_falls_back_to_it() {
    let us = inboxcraft_engine::week::for_locale("en_US.UTF-8");
    assert_eq!(us, SUNDAY, "a Sunday region counts from Sunday and the week of 1 January");
    assert_eq!(inboxcraft_engine::week::for_locale("de_DE.UTF-8"), WeekRule::default(), "Monday and ISO weeks");
    for junk_locale in ["", "C", "POSIX", "en", "\u{202e}en_US", "en_ZZ", &"x".repeat(10_000)] {
        assert_eq!(inboxcraft_engine::week::for_locale(junk_locale), WeekRule::default(), "{junk_locale:?}: Monday");
    }
    assert_eq!(inboxcraft_engine::week::for_locale("en-CA"), SUNDAY, "a hyphen too");
    // Nothing kept: the locale's.
    let app = real_app().with_week_default(us).with_keep(Box::new(Held::default()));
    assert_eq!(app.mailbox().week_rule(), us);
    // A junk file: the locale's, not Monday.
    let keep = Held::default();
    *keep.week.lock().unwrap() = Some(json!({ "first_day": "\u{202e}sunday", "first_week": 3 }));
    assert_eq!(real_app().with_week_default(us).with_keep(Box::new(keep)).mailbox().week_rule(), us);
    // What the owner chose stands over the locale.
    let keep = Held::default();
    *keep.week.lock().unwrap() = Some(json!({ "first_day": "tuesday", "first_week": "first_four_day_week" }));
    let chosen = real_app().with_week_default(us).with_keep(Box::new(keep)).mailbox().week_rule();
    assert_eq!(chosen, WeekRule { first_day: Weekday::Tuesday, first_week: FirstWeek::FirstFourDayWeek });
}

#[test]
fn the_calendar_page_sets_the_first_day_and_the_first_week_on_the_owner_s_press() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable as _;
    let mut app = real_app();
    app.open_settings_page(Some(crate::settings::Section::Calendar.id()));
    let mut harness = harness_of(app);
    harness.get_by_role_and_label(Role::RadioButton, "Week of 1 January").hover();
    harness.run();
    harness.get_by_role_and_label(Role::RadioButton, "Week of 1 January").focus();
    harness.run();
    assert_eq!(harness.state().mailbox().week_rule(), WeekRule::default(), "a hover or a focus changes nothing");
    harness.get_by_role_and_label(Role::RadioButton, "Week of 1 January").click();
    harness.run();
    assert_eq!(harness.state().mailbox().week_rule().first_week, FirstWeek::WeekOf1January);
    harness.get_by_label("Week starts on").click();
    harness.run();
    assert_eq!(harness.state().mailbox().week_rule().first_day, Weekday::Monday, "opening the choice changes nothing");
    harness.get_by_label("Sunday").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().mailbox().week_rule(), SUNDAY);
    // The calendar counts by it.
    harness.state_mut().show_calendar();
    harness.step();
    assert_eq!(harness.state().calendar.week_rule(), SUNDAY);
}

#[test]
fn the_calendar_page_sets_the_hour_all_day_alerts_ring_at_on_the_owner_s_press() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};
    let mut app = real_app();
    app.open_settings_page(Some(crate::settings::Section::Calendar.id()));
    let mut harness = harness_of(app);
    let shown =
        |harness: &Harness<'static, InboxApp>| harness.get_by_role_and_label(Role::ComboBox, "All-day alerts ring at").accesskit_node().value();
    assert_eq!(shown(&harness).as_deref(), Some("09:00 the day before"), "the default");
    harness.get_by_label("All-day alerts ring at").click();
    harness.run();
    assert_eq!(harness.state().mailbox().all_day_alert(), AllDayAlert::DayBefore9, "opening the choice changes nothing");
    harness.get_by_label("18:00 the day before").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("event.all_day_alert.set"));
    assert_eq!(harness.state().mailbox().all_day_alert(), AllDayAlert::DayBefore18);
    assert_eq!(shown(&harness).as_deref(), Some("18:00 the day before"));
}

#[test]
fn the_all_day_alert_is_kept_beside_the_week_rule_and_an_old_file_reads_as_the_default() {
    let keep = Held::default();
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())));
    harness.state_mut().mailbox_mut().run("event.all_day_alert.set", &json!({ "at": "on_the_day_9" })).unwrap();
    harness.run();
    harness.run();
    let stored = keep.stored.lock().unwrap().clone();
    assert_eq!(stored.len(), 1, "kept once per change, the week rule's file too");
    assert_eq!(stored.last().and_then(|kept| kept.get("all_day_alert")), Some(&json!("on_the_day_9")));
    let again = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(again.mailbox().all_day_alert(), AllDayAlert::OnTheDay9, "it came back at the start");
    assert_eq!(again.mailbox().week_rule(), WeekRule::default(), "beside the week rule, which stays");
    // A file kept before the setting, or with junk under its key: the default, and the week rule still read.
    for old in [sunday(), json!({ "first_day": "sunday", "first_week": "week_of_1_january", "all_day_alert": 9 })] {
        let keep = Held::default();
        *keep.week.lock().unwrap() = Some(old.clone());
        let app = real_app().with_keep(Box::new(keep));
        assert_eq!(app.mailbox().all_day_alert(), AllDayAlert::DayBefore9, "{old}");
        assert_eq!(app.mailbox().week_rule(), SUNDAY, "{old}");
    }
}
