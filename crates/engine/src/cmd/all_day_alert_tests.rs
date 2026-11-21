// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `event.all_day_alert.set`: the hour a new all-day event's alert rings, written as its alarm's trigger and rung
//! here at that hour; a timed event made all-day takes it in place of the timed default; events already written keep
//! their alarms.

use std::time::Duration;

use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, CalendarReport, Event, EventKind, FetchedEvent};
use inboxcraft_time::{CivilDateTime, FakeClock, Offset, OffsetTable, Timestamp};
use serde_json::{Value, json};

use super::calendar_tests::{WORK, object, session_at};
use crate::all_day_alert::AllDayAlert;
use crate::notify::Notice;
use crate::test_support::{NOW, at};
use crate::{Caller, Session};

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

/// A session on Thursday 5 November 2026 at 09:00 UTC, on a Central European clock.
fn session() -> Session {
    let mut session = session_at(&FakeClock::new(at(NOW)));
    session.set_offsets(Box::new(europe()));
    session
}

fn text(session: &Session, id: u64) -> String {
    session.store().calendar_event(CalendarEventId::new(id)).unwrap().unwrap().ical
}

fn alerts(session: &Session, id: u64) -> Vec<Timestamp> {
    session.store().alerts(CalendarEventId::new(id)).unwrap().into_iter().map(|alert| alert.at).collect()
}

fn create(session: &mut Session, params: Value) -> u64 {
    session.run("event.create", &params).unwrap()["id"].as_u64().unwrap()
}

/// A day off on Tuesday 10 November 2026, with no alert picked.
fn day_off(session: &mut Session) -> u64 {
    create(session, json!({ "calendar": 1, "summary": "Day off", "start": "2026-11-10", "all_day": true }))
}

fn set(session: &mut Session, at: &str) -> Value {
    session.run("event.all_day_alert.set", &json!({ "at": at })).unwrap()
}

#[test]
fn each_choice_writes_its_trigger_and_rings_at_its_hour_on_the_owners_clock() {
    // Midnight of the 10th in Budapest is 23:00 UTC on the 9th.
    let cases = [
        ("day_before_9", "TRIGGER:-PT15H\r\n", utc(2026, 11, 9, 8, 0)),
        ("day_before_12", "TRIGGER:-PT12H\r\n", utc(2026, 11, 9, 11, 0)),
        ("day_before_18", "TRIGGER:-PT6H\r\n", utc(2026, 11, 9, 17, 0)),
        ("on_the_day_9", "TRIGGER:PT9H\r\n", utc(2026, 11, 10, 8, 0)),
    ];
    for (name, trigger, rings) in cases {
        let mut session = session();
        let answer = set(&mut session, name);
        assert_eq!(answer["all_day_alert"], json!(name));
        assert_eq!(session.all_day_alert().name(), name);
        let id = day_off(&mut session);
        let written = text(&session, id);
        assert!(written.contains(trigger), "{name}: {written}");
        assert!(written.contains("ACTION:DISPLAY\r\n"), "{name}: {written}");
        assert_eq!(alerts(&session, id), [rings], "{name}");
    }
}

