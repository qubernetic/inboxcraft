// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A full `event.update` keeps the form of the event's times (RFC 5545 §3.3.5): a series in UTC stays in UTC, one in
//! a zone stays in its zone, a floating one stays floating, whole days stay days; a time the owner did not change is
//! written as it came, and the changed instances keep theirs.

use inboxcraft_ical::{Calendar, When};
use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, CalendarJob, CalendarReport, Event, EventKind, FetchedEvent, JobKind};
use inboxcraft_time::{CivilDateTime, FakeClock, Offset, Timestamp};
use serde_json::{Value, json};

use super::calendar_tests::{WORK, object, session_at};
use crate::Session;
use crate::test_support::{NOW, at};

fn session() -> Session {
    session_at(&FakeClock::new(at(NOW)))
}

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap().to_timestamp(Offset::UTC).unwrap()
}

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap()
}

fn held(session: &Session, id: u64) -> Calendar {
    Calendar::parse(text(session, id).as_bytes()).unwrap()
}

/// The text event `id` holds, unfolded.
fn text(session: &Session, id: u64) -> String {
    session.store().calendar_event(CalendarEventId::new(id)).unwrap().unwrap().ical.replace("\r\n ", "")
}

/// Event `name` as the server sends it, with entity tag `etag`; the first new one is event 5.
fn synced(session: &mut Session, name: &str, etag: &str, ical: String) {
    let fetched = FetchedEvent { url: format!("{WORK}{name}.ics"), etag: Some(etag.to_owned()), ical };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
}

/// A daily check-in from Friday 6 November 2026 at 9:00 UTC for a quarter of an hour, three times: Saturday's moved
/// to 10:00, Sunday's to 9:30.
fn checkin() -> String {
    object(
        "BEGIN:VEVENT\r\nUID:checkin@example.org\r\nDTSTAMP:20261101T080000Z\r\nDTSTART:20261106T090000Z\r\nDURATION:PT15M\r\n\
         SUMMARY:Check-in\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:checkin@example.org\r\nRECURRENCE-ID:20261107T090000Z\r\nDTSTAMP:20261101T080000Z\r\n\
         DTSTART:20261107T100000Z\r\nDURATION:PT15M\r\nSUMMARY:Check-in\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:checkin@example.org\r\nRECURRENCE-ID:20261108T090000Z\r\nDTSTAMP:20261101T080000Z\r\n\
         DTSTART:20261108T093000Z\r\nDURATION:PT15M\r\nSUMMARY:Check-in\r\nEND:VEVENT\r\n",
    )
}

/// Evening yoga on the wall clock: Mondays at 18:00 for an hour, from Monday 9 November 2026, Tuesday 17th's moved.
fn yoga() -> String {
    object(
        "BEGIN:VEVENT\r\nUID:yoga@example.org\r\nDTSTART:20261109T180000\r\nDURATION:PT1H\r\nSUMMARY:Yoga\r\n\
         RRULE:FREQ=WEEKLY;COUNT=4\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:yoga@example.org\r\nRECURRENCE-ID:20261116T180000\r\nDTSTART:20261117T180000\r\n\
         DURATION:PT1H\r\nSUMMARY:Yoga\r\nEND:VEVENT\r\n",
    )
}

/// The `RECURRENCE-ID` lines of `text`.
fn recurrence_ids(text: &str) -> Vec<&str> {
    text.lines().filter(|line| line.starts_with("RECURRENCE-ID")).collect()
}

/// Whether a `DTSTART` or `RECURRENCE-ID` value is in UTC, in a zone, or floating.
fn form(when: &When) -> &'static str {
    match when {
        When::Utc(_) => "utc",
        When::Zoned(..) => "zoned",
        When::Floating(_) => "floating",
        When::Date(_) => "date",
    }
}

#[test]
fn a_full_edit_of_a_utc_series_writes_its_times_as_they_came() {
    let mut session = session();
    synced(&mut session, "checkin", "\"k1\"", checkin());
    session.run("event.update", &json!({ "id": 5, "summary": "Morning check-in", "location": "Room 2", "busy": true })).unwrap();
    let written = text(&session, 5);
    assert!(written.contains("\r\nDTSTART:20261106T090000Z\r\nDURATION:PT15M\r\n"), "{written}");
    assert!(!written.contains("DTEND"), "{written}");
    assert_eq!(held(&session, 5).events[0].summary.as_deref(), Some("Morning check-in"));
}

