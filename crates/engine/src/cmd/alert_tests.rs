// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Event reminders in a session on a fake clock: the store holds each event's alerts as it changes, the scheduler
//! fires them, and the owner is told at the alert time, also across a change of the clock.

use std::time::Duration;

use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, CalendarReport, Due, Event, EventKind, FetchedEvent};
use inboxcraft_time::{CivilDateTime, FakeClock, Offset, OffsetTable, Timestamp};
use serde_json::json;

use super::calendar_tests::{WORK, lunch, session_at};
use crate::Session;
use crate::notify::{EventAbout, Notice, Settings};
use crate::test_support::{NOW, at};

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap().to_timestamp(Offset::UTC).unwrap()
}

fn fresh() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (session_at(&clock), clock)
}

/// When event `id`'s alerts go off, as the store holds them.
fn alerts(session: &Session, id: u64) -> Vec<Timestamp> {
    session.store().alerts(CalendarEventId::new(id)).unwrap().into_iter().map(|alert| alert.at).collect()
}

/// The schedule's alert entries, by time.
fn scheduled(session: &Session) -> Vec<Timestamp> {
    session.store().schedule().unwrap().into_iter().filter(|entry| matches!(entry.what, Due::Alert(_))).map(|entry| entry.due).collect()
}

fn told(session: &mut Session) -> Vec<Notice> {
    session.take_notices().into_iter().filter(|notice| matches!(notice, Notice::Alert { .. })).collect()
}

/// Moves `clock` to `to` and ticks.
fn at_time(session: &mut Session, clock: &FakeClock, to: Timestamp) {
    clock.advance(to.duration_since(clock_now(clock)).unwrap());
    session.tick().unwrap();
}

fn clock_now(clock: &FakeClock) -> Timestamp {
    inboxcraft_time::Clock::now(clock)
}

fn dentist(session: &mut Session) -> u64 {
    let params = json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-05T12:00:00Z", "location": "Main Street 1" });
    session.run("event.create", &params).unwrap()["id"].as_u64().unwrap()
}

#[test]
fn a_new_event_alerts_ten_minutes_before_and_tells_the_owner_then() {
    let (mut session, clock) = fresh();
    let id = dentist(&mut session);
    assert_eq!(alerts(&session, id), [utc(2026, 11, 5, 11, 50)]);
    assert_eq!(scheduled(&session), [utc(2026, 11, 5, 11, 50)]);
    assert_eq!(session.next_due().unwrap(), Some(utc(2026, 11, 5, 11, 50)));
    at_time(&mut session, &clock, utc(2026, 11, 5, 11, 49));
    assert!(told(&mut session).is_empty(), "not before its time");
    at_time(&mut session, &clock, utc(2026, 11, 5, 11, 50));
    let about = EventAbout {
        account: AccountId::new(1),
        calendar: CalendarId::new(1),
        event: CalendarEventId::new(id),
        summary: "Dentist".to_owned(),
        location: "Main Street 1".to_owned(),
        start: utc(2026, 11, 5, 12, 0),
        end: utc(2026, 11, 5, 13, 0),
        all_day: false,
    };
    assert_eq!(told(&mut session), [Notice::Alert { about, missed: false }]);
    assert!(alerts(&session, id).is_empty(), "a fired alert leaves the store");
    assert!(scheduled(&session).is_empty());
    session.tick().unwrap();
    assert!(told(&mut session).is_empty(), "it fires once");
}

#[test]
fn an_alert_notice_names_its_event_for_a_click_and_belongs_to_its_account() {
    let (mut session, clock) = fresh();
    let id = dentist(&mut session);
    at_time(&mut session, &clock, utc(2026, 11, 5, 11, 50));
    let notice = told(&mut session).remove(0);
    assert_eq!(notice.account(), AccountId::new(1));
    assert_eq!(notice.message(), None, "it opens an event, not a message");
    assert!(matches!(notice, Notice::Alert { about, .. } if about.event == CalendarEventId::new(id)));
}

