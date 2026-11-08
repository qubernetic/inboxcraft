// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! When an event's alerts go off: the defaults, the event's own alarms, nominal days across a change of the clock,
//! and what tells nothing.

use inboxcraft_ical::Calendar;
use inboxcraft_time::{CivilDateTime, Offset, OffsetTable, Timestamp};

use super::{HORIZON, MAX_ALERTS, rings, upcoming};
use crate::cmd::calendar_tests::{BUDAPEST, object};

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap().to_timestamp(Offset::UTC).unwrap()
}

fn hours(hours: i16) -> Offset {
    Offset::from_minutes(hours * 60).unwrap()
}

/// Central Europe in 2026: +01:00, +02:00 from 29 March 01:00 UTC, +01:00 again from 25 October 01:00 UTC.
fn europe() -> OffsetTable {
    OffsetTable::new(hours(1)).with_change(utc(2026, 3, 29, 1, 0), hours(2)).with_change(utc(2026, 10, 25, 1, 0), hours(1))
}

fn calendar(events: &str) -> Calendar {
    Calendar::parse(object(events).as_bytes()).unwrap()
}

fn event(lines: &str) -> Calendar {
    calendar(&format!("BEGIN:VEVENT\r\nUID:e@example.org\r\nSUMMARY:Event\r\n{lines}END:VEVENT\r\n"))
}

fn alarm(trigger: &str, action: &str) -> String {
    format!("BEGIN:VALARM\r\nACTION:{action}\r\nDESCRIPTION:Reminder\r\n{trigger}\r\nEND:VALARM\r\n")
}

/// The alert times in the horizon after `now`.
fn times(calendar: &Calendar, now: Timestamp) -> Vec<Timestamp> {
    upcoming(calendar, now, &europe(), None)
}

#[test]
fn a_timed_event_without_alarms_alerts_ten_minutes_before() {
    let dentist = event("DTSTART:20261112T080000Z\r\nDTEND:20261112T090000Z\r\n");
    assert_eq!(times(&dentist, utc(2026, 11, 5, 9, 0)), [utc(2026, 11, 12, 7, 50)]);
}

#[test]
fn an_all_day_event_without_alarms_alerts_a_day_before_on_the_wall_clock() {
    let off = event("DTSTART;VALUE=DATE:20261110\r\nDTEND;VALUE=DATE:20261112\r\n");
    // Midnight of the 10th on the computer's clock is 23:00 UTC on the 9th; a day before that.
    assert_eq!(times(&off, utc(2026, 11, 5, 9, 0)), [utc(2026, 11, 8, 23, 0)]);
}

#[test]
fn the_events_own_display_and_audio_alarms_take_the_place_of_the_defaults() {
    let lines = format!(
        "DTSTART:20261112T080000Z\r\nDTEND:20261112T090000Z\r\n{}{}{}",
        alarm("TRIGGER:-PT30M", "DISPLAY"),
        alarm("TRIGGER;RELATED=END:PT5M", "AUDIO"),
        alarm("TRIGGER:-PT1H", "EMAIL"),
    );
    assert_eq!(times(&event(&lines), utc(2026, 11, 5, 9, 0)), [utc(2026, 11, 12, 7, 30), utc(2026, 11, 12, 9, 5)]);
}

#[test]
fn an_alarm_at_a_set_moment_goes_off_then() {
    let lines = format!("DTSTART:20261112T080000Z\r\n{}", alarm("TRIGGER;VALUE=DATE-TIME:20261111T170000Z", "DISPLAY"));
    assert_eq!(times(&event(&lines), utc(2026, 11, 5, 9, 0)), [utc(2026, 11, 11, 17, 0)]);
}

#[test]
fn an_event_with_only_mail_alarms_alerts_nothing_here() {
    let lines = format!("DTSTART:20261112T080000Z\r\n{}", alarm("TRIGGER:-PT1H", "EMAIL"));
    assert!(times(&event(&lines), utc(2026, 11, 5, 9, 0)).is_empty());
}

#[test]
fn a_daily_event_keeps_its_alert_at_the_same_wall_clock_time_across_the_change() {
    // Nine o'clock on the computer's clock every day from Friday 23 October 2026; the clock goes back on Sunday 25th.
    let daily = event("DTSTART:20261023T090000\r\nDURATION:PT1H\r\nRRULE:FREQ=DAILY;COUNT=4\r\n");
    let alerts = times(&daily, utc(2026, 10, 22, 12, 0));
    assert_eq!(alerts, [utc(2026, 10, 23, 6, 50), utc(2026, 10, 24, 6, 50), utc(2026, 10, 25, 7, 50), utc(2026, 10, 26, 7, 50)]);
}

