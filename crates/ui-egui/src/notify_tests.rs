// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The notification choices in the mailbox, through the engine's `notify.set`, over the sample.

use inboxcraft_engine::notify::{Level, Settings};
use inboxcraft_model::AccountId;
use inboxcraft_time::{FakeClock, Timestamp};

use super::Mailbox;
use crate::sample;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

#[test]
fn every_account_notifies_of_all_its_mail_until_the_owner_chooses_otherwise() {
    let mut mailbox = mailbox();
    assert_eq!(mailbox.notify_level(1), Level::All);
    assert_eq!(mailbox.notify_level(2), Level::All);
    mailbox.set_notify_level(2, Level::People);
    assert_eq!(mailbox.notify_level(2), Level::People);
    assert_eq!(mailbox.notify_level(1), Level::All, "one account at a time");
    mailbox.set_notify_level(2, Level::None);
    assert_eq!(mailbox.notify_settings().level(AccountId::new(2)), Level::None);
    assert!(mailbox.error().is_none());
}

#[test]
fn returned_snoozes_and_reminders_are_switched_apart() {
    let mut mailbox = mailbox();
    assert!(mailbox.notify_settings().returns);
    assert!(mailbox.notify_settings().reminders);
    mailbox.set_notify_returns(false);
    assert!(!mailbox.notify_settings().returns);
    assert!(mailbox.notify_settings().reminders);
    mailbox.set_notify_reminders(false);
    assert!(!mailbox.notify_settings().reminders);
    mailbox.set_notify_returns(true);
    assert!(mailbox.notify_settings().returns);
}

#[test]
fn a_level_for_an_account_that_is_gone_shows_in_the_error_banner() {
    let mut mailbox = mailbox();
    mailbox.set_notify_level(99, Level::None);
    assert!(mailbox.error().is_some());
    assert_eq!(mailbox.notify_level(99), Level::All);
}

#[test]
fn kept_choices_come_back_without_a_command() {
    let mut mailbox = mailbox();
    let mut kept = Settings::default();
    kept.set_level(AccountId::new(1), Level::People);
    kept.reminders = false;
    mailbox.restore_notify(kept.clone());
    assert_eq!(mailbox.notify_settings(), &kept);
    assert_eq!(mailbox.notify_level(1), Level::People);
}

#[test]
fn the_week_rule_is_kept_beside_and_comes_back_at_the_start() {
    use inboxcraft_time::{FirstWeek, WeekRule, Weekday};
    let mut mailbox = mailbox();
    assert_eq!(mailbox.week_rule(), WeekRule::default());
    assert_eq!(mailbox.kept_week(), serde_json::json!({ "first_day": "monday", "first_week": "first_four_day_week" }));
    mailbox.run("week.set", &serde_json::json!({ "first_day": "sunday", "first_week": "week_of_1_january" })).unwrap();
    let kept = mailbox.kept_week();
    let mut next = self::mailbox();
    next.restore_week(&kept);
    assert_eq!(next.week_rule(), WeekRule { first_day: Weekday::Sunday, first_week: FirstWeek::WeekOf1January });
}

#[test]
fn a_junk_kept_week_rule_is_the_default_never_an_error() {
    use inboxcraft_time::{FirstWeek, WeekRule, Weekday};
    let long = "x".repeat(10_000);
    for junk in [
        serde_json::json!(7),
        serde_json::Value::Null,
        serde_json::json!(long),
        serde_json::json!({ "first_day": long, "first_week": long }),
        serde_json::json!({ "first_day": "funday", "first_week": "week_of_the_moon" }),
        serde_json::json!({ "first_day": 6, "first_week": null }),
        serde_json::json!(["sunday"]),
    ] {
        let mut mailbox = mailbox();
        mailbox.run("week.set", &serde_json::json!({ "first_day": "saturday", "first_week": "week_of_1_january" })).unwrap();
        mailbox.restore_week(&junk);
        assert_eq!(mailbox.week_rule(), WeekRule::default(), "{}", junk.to_string().chars().take(40).collect::<String>());
        assert!(mailbox.error().is_none());
    }
    // A part that reads stands; the other is the default.
    let mut mailbox = mailbox();
    mailbox.restore_week(&serde_json::json!({ "first_day": "sunday", "first_week": "funday" }));
    assert_eq!(mailbox.week_rule(), WeekRule { first_day: Weekday::Sunday, first_week: FirstWeek::FirstFourDayWeek });
}