#[test]
fn an_edited_event_moves_its_alert_and_a_deleted_one_takes_it_and_undo_brings_it_back() {
    let (mut session, _) = fresh();
    let id = dentist(&mut session);
    session.run("event.update", &json!({ "id": id, "start": "2026-11-05T15:00:00Z" })).unwrap();
    assert_eq!(alerts(&session, id), [utc(2026, 11, 5, 14, 50)]);
    assert_eq!(scheduled(&session), [utc(2026, 11, 5, 14, 50)], "the old entry left with its alert");
    session.run("event.delete", &json!({ "id": id })).unwrap();
    assert!(alerts(&session, id).is_empty());
    assert!(scheduled(&session).is_empty());
    session.run("history.undo", &json!({})).unwrap();
    assert_eq!(alerts(&session, id), [utc(2026, 11, 5, 14, 50)]);
}

#[test]
fn an_unchanged_alert_keeps_its_id_and_schedule_entry() {
    let (mut session, _) = fresh();
    let id = dentist(&mut session);
    let before = session.store().schedule().unwrap();
    session.run("event.update", &json!({ "id": id, "summary": "Dentist again" })).unwrap();
    assert_eq!(session.store().schedule().unwrap(), before);
}

#[test]
fn the_start_works_out_the_alerts_of_every_event_held() {
    let (mut session, _) = fresh();
    assert!(scheduled(&session).is_empty(), "the store came without alerts");
    session.start();
    // Lunch at 11:00 and the call at 14:00 UTC on Friday 6th, the stand-up at 9:30 in Budapest on Monday 9th, the
    // days off from Tuesday 10th on the computer's clock (+01:00).
    assert_eq!(alerts(&session, 2), [utc(2026, 11, 6, 10, 50)]);
    assert_eq!(alerts(&session, 3), [utc(2026, 11, 6, 13, 50)]);
    assert_eq!(alerts(&session, 1), [utc(2026, 11, 9, 8, 20), utc(2026, 11, 16, 8, 20)]);
    assert_eq!(alerts(&session, 4), [utc(2026, 11, 8, 23, 0)]);
    assert_eq!(session.next_due().unwrap(), Some(utc(2026, 11, 6, 9, 0)), "the next sweep, a day after the start");
}

#[test]
fn an_alert_missed_while_off_tells_the_owner_late_unless_the_event_is_over() {
    let (mut session, clock) = fresh();
    let id = dentist(&mut session);
    clock.advance(Duration::from_secs(3 * 3600 - 300));
    session.start();
    session.tick().unwrap();
    assert!(matches!(told(&mut session).as_slice(), [Notice::Alert { about, missed: true }] if about.event == CalendarEventId::new(id)));

    let (mut session, clock) = fresh();
    dentist(&mut session);
    clock.advance(Duration::from_secs(5 * 3600));
    session.start();
    session.tick().unwrap();
    assert!(told(&mut session).is_empty(), "the dentist was over at 13:00");
    assert!(alerts(&session, 5).is_empty());
}

#[test]
fn a_repeating_event_has_the_alerts_of_the_next_two_weeks_and_the_sweep_adds_the_next() {
    let (mut session, clock) = fresh();
    session.start();
    let params = json!({ "calendar": 2, "summary": "Walk", "start": "2026-11-05T12:00:00Z", "repeat": "daily" });
    let id = session.run("event.create", &params).unwrap()["id"].as_u64().unwrap();
    let held = alerts(&session, id);
    assert_eq!(held.len(), 14);
    assert_eq!(held.first(), Some(&utc(2026, 11, 5, 11, 50)));
    assert_eq!(held.last(), Some(&utc(2026, 11, 18, 11, 50)));
    at_time(&mut session, &clock, utc(2026, 11, 5, 11, 50));
    assert_eq!(told(&mut session).len(), 1);
    assert_eq!(alerts(&session, id).len(), 13, "the sweep comes a day after the start");
    assert_eq!(session.next_due().unwrap(), Some(utc(2026, 11, 6, 9, 0)), "the sweep is due before the next alert");
    at_time(&mut session, &clock, utc(2026, 11, 6, 9, 0));
    let held = alerts(&session, id);
    assert_eq!(held.len(), 14);
    assert_eq!(held.last(), Some(&utc(2026, 11, 19, 11, 50)));
}