#[test]
fn a_day_before_follows_the_wall_clock_so_it_is_25_hours_when_the_clock_goes_back_between() {
    let lines = format!(
        "{BUDAPEST}BEGIN:VEVENT\r\nUID:z@example.org\r\nDTSTART;TZID=Europe/Budapest:20261025T120000\r\nDURATION:PT1H\r\n{}END:VEVENT\r\n",
        alarm("TRIGGER:-P1D", "DISPLAY")
    );
    let zoned = calendar(&lines);
    // Noon on Sunday 25th is 11:00 UTC (+01:00); noon on Saturday 24th is 10:00 UTC (+02:00).
    assert_eq!(upcoming(&zoned, utc(2026, 10, 20, 0, 0), &Offset::UTC, None), [utc(2026, 10, 24, 10, 0)]);
}

#[test]
fn an_alert_in_the_hour_the_clock_skips_lands_after_it() {
    // A day before 02:50 on Monday 30 March 2026 is 02:50 on Sunday 29th, an hour the clock skips: it is read as
    // 03:50 (+02:00), as far after the change as it was meant after two.
    let lines = format!("DTSTART:20260330T025000\r\nDURATION:PT10M\r\n{}", alarm("TRIGGER:-P1D", "DISPLAY"));
    assert_eq!(times(&event(&lines), utc(2026, 3, 20, 0, 0)), [utc(2026, 3, 29, 1, 50)]);
}

#[test]
fn only_alerts_after_now_and_within_the_horizon_count() {
    let daily = event("DTSTART:20261101T090000Z\r\nDURATION:PT1H\r\nRRULE:FREQ=DAILY\r\n");
    let now = utc(2026, 11, 5, 8, 50);
    let alerts = times(&daily, now);
    assert_eq!(alerts.first(), Some(&utc(2026, 11, 6, 8, 50)), "the alert of now went off already");
    assert!(alerts.iter().all(|at| *at > now && at.duration_since(now).unwrap() <= HORIZON));
    assert_eq!(alerts.len(), 14);
}

#[test]
fn an_event_keeps_at_most_max_alerts() {
    let lines = format!("DTSTART:20261101T090000Z\r\nDURATION:PT1M\r\nRRULE:FREQ=HOURLY\r\n{}", alarm("TRIGGER:-PT1M", "DISPLAY"));
    let alerts = times(&event(&lines), utc(2026, 11, 5, 0, 0));
    assert_eq!(alerts.len(), MAX_ALERTS);
    assert_eq!(alerts.first(), Some(&utc(2026, 11, 5, 0, 59)));
}

#[test]
fn a_cancelled_event_and_one_the_owner_declined_alert_nothing() {
    let now = utc(2026, 11, 5, 9, 0);
    let cancelled = event("DTSTART:20261112T080000Z\r\nSTATUS:CANCELLED\r\n");
    assert!(times(&cancelled, now).is_empty());
    let declined = event("DTSTART:20261112T080000Z\r\nATTENDEE;PARTSTAT=DECLINED:mailto:Anna@Example.org\r\n");
    assert!(upcoming(&declined, now, &europe(), Some("anna@example.org")).is_empty());
    assert_eq!(upcoming(&declined, now, &europe(), Some("bob@example.org")), [utc(2026, 11, 12, 7, 50)]);
}

#[test]
fn a_changed_instance_alerts_at_its_own_time_with_its_own_alarms() {
    let lines = format!(
        "BEGIN:VEVENT\r\nUID:w@example.org\r\nDTSTART:20261109T090000Z\r\nDURATION:PT1H\r\nRRULE:FREQ=WEEKLY;COUNT=2\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:w@example.org\r\nRECURRENCE-ID:20261116T090000Z\r\nDTSTART:20261116T130000Z\r\nDURATION:PT1H\r\n{}END:VEVENT\r\n",
        alarm("TRIGGER:-PT1H", "DISPLAY")
    );
    assert_eq!(times(&calendar(&lines), utc(2026, 11, 5, 9, 0)), [utc(2026, 11, 9, 8, 50), utc(2026, 11, 16, 12, 0)]);
}

#[test]
fn rings_name_the_occurrence_each_alert_is_for() {
    let daily = event("DTSTART:20261101T090000Z\r\nDURATION:PT1H\r\nRRULE:FREQ=DAILY\r\n");
    let found = rings(&daily, utc(2026, 11, 6, 8, 49), utc(2026, 11, 6, 8, 50), &europe(), None);
    assert_eq!(found.len(), 1);
    let ring = &found[0];
    assert_eq!((ring.at, ring.start, ring.end, ring.event), (utc(2026, 11, 6, 8, 50), utc(2026, 11, 6, 9, 0), utc(2026, 11, 6, 10, 0), 0));
}

#[test]
fn an_alarm_further_than_a_year_from_its_event_is_left_out() {
    let lines = format!("DTSTART:20261112T080000Z\r\n{}{}", alarm("TRIGGER:-P400D", "DISPLAY"), alarm("TRIGGER:-PT9999999999S", "DISPLAY"));
    assert!(times(&event(&lines), utc(2025, 1, 1, 0, 0)).is_empty());
}
