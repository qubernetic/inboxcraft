// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window and the week rule (`week.set`): the calendar takes a new rule at the next frame, the rule is kept
//! between runs beside the notification choices, a kept file that cannot be read is the default, and a rule taken back
//! at the start reads no calendar the owner has not opened.

use std::sync::{Arc, Mutex};

use egui::vec2;
use egui_kittest::Harness;
use inboxcraft_engine::Session;
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
    let mut app = InboxApp::new(Mailbox::new(session, clock()));
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