#[test]
fn a_far_event_comes_into_the_horizon_with_a_sweep() {
    let (mut session, clock) = fresh();
    session.start();
    let id =
        session.run("event.create", &json!({ "calendar": 1, "summary": "Later", "start": "2026-11-25T12:00:00Z" })).unwrap()["id"].as_u64().unwrap();
    assert!(alerts(&session, id).is_empty(), "beyond two weeks");
    for _ in 0..7 {
        clock.advance(Duration::from_secs(86_400));
        session.tick_if_due().unwrap();
    }
    assert_eq!(alerts(&session, id), [utc(2026, 11, 25, 11, 50)]);
}

#[test]
fn a_synced_event_gets_its_alerts_and_one_removed_by_a_sync_takes_them() {
    let (mut session, _) = fresh();
    let fetched = FetchedEvent { url: format!("{WORK}new.ics"), etag: Some("\"n1\"".to_owned()), ical: lunch().replace("UID:lunch", "UID:new") };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    assert_eq!(alerts(&session, 5), [utc(2026, 11, 6, 10, 50)]);
    let removed =
        CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![], removed: vec![format!("{WORK}new.ics")], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(removed))).unwrap();
    assert!(scheduled(&session).is_empty());
}

#[test]
fn with_alerts_turned_off_an_alert_tells_nothing_but_still_leaves() {
    let (mut session, clock) = fresh();
    let id = dentist(&mut session);
    let mut settings = Settings::default();
    settings.alerts = false;
    session.set_notify_settings(settings);
    at_time(&mut session, &clock, utc(2026, 11, 5, 11, 50));
    assert!(told(&mut session).is_empty());
    assert!(alerts(&session, id).is_empty());
}

#[test]
fn an_event_the_owner_declined_alerts_nothing() {
    let (mut session, _) = fresh();
    let ical = lunch().replace("ATTENDEE;CN=Dora:mailto:dora@example.com", "ATTENDEE;PARTSTAT=DECLINED:mailto:anna@example.org");
    let fetched = FetchedEvent { url: format!("{WORK}declined.ics"), etag: Some("\"d1\"".to_owned()), ical };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    assert!(alerts(&session, 5).is_empty());
}

/// Central Europe in 2026: +02:00 until 25 October 01:00 UTC, +01:00 after.
fn europe() -> OffsetTable {
    let hours = |hours: i16| Offset::from_minutes(hours * 60).unwrap();
    OffsetTable::new(hours(2)).with_change(utc(2026, 10, 25, 1, 0), hours(1))
}

#[test]
fn a_daily_alert_follows_the_wall_clock_across_the_change_to_winter_time() {
    let clock = FakeClock::new(utc(2026, 10, 23, 12, 0));
    let mut session = session_at(&clock);
    session.set_offsets(Box::new(europe()));
    session.start();
    // Nine o'clock on the computer's clock every day; a repeating event is written on the wall clock.
    let params = json!({ "calendar": 2, "summary": "Gym", "start": "2026-10-24T09:00:00+02:00", "repeat": "daily" });
    session.run("event.create", &params).unwrap();
    let mut fired = Vec::new();
    for _ in 0..(3 * 24 * 6) {
        clock.advance(Duration::from_secs(600));
        session.tick_if_due().unwrap();
        fired.extend(told(&mut session).into_iter().map(|_| clock_now(&clock)));
    }
    // 08:50 on the wall clock: 06:50 UTC on Saturday 24th, 07:50 UTC from Sunday 25th.
    assert_eq!(fired, [utc(2026, 10, 24, 6, 50), utc(2026, 10, 25, 7, 50), utc(2026, 10, 26, 7, 50)]);
}
