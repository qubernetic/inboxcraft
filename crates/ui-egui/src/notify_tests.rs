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