#[test]
fn a_utc_series_whose_time_changes_stays_in_utc_at_the_right_moment() {
    let mut session = session();
    synced(&mut session, "checkin", "\"k1\"", checkin());
    // 9:30 on the computer's clock (+01:00) is 8:30 UTC; the quarter of an hour stays.
    session.run("event.update", &json!({ "id": 5, "start": "2026-11-06T09:30:00+01:00" })).unwrap();
    let series = &held(&session, 5).events[0];
    assert_eq!(series.start, When::Utc(utc(2026, 11, 6, 8, 30)));
    assert_eq!(series.end, Some(When::Utc(utc(2026, 11, 6, 8, 45))));
    // A new repeat on a series that already repeats keeps it in UTC too.
    session.run("event.update", &json!({ "id": 5, "repeat": "weekly", "end": "2026-11-06T09:00:00Z" })).unwrap();
    let series = &held(&session, 5).events[0];
    assert_eq!((series.start.clone(), series.end.clone()), (When::Utc(utc(2026, 11, 6, 8, 30)), Some(When::Utc(utc(2026, 11, 6, 9, 0)))));
}

#[test]
fn the_changed_instances_keep_their_form_and_match_the_series() {
    let mut session = session();
    synced(&mut session, "checkin", "\"k1\"", checkin());
    for change in [json!({ "id": 5, "summary": "Check-in, short" }), json!({ "id": 5, "start": "2026-11-06T09:15:00Z" })] {
        session.run("event.update", &change).unwrap();
        let written = text(&session, 5);
        assert_eq!(recurrence_ids(&written), ["RECURRENCE-ID:20261107T090000Z", "RECURRENCE-ID:20261108T090000Z"], "{change}");
        assert!(written.contains("DTSTART:20261107T100000Z") && written.contains("DTSTART:20261108T093000Z"), "{written}");
        let calendar = held(&session, 5);
        let series = form(&calendar.events[0].start);
        for instance in &calendar.events[1..] {
            assert_eq!(form(instance.recurrence_id.as_ref().unwrap()), series, "{change}: {written}");
        }
    }
    // The same for a floating series.
    synced(&mut session, "yoga", "\"y1\"", yoga());
    session.run("event.update", &json!({ "id": 6, "start": "2026-11-09T18:30:00+01:00" })).unwrap();
    let written = text(&session, 6);
    assert_eq!(recurrence_ids(&written), ["RECURRENCE-ID:20261116T180000"], "{written}");
    assert_eq!(held(&session, 6).events[0].start, When::Floating(civil(2026, 11, 9, 18, 30)));
}

#[test]
fn a_zoned_series_keeps_its_zone() {
    let mut session = session();
    session.run("event.update", &json!({ "id": 1, "summary": "Daily stand-up" })).unwrap();
    let written = text(&session, 1);
    assert!(written.contains("\r\nDTSTART;TZID=Europe/Budapest:20261109T093000\r\nDURATION:PT15M\r\n"), "{written}");
    session.run("event.update", &json!({ "id": 1, "start": "2026-11-09T10:00:00+01:00" })).unwrap();
    let series = &held(&session, 1).events[0];
    let budapest = |hour, minute| When::Zoned(civil(2026, 11, 9, hour, minute), "Europe/Budapest".to_owned());
    assert_eq!((series.start.clone(), series.end.clone()), (budapest(10, 0), Some(budapest(10, 15))));
}

#[test]
fn a_floating_series_stays_floating() {
    let mut session = session();
    synced(&mut session, "yoga", "\"y1\"", yoga());
    session.run("event.update", &json!({ "id": 5, "summary": "Hot yoga" })).unwrap();
    let written = text(&session, 5);
    assert!(written.contains("\r\nDTSTART:20261109T180000\r\nDURATION:PT1H\r\n"), "{written}");
    session.run("event.update", &json!({ "id": 5, "end": "2026-11-09T19:30:00+01:00" })).unwrap();
    let series = &held(&session, 5).events[0];
    assert_eq!(
        (series.start.clone(), series.end.clone()),
        (When::Floating(civil(2026, 11, 9, 18, 0)), Some(When::Floating(civil(2026, 11, 9, 19, 30))))
    );
}