#[test]
fn the_default_is_nine_the_day_before_and_a_timed_event_keeps_ten_minutes() {
    let mut session = session();
    assert_eq!(session.all_day_alert(), AllDayAlert::DayBefore9);
    let id = day_off(&mut session);
    assert!(text(&session, id).contains("TRIGGER:-PT15H\r\n"), "{}", text(&session, id));
    set(&mut session, "day_before_18");
    let timed = create(&mut session, json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-06T14:00:00Z" }));
    assert!(text(&session, timed).contains("TRIGGER:-PT10M\r\n"), "{}", text(&session, timed));
    let picked = create(&mut session, json!({ "calendar": 1, "summary": "Trip", "start": "2026-11-11", "all_day": true, "alert": 1440 }));
    assert!(text(&session, picked).contains("TRIGGER:-P1D\r\n"), "an alert the owner picks wins: {}", text(&session, picked));
}

#[test]
fn the_hour_rings_on_the_day_the_clock_is_put_forward() {
    // Friday 20 March 2026; the clock goes from 02:00 to 03:00 on Sunday 29 March.
    let clock = FakeClock::new(utc(2026, 3, 20, 9, 0));
    let mut session = session_at(&clock);
    session.set_offsets(Box::new(europe()));
    set(&mut session, "on_the_day_9");
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Easter walk", "start": "2026-03-29", "all_day": true }));
    assert!(text(&session, id).contains("TRIGGER:PT9H\r\n"), "{}", text(&session, id));
    assert_eq!(alerts(&session, id), [utc(2026, 3, 29, 7, 0)], "09:00 summer time");
}

#[test]
fn a_timed_event_made_all_day_trades_the_timed_default_for_the_hour_chosen() {
    let mut session = session();
    set(&mut session, "day_before_12");
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-06T14:00:00Z" }));
    assert!(text(&session, id).contains("TRIGGER:-PT10M\r\n"));
    session.run("event.update", &json!({ "id": id, "all_day": true, "start": "2026-11-10" })).unwrap();
    let written = text(&session, id);
    assert!(written.contains("TRIGGER:-PT12H\r\n"), "{written}");
    assert!(!written.contains("TRIGGER:-PT10M"), "{written}");
    assert_eq!(written.matches("BEGIN:VALARM").count(), 1, "{written}");
    assert_eq!(alerts(&session, id), [utc(2026, 11, 9, 11, 0)]);
}

#[test]
fn a_custom_alarm_is_kept_when_the_event_is_made_all_day() {
    let mut session = session();
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-06T14:00:00Z", "alert": 30 }));
    session.run("event.update", &json!({ "id": id, "all_day": true, "start": "2026-11-10" })).unwrap();
    assert!(text(&session, id).contains("TRIGGER:-PT30M\r\n"), "{}", text(&session, id));
    // An alert given with the switch wins over both.
    let other = create(&mut session, json!({ "calendar": 1, "summary": "Call", "start": "2026-11-06T15:00:00Z" }));
    session.run("event.update", &json!({ "id": other, "all_day": true, "start": "2026-11-10", "alert": 60 })).unwrap();
    assert!(text(&session, other).contains("TRIGGER:-PT1H\r\n"), "{}", text(&session, other));
    // An all-day event moved to another day keeps the alarm it has, whatever the setting says now.
    let day = day_off(&mut session);
    set(&mut session, "day_before_18");
    session.run("event.update", &json!({ "id": day, "start": "2026-11-11" })).unwrap();
    assert!(text(&session, day).contains("TRIGGER:-PT15H\r\n"), "{}", text(&session, day));
}

#[test]
fn events_already_written_keep_their_alarms_when_the_hour_changes() {
    let mut session = session();
    let id = day_off(&mut session);
    let before = text(&session, id);
    set(&mut session, "on_the_day_9");
    assert_eq!(text(&session, id), before);
    assert_eq!(alerts(&session, id), [utc(2026, 11, 9, 8, 0)]);
}

/// An all-day event from the server on Tuesday 10 November 2026, without any alarm.
fn synced_without_alarms(session: &mut Session) -> u64 {
    let ical = object("BEGIN:VEVENT\r\nUID:holiday@example.org\r\nDTSTART;VALUE=DATE:20261110\r\nSUMMARY:Holiday\r\nEND:VEVENT\r\n");
    let fetched = FetchedEvent { url: format!("{WORK}holiday.ics"), etag: Some("\"h1\"".to_owned()), ical };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    let list = session.run("event.list", &json!({ "from": "2026-11-09", "to": "2026-11-12" })).unwrap();
    let rows = list["events"].as_array().unwrap();
    rows.iter().find(|row| row["summary"] == json!("Holiday")).unwrap()["id"].as_u64().unwrap()
}

#[test]
fn an_all_day_event_without_alarms_rings_here_at_the_hour_and_follows_a_change() {
    let clock = FakeClock::new(at(NOW));
    let mut session = session_at(&clock);
    session.set_offsets(Box::new(europe()));
    session.start();
    let id = synced_without_alarms(&mut session);
    assert_eq!(alerts(&session, id), [utc(2026, 11, 9, 8, 0)], "09:00 the day before");
    set(&mut session, "day_before_18");
    session.tick().unwrap();
    assert_eq!(alerts(&session, id), [utc(2026, 11, 9, 17, 0)], "the next tick moves it to 18:00");
}

#[test]
fn what_is_written_comes_back_from_the_server_with_its_hour() {
    let mut session = session();
    set(&mut session, "day_before_18");
    let id = day_off(&mut session);
    let written = text(&session, id);
    // The server hands the same text back under another name, as a second device would see it.
    let fetched =
        FetchedEvent { url: format!("{WORK}copy.ics"), etag: Some("\"c1\"".to_owned()), ical: written.replace("@inboxcraft", "-copy@inboxcraft") };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    let list = session.run("event.list", &json!({ "from": "2026-11-09", "to": "2026-11-12" })).unwrap();
    let rows: Vec<&Value> = list["events"].as_array().unwrap().iter().filter(|row| row["summary"] == json!("Day off")).collect();
    assert_eq!(rows.len(), 2, "{list}");
    for row in rows {
        assert_eq!(row["alert"], json!(360), "six hours before midnight: {row}");
        let copy = row["id"].as_u64().unwrap();
        assert_eq!(alerts(&session, copy), [utc(2026, 11, 9, 17, 0)]);
    }
}

#[test]
fn the_setting_is_the_owners_and_takes_only_its_choices() {
    let mut session = session();
    let refused = session.run_as(Caller::Agent, "event.all_day_alert.set", &json!({ "at": "day_before_18" })).unwrap_err();
    assert_eq!(refused.code(), "not_allowed");
    for params in [
        json!({}),
        json!({ "at": "noon" }),
        json!({ "at": 9 }),
        json!({ "at": "DAY_BEFORE_9" }),
        json!({ "at": "day_before_9", "hour": 9 }),
        json!([]),
    ] {
        assert_eq!(session.run("event.all_day_alert.set", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert_eq!(session.all_day_alert(), AllDayAlert::DayBefore9, "nothing changed");
    let answer = set(&mut session, "on_the_day_9");
    assert_eq!(answer["choices"], json!(["day_before_9", "day_before_12", "day_before_18", "on_the_day_9"]));
    // Kept from an earlier run and handed back at the start.
    let mut next = self::session();
    next.set_all_day_alert(crate::all_day_alert::from_value(&crate::all_day_alert::to_value(session.all_day_alert())));
    let id = day_off(&mut next);
    assert!(text(&next, id).contains("TRIGGER:PT9H\r\n"));
}

/// The alert notices of event `id` the owner was told since the last call.
fn told(session: &mut Session, id: u64) -> usize {
    let id = CalendarEventId::new(id);
    session.take_notices().into_iter().filter(|notice| matches!(notice, Notice::Alert { about, .. } if about.event == id)).count()
}

#[test]
fn a_choice_kept_from_an_earlier_run_rings_from_the_first_sweep_on() {
    // The front end hands the kept choice back after the start's sweep: the next tick works the alerts out again.
    let clock = FakeClock::new(at(NOW));
    let mut session = session_at(&clock);
    session.set_offsets(Box::new(europe()));
    session.start();
    let id = synced_without_alarms(&mut session);
    session.tick().unwrap();
    assert_eq!(alerts(&session, id), [utc(2026, 11, 9, 8, 0)], "the start sweeps with the default");
    session.set_all_day_alert(AllDayAlert::DayBefore18);
    session.tick().unwrap();
    assert_eq!(alerts(&session, id), [utc(2026, 11, 9, 17, 0)], "the kept choice holds from the next tick");
    clock.set(utc(2026, 11, 9, 17, 0));
    session.tick().unwrap();
    assert_eq!(told(&mut session, id), 1, "it rings at 18:00 on the owner's clock");
}

#[test]
fn a_change_of_choice_never_loses_the_next_ring_still_ahead() {
    // Monday 9 November at 10:00 in Budapest; the holiday is on Tuesday, its 18:00 ring ahead.
    for (choice, rings) in [("day_before_12", utc(2026, 11, 9, 11, 0)), ("on_the_day_9", utc(2026, 11, 10, 8, 0))] {
        let clock = FakeClock::new(utc(2026, 11, 9, 9, 0));
        let mut session = session_at(&clock);
        session.set_offsets(Box::new(europe()));
        set(&mut session, "day_before_18");
        session.start();
        let id = synced_without_alarms(&mut session);
        session.tick().unwrap();
        assert_eq!(alerts(&session, id), [utc(2026, 11, 9, 17, 0)], "{choice}");
        set(&mut session, choice);
        session.tick().unwrap();
        assert_eq!(alerts(&session, id), [rings], "{choice}");
        clock.set(rings);
        session.tick().unwrap();
        assert_eq!(told(&mut session, id), 1, "{choice}: the new hour rings");
        clock.advance(Duration::from_secs(86_400));
        session.tick().unwrap();
        assert_eq!(told(&mut session, id), 0, "{choice}: and nothing else, the old hour neither");
    }
}

#[test]
fn an_all_day_event_made_timed_trades_any_of_the_hours_for_the_timed_default() {
    for choice in AllDayAlert::ALL {
        let mut session = session();
        set(&mut session, choice.name());
        let id = day_off(&mut session);
        session.run("event.update", &json!({ "id": id, "all_day": false, "start": "2026-11-10T13:00:00Z" })).unwrap();
        let written = text(&session, id);
        assert!(written.contains("TRIGGER:-PT10M\r\n"), "{}: {written}", choice.name());
        assert_eq!(written.matches("BEGIN:VALARM").count(), 1, "{written}");
        assert_eq!(alerts(&session, id), [utc(2026, 11, 10, 12, 50)], "{}", choice.name());
    }
}

#[test]
fn an_all_day_event_made_timed_keeps_an_alarm_the_owner_set() {
    let mut session = session();
    for minutes in [1440, 180] {
        let id = create(&mut session, json!({ "calendar": 1, "summary": "Trip", "start": "2026-11-10", "all_day": true, "alert": minutes }));
        let before = text(&session, id);
        let trigger = before.lines().find(|line| line.starts_with("TRIGGER")).unwrap().to_owned();
        session.run("event.update", &json!({ "id": id, "all_day": false, "start": "2026-11-10T13:00:00Z" })).unwrap();
        let written = text(&session, id);
        assert!(written.contains(&trigger), "{trigger}: {written}");
        assert!(!written.contains("TRIGGER:-PT10M"), "{written}");
    }
}

#[test]
fn timed_to_all_day_and_back_comes_home_to_the_timed_default() {
    let mut session = session();
    set(&mut session, "day_before_12");
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-06T14:00:00Z" }));
    session.run("event.update", &json!({ "id": id, "all_day": true, "start": "2026-11-10" })).unwrap();
    assert!(text(&session, id).contains("TRIGGER:-PT12H\r\n"), "{}", text(&session, id));
    // The setting changed since: the hour written is still one of its hours, so it goes back all the same.
    set(&mut session, "on_the_day_9");
    session.run("event.update", &json!({ "id": id, "all_day": false, "start": "2026-11-10T13:00:00Z" })).unwrap();
    let written = text(&session, id);
    assert!(written.contains("TRIGGER:-PT10M\r\n"), "{written}");
    assert_eq!(written.matches("BEGIN:VALARM").count(), 1, "{written}");
}

#[test]
fn only_the_series_the_edit_makes_all_day_trades_its_alarm() {
    let mut session = session();
    // A weekly walk, with its 17 November already a whole day of its own, keeping ten minutes.
    let alarm = "BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Walk\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\n";
    let ical = object(&format!(
        "BEGIN:VEVENT\r\nUID:walk@example.org\r\nDTSTART:20261110T080000Z\r\nDTEND:20261110T090000Z\r\nRRULE:FREQ=WEEKLY;COUNT=3\r\nSUMMARY:Walk\r\n{alarm}END:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:walk@example.org\r\nRECURRENCE-ID:20261117T080000Z\r\nDTSTART;VALUE=DATE:20261117\r\nDTEND;VALUE=DATE:20261118\r\nSUMMARY:Walk day\r\n{alarm}END:VEVENT\r\n"
    ));
    let fetched = FetchedEvent { url: format!("{WORK}walk.ics"), etag: Some("\"w1\"".to_owned()), ical };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    let list = session.run("event.list", &json!({ "from": "2026-11-09", "to": "2026-11-12" })).unwrap();
    let id = list["events"].as_array().unwrap().iter().find(|row| row["summary"] == json!("Walk")).unwrap()["id"].as_u64().unwrap();
    session.run("event.update", &json!({ "id": id, "all_day": true, "start": "2026-11-10" })).unwrap();
    let written = text(&session, id);
    assert_eq!(written.matches("TRIGGER:-PT15H\r\n").count(), 1, "the series: {written}");
    assert_eq!(written.matches("TRIGGER:-PT10M\r\n").count(), 1, "the day already whole keeps its own: {written}");
}