#[test]
fn whole_days_the_owner_did_not_change_are_written_as_they_came() {
    let mut session = session();
    session.run("event.update", &json!({ "id": 4, "summary": "Holiday" })).unwrap();
    let written = text(&session, 4);
    assert!(written.contains("\r\nDTSTART;VALUE=DATE:20261110\r\nDTEND;VALUE=DATE:20261112\r\n"), "{written}");
    // A yearly birthday written with a length of a day keeps it.
    let birthday = object(
        "BEGIN:VEVENT\r\nUID:bday@example.org\r\nDTSTART;VALUE=DATE:20261111\r\nDURATION:P1D\r\nSUMMARY:Dora\r\n\
         RRULE:FREQ=YEARLY\r\nEND:VEVENT\r\n",
    );
    synced(&mut session, "bday", "\"b1\"", birthday);
    session.run("event.update", &json!({ "id": 5, "summary": "Dora's birthday" })).unwrap();
    let written = text(&session, 5);
    assert!(written.contains("\r\nDTSTART;VALUE=DATE:20261111\r\nDURATION:P1D\r\n"), "{written}");
}

#[test]
fn what_goes_to_the_server_comes_back_at_the_same_moments() {
    let mut session = session();
    // The series alone: changed instances stay where they were when the series moves (docs/gaps.md).
    let series = checkin().split("BEGIN:VEVENT\r\nUID:checkin@example.org\r\nRECURRENCE-ID").next().unwrap().to_owned();
    synced(&mut session, "checkin", "\"k1\"", format!("{series}END:VCALENDAR\r\n"));
    session.take_jobs();
    session.run("event.update", &json!({ "id": 5, "summary": "Check-in", "start": "2026-11-06T09:30:00+01:00" })).unwrap();
    let put = session
        .take_jobs()
        .into_iter()
        .find_map(|job| match job.kind {
            JobKind::Calendar(CalendarJob::Put { ical, .. }) => Some(ical),
            _ => None,
        })
        .unwrap();
    assert!(put.contains("DTSTART:20261106T083000Z\r\nDTEND:20261106T084500Z\r\n"), "{put}");
    // The server sends the same text back on the next sync; the list shows the series at 9:30 on the computer's clock.
    synced(&mut session, "checkin", "\"k2\"", put);
    let list = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12" })).unwrap();
    let starts: Vec<&Value> = list["events"].as_array().unwrap().iter().filter(|row| row["id"] == json!(5)).map(|row| &row["local_start"]).collect();
    assert_eq!(starts, [&json!("2026-11-06T09:30:00+01:00"), &json!("2026-11-07T09:30:00+01:00"), &json!("2026-11-08T09:30:00+01:00")]);
}

#[test]
fn hostile_times_stay_bounded_through_a_full_edit() {
    let mut session = session();
    let hostile = [
        // A zone the object does not describe.
        "DTSTART;TZID=Nowhere/Unknown:20261109T093000\r\nDURATION:PT15M\r\nRRULE:FREQ=DAILY\r\n",
        // The last moment of year 9999 with a length past it.
        "DTSTART:99991231T235959Z\r\nDURATION:P9999W\r\nRRULE:FREQ=SECONDLY\r\n",
        // An end before the start.
        "DTSTART:20261109T093000Z\r\nDTEND:20261101T093000Z\r\nRRULE:FREQ=WEEKLY\r\n",
        // A length below zero, floating.
        "DTSTART:20261109T093000\r\nDURATION:-P400D\r\nRRULE:FREQ=WEEKLY\r\n",
    ];
    for (index, times) in hostile.iter().enumerate() {
        let id = 5 + u64::try_from(index).unwrap();
        let ical = object(&format!("BEGIN:VEVENT\r\nUID:h{index}@example.org\r\n{times}SUMMARY:Odd\r\nEND:VEVENT\r\n"));
        synced(&mut session, &format!("h{index}"), "\"h1\"", ical);
        for change in [
            json!({ "id": id, "summary": "Still odd" }),
            json!({ "id": id, "start": "9999-12-31T23:59:59Z" }),
            json!({ "id": id, "end": "2026-11-09T10:00:00Z" }),
            json!({ "id": id, "all_day": true, "start": "9999-12-31" }),
        ] {
            if let Err(error) = session.run("event.update", &change) {
                assert!(matches!(error.code(), "bad_params" | "unreadable" | "not_found"), "{change}: {}", error.code());
            }
        }
    }
    // A title edit keeps the unknown zone as written.
    let ical =
        object("BEGIN:VEVENT\r\nUID:z@example.org\r\nDTSTART;TZID=Nowhere/Unknown:20261109T093000\r\nDURATION:PT15M\r\nSUMMARY:Z\r\nEND:VEVENT\r\n");
    synced(&mut session, "z", "\"z1\"", ical);
    session.run("event.update", &json!({ "id": 9, "summary": "Zed" })).unwrap();
    assert!(text(&session, 9).contains("\r\nDTSTART;TZID=Nowhere/Unknown:20261109T093000\r\nDURATION:PT15M\r\n"));
}
