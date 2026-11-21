// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `calendar.import` and `calendar.import.preview`: invented `.ics` files of `example.org` added to the Work calendar
//! of the test mailbox, idempotent by UID, as first writes, one step of undo; hostile files refused whole or event by
//! event.

use inboxcraft_ical::{Calendar, End, Frequency, When};
use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, CalendarJob, CalendarReport, Event, EventKind, JobKind};
use inboxcraft_time::{CivilDateTime, FakeClock, Offset};
use proptest::prelude::*;
use serde_json::{Value, json};

use super::calendar_tests::{WORK, session_at};
use crate::calendar::alert;
use crate::test_support::{NOW, at};
use crate::{Caller, EngineError, Session};

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (session_at(&clock), clock)
}

fn calendar_jobs(session: &mut Session) -> Vec<CalendarJob> {
    session
        .take_jobs()
        .into_iter()
        .filter_map(|job| match job.kind {
            JobKind::Calendar(calendar) => Some(calendar),
            _ => None,
        })
        .collect()
}

/// The calendar object event `id` holds now.
fn object(session: &Session, id: u64) -> Calendar {
    let held = session.store().calendar_event(CalendarEventId::new(id)).unwrap().unwrap();
    Calendar::parse(held.ical.as_bytes()).unwrap()
}

/// An `.ics` file of `events` (`VEVENT` bodies without their last line break).
fn ics(events: &[&str]) -> String {
    let mut text = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Export//EN\r\n");
    for event in events {
        text.push_str(&format!("BEGIN:VEVENT\r\n{event}\r\nEND:VEVENT\r\n"));
    }
    text.push_str("END:VCALENDAR\r\n");
    text
}

fn import(session: &mut Session, events: &[&str]) -> Value {
    session.run("calendar.import", &json!({ "calendar": 1, "ics": ics(events) })).unwrap()
}

/// The first event a seeded session adds gets this id.
const FIRST: u64 = 5;

/// The computer's wall clock in the tests: +01:00.
fn local(year: i32, month: u8, day: u8, hour: u8) -> When {
    When::Floating(CivilDateTime::new(year, month, day, hour, 0, 0).unwrap())
}

const DENTIST: &str = "UID:dentist@example.org\r\nDTSTART:20261112T080000Z\r\nDTEND:20261112T090000Z\r\nSUMMARY:Dentist\r\nLOCATION:Main Street 1";
const HOLIDAY: &str = "UID:holiday@example.org\r\nDTSTART;VALUE=DATE:20261224\r\nDTEND;VALUE=DATE:20261227\r\nSUMMARY:Winter break";

#[test]
fn import_adds_each_event_under_its_uid_as_a_first_write() {
    let (mut session, _) = session();
    let before = session.history_len();
    let answer = session.run("calendar.import", &json!({ "calendar": 1, "ics": ics(&[DENTIST, HOLIDAY]) })).unwrap();
    assert_eq!((answer["added"].clone(), answer["skipped"].clone(), answer["errors_count"].clone()), (json!(2), json!(0), json!(0)));
    let jobs = calendar_jobs(&mut session);
    assert_eq!(jobs.len(), 2);
    for job in &jobs {
        let CalendarJob::Put { url, etag, ical, .. } = job else { panic!("{job:?}") };
        assert_eq!(*etag, None, "a first write: If-None-Match");
        assert!(url.starts_with(WORK) && url.ends_with(".ics") && !url.contains("dentist") && !url.contains("holiday"), "{url}");
        assert_eq!(Calendar::parse(ical.as_bytes()).unwrap().events.len(), 1);
    }
    assert_eq!(session.history_len(), before + 1, "one step of undo");
    assert_eq!(object(&session, FIRST).events[0].uid, "dentist@example.org");
    assert_eq!(object(&session, FIRST + 1).events[0].uid, "holiday@example.org");
    assert_eq!(
        answer,
        json!({ "calendar": 1, "added": 2, "skipped": 0, "single": 0, "not_supported": [], "not_supported_count": 0, "errors": [], "errors_count": 0 })
    );
    // Kept in the store's queue until the server answers.
    assert_eq!(session.store().queued_jobs().unwrap().len(), 2);
}

#[test]
fn a_second_import_adds_nothing() {
    let (mut session, _) = session();
    let params = json!({ "calendar": 1, "ics": ics(&[DENTIST, HOLIDAY]) });
    session.run("calendar.import", &params).unwrap();
    calendar_jobs(&mut session);
    let (before, again) = (session.history_len(), session.run("calendar.import", &params).unwrap());
    assert_eq!((again["added"].clone(), again["skipped"].clone()), (json!(0), json!(2)));
    assert!(calendar_jobs(&mut session).is_empty());
    assert_eq!(session.history_len(), before, "nothing added is no step");
}

#[test]
fn undo_takes_the_whole_import_back_in_one_step() {
    let (mut session, _) = session();
    session.run("calendar.import", &json!({ "calendar": 1, "ics": ics(&[DENTIST, HOLIDAY]) })).unwrap();
    let puts: Vec<_> = session.take_jobs().into_iter().map(|job| job.id).collect();
    let undone = session.run("history.undo", &json!({})).unwrap();
    assert_eq!(undone, json!({ "command": "calendar.import", "undone": true, "removed": 2 }));
    // Each delete goes out once the server has taken the write it takes back, with the tag that write got.
    for (event, job) in (FIRST..).zip(puts) {
        let written =
            CalendarReport::Written { calendar: CalendarId::new(1), event: CalendarEventId::new(event), etag: Some(format!("\"e{event}\"")) };
        session.apply(&Event::new(AccountId::new(1), Some(job), EventKind::Calendar(written))).unwrap();
    }
    let jobs = calendar_jobs(&mut session);
    assert_eq!(jobs.iter().filter(|job| matches!(job, CalendarJob::Delete { etag: Some(_), .. })).count(), 2, "{jobs:?}");
    assert_eq!(session.store().calendar_event(CalendarEventId::new(FIRST)).unwrap(), None);
    assert_eq!(session.store().calendar_events(CalendarId::new(1)).unwrap().len(), 3, "the calendar's own events stay");
}

#[test]
fn undo_after_the_events_went_away_removes_what_is_left() {
    let (mut session, _) = session();
    import(&mut session, &[DENTIST, HOLIDAY]);
    calendar_jobs(&mut session);
    // The server took one away meanwhile.
    let deleted = CalendarReport::Deleted { calendar: CalendarId::new(1), event: CalendarEventId::new(FIRST) };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(deleted))).unwrap();
    assert_eq!(session.run("history.undo", &json!({})).unwrap(), json!({ "command": "calendar.import", "undone": true, "removed": 1 }));
    // Both gone: nothing to remove, and the step is off the history either way.
    import(&mut session, &[DENTIST]);
    crate::test_support::change(&mut session, inboxcraft_store::Change::RemoveCalendar(CalendarId::new(1)));
    let before = session.history_len();
    assert_eq!(session.run("history.undo", &json!({})).unwrap(), json!({ "command": "calendar.import", "undone": false, "removed": 0 }));
    assert_eq!(session.history_len(), before - 1);
}

#[test]
fn import_is_the_owner_s_and_the_preview_changes_nothing() {
    let (mut session, _) = session();
    let params = json!({ "calendar": 1, "ics": ics(&[DENTIST]) });
    assert!(matches!(session.run_as(Caller::Agent, "calendar.import", &params), Err(EngineError::NotAllowed { .. })));
    let preview = session.run_as(Caller::Agent, "calendar.import.preview", &params).unwrap();
    assert_eq!((preview["add"].clone(), preview["first"][0]["summary"].clone()), (json!(1), json!("Dentist")));
    assert!(calendar_jobs(&mut session).is_empty());
    assert_eq!(session.history_len(), 0);
    assert_eq!(session.store().calendar_events(CalendarId::new(1)).unwrap().len(), 3);
}

/// The fields of an event `event.create` and `calendar.import` both write; the alarms without their own `UID`, which
/// names the event's.
fn written(calendar: &Calendar) -> Value {
    let event = &calendar.events[0];
    let alarms: Vec<String> = event.alarms.iter().map(|alarm| format!("{} {:?} {:?}", alarm.action, alarm.trigger, alarm.description)).collect();
    json!({
        "start": format!("{:?}", event.start),
        "end": format!("{:?}", event.end),
        "summary": event.summary,
        "location": event.location,
        "description": event.description,
        "free": event.free,
        "alarms": alarms,
        "rules": format!("{:?}", event.rules),
        "attendees": event.attendees.len(),
        "organizer": event.organizer.is_some(),
    })
}

#[test]
fn an_imported_event_is_what_event_create_writes() {
    let (mut session, _) = session();
    let timed = "UID:t@example.org\r\nDTSTART:20261112T080000Z\r\nDTEND:20261112T093000Z\r\nSUMMARY:Dentist\r\nLOCATION:Main Street 1\r\n\
                 DESCRIPTION:Bring the card\r\nTRANSP:TRANSPARENT";
    import(&mut session, &[timed, HOLIDAY]);
    let (mut made, _) = self::session();
    let create = |session: &mut Session, params: Value| session.run("event.create", &params).unwrap()["id"].as_u64().unwrap();
    let first = create(
        &mut made,
        json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-12T08:00:00Z", "end": "2026-11-12T09:30:00Z",
                "location": "Main Street 1", "description": "Bring the card", "busy": false }),
    );
    // The file's DTEND is the day after the last day; `event.create` takes the last day.
    let second = create(&mut made, json!({ "calendar": 1, "summary": "Winter break", "all_day": true, "start": "2026-12-24", "end": "2026-12-26" }));
    assert_eq!(written(&object(&session, FIRST)), written(&object(&made, first)));
    assert_eq!(written(&object(&session, FIRST + 1)), written(&object(&made, second)));
    assert_eq!(object(&session, FIRST + 1).events[0].end, Some(When::Date(inboxcraft_ical::Date::new(2026, 12, 27).unwrap())));
}

#[test]
fn times_without_an_end_follow_the_duration_or_last_an_hour_or_a_day() {
    let (mut session, _) = session();
    let answer = import(
        &mut session,
        &[
            "UID:d@example.org\r\nDTSTART:20261112T080000Z\r\nDURATION:PT45M\r\nSUMMARY:Call",
            "UID:h@example.org\r\nDTSTART:20261112T100000Z\r\nSUMMARY:Short",
            "UID:a@example.org\r\nDTSTART;VALUE=DATE:20261201\r\nSUMMARY:Day",
            "UID:w@example.org\r\nDTSTART;VALUE=DATE:20261201\r\nDURATION:P3D\r\nSUMMARY:Days",
            "UID:z@example.org\r\nDTSTART;VALUE=DATE:20261201\r\nDTEND:20261202T100000Z\r\nSUMMARY:Mixed",
        ],
    );
    assert_eq!(answer["added"], json!(4));
    assert_eq!(answer["errors"], json!([{ "index": 4, "reason": "times" }]));
    let ends: Vec<Option<When>> = (FIRST..FIRST + 4).map(|id| object(&session, id).events[0].end.clone()).collect();
    let utc = |text: &str| When::Utc(crate::params::parse_date(text).unwrap());
    let day = |d: u8| When::Date(inboxcraft_ical::Date::new(2026, 12, d).unwrap());
    assert_eq!(ends, [Some(utc("2026-11-12T08:45:00Z")), Some(utc("2026-11-12T11:00:00Z")), Some(day(2)), Some(day(4))]);
}

#[test]
fn a_uid_already_in_the_calendar_is_skipped_and_left_as_it_is() {
    let (mut session, _) = session();
    let before = session.store().calendar_event(CalendarEventId::new(2)).unwrap().unwrap();
    let answer = import(&mut session, &["UID:lunch@example.org\r\nDTSTART:20261106T110000Z\r\nSUMMARY:Another lunch"]);
    assert_eq!((answer["added"].clone(), answer["skipped"].clone()), (json!(0), json!(1)));
    assert_eq!(session.store().calendar_event(CalendarEventId::new(2)).unwrap().unwrap(), before);
    assert!(calendar_jobs(&mut session).is_empty());
    // The same UID in another calendar is no reason to skip it here.
    let home = session.run("calendar.import", &json!({ "calendar": 2, "ics": ics(&["UID:lunch@example.org\r\nDTSTART:20261106T110000Z"]) })).unwrap();
    assert_eq!(home["added"], json!(1));
}

#[test]
fn a_uid_twice_in_the_file_is_added_once() {
    let (mut session, _) = session();
    let again = "UID:dentist@example.org\r\nDTSTART:20261113T080000Z\r\nSUMMARY:Dentist again";
    let answer = import(&mut session, &[DENTIST, again]);
    assert_eq!((answer["added"].clone(), answer["skipped"].clone()), (json!(1), json!(1)));
    assert_eq!(calendar_jobs(&mut session).len(), 1);
}

#[test]
fn a_uid_whose_first_event_failed_is_added_from_its_second() {
    let (mut session, _) = session();
    let broken = "UID:dentist@example.org\r\nDTSTART:20261112T080000Z\r\nDTEND:20261112T070000Z\r\nSUMMARY:Dentist";
    let answer = import(&mut session, &[broken, DENTIST]);
    assert_eq!(
        (answer["added"].clone(), answer["skipped"].clone(), answer["errors"].clone()),
        (json!(1), json!(0), json!([{ "index": 0, "reason": "times" }]))
    );
}

#[test]
fn a_repeat_the_app_writes_is_kept_with_its_end() {
    let (mut session, _) = session();
    let answer = import(
        &mut session,
        &[
            "UID:w@example.org\r\nDTSTART:20261112T080000Z\r\nDTEND:20261112T090000Z\r\nSUMMARY:Weekly\r\nRRULE:FREQ=WEEKLY;COUNT=10\r\nEXDATE:20261119T080000Z",
            "UID:d@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Daily\r\nRRULE:FREQ=DAILY;UNTIL=20261130T080000Z",
            "UID:y@example.org\r\nDTSTART;VALUE=DATE:20261224\r\nSUMMARY:Yearly\r\nRRULE:FREQ=YEARLY;UNTIL=20301224\r\nEXDATE;VALUE=DATE:20271224",
            "UID:e@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Lost\r\nRRULE:FREQ=WEEKLY\r\nEXDATE;TZID=Nowhere/Land:00000101T000000",
        ],
    );
    assert_eq!((answer["added"].clone(), answer["single"].clone()), (json!(3), json!(0)));
    let weekly = &object(&session, FIRST).events[0];
    assert_eq!(weekly.rules.len(), 1);
    assert_eq!((weekly.rules[0].frequency, weekly.rules[0].end.clone()), (Frequency::Weekly, End::Count(10)));
    // A repeating timed event is written on the computer's wall clock, its exceptions too.
    assert_eq!(weekly.start, local(2026, 11, 12, 9));
    assert_eq!(weekly.exceptions, [local(2026, 11, 19, 9)]);
    let daily = &object(&session, FIRST + 1).events[0];
    assert_eq!((daily.rules[0].frequency, daily.rules[0].end.clone()), (Frequency::Daily, End::Until(local(2026, 11, 30, 9))));
    let yearly = &object(&session, FIRST + 2).events[0];
    let date = |year: i32| When::Date(inboxcraft_ical::Date::new(year, 12, 24).unwrap());
    assert_eq!((yearly.rules[0].end.clone(), yearly.exceptions.clone()), (End::Until(date(2030)), vec![date(2027)]));
    // An exception that cannot be placed is no guess either.
    assert_eq!(answer["errors"], json!([{ "index": 3, "reason": "times" }]));
}

#[test]
fn a_repeat_the_app_does_not_write_is_added_once_and_counted() {
    let (mut session, _) = session();
    let answer = import(
        &mut session,
        &[
            "UID:f@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Fortnightly\r\nRRULE:FREQ=WEEKLY;INTERVAL=2",
            "UID:r@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Twice\r\nRDATE:20261120T080000Z",
        ],
    );
    assert_eq!((answer["added"].clone(), answer["single"].clone()), (json!(2), json!(2)));
    assert_eq!(answer["not_supported"], json!([{ "index": 0, "reason": "repeat" }, { "index": 1, "reason": "repeat" }]));
    assert_eq!(answer["not_supported_count"], json!(2));
    for id in [FIRST, FIRST + 1] {
        let event = &object(&session, id).events[0];
        assert!(event.rules.is_empty() && event.dates.is_empty(), "{event:?}");
        // Once: a moment in UTC, as `event.create` writes a single event.
        assert!(matches!(event.start, When::Utc(_)));
    }
}

#[test]
fn a_changed_instance_is_left_out_and_counted() {
    let (mut session, _) = session();
    let changed = "UID:w@example.org\r\nRECURRENCE-ID:20261119T080000Z\r\nDTSTART:20261119T100000Z\r\nSUMMARY:Moved";
    let answer = import(&mut session, &[changed]);
    assert_eq!(answer["added"], json!(0));
    assert_eq!(answer["not_supported"], json!([{ "index": 0, "reason": "changed_instance" }]));
    assert!(calendar_jobs(&mut session).is_empty());
    let preview = session.run("calendar.import.preview", &json!({ "calendar": 1, "ics": ics(&[changed]) })).unwrap();
    assert_eq!(preview["left_out"], json!(1));
}

#[test]
fn an_alarm_the_app_knows_is_kept_and_any_other_gives_the_default() {
    let (mut session, _) = session();
    let alarm = |action: &str, trigger: &str| format!("BEGIN:VALARM\r\nACTION:{action}\r\nTRIGGER{trigger}\r\nDESCRIPTION:Ring\r\nEND:VALARM");
    let thirty = format!("UID:a@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Thirty\r\n{}", alarm("DISPLAY", ":-PT30M"));
    let email = format!(
        "UID:b@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Mail\r\n{}",
        alarm("EMAIL", ";VALUE=DATE-TIME:20261111T080000Z\r\nSUMMARY:Ring\r\nATTENDEE:mailto:anna@example.org")
    );
    let noon = format!("UID:c@example.org\r\nDTSTART;VALUE=DATE:20261224\r\nSUMMARY:Noon\r\n{}", alarm("DISPLAY", ":-PT12H"));
    let far = format!("UID:d@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Far\r\n{}", alarm("DISPLAY", ":-P8D"));
    let holiday = "UID:e@example.org\r\nDTSTART;VALUE=DATE:20261225\r\nSUMMARY:Holiday";
    assert_eq!(import(&mut session, &[&thirty, &email, &noon, &far, holiday])["added"], json!(5));
    let (mut made, _) = self::session();
    let default =
        made.run("event.create", &json!({ "calendar": 1, "summary": "x", "start": "2026-11-12T08:00:00Z" })).unwrap()["id"].as_u64().unwrap();
    let all_day =
        made.run("event.create", &json!({ "calendar": 1, "summary": "x", "all_day": true, "start": "2026-12-25" })).unwrap()["id"].as_u64().unwrap();
    let lead = |session: &Session, id: u64| alert::lead_of(&object(session, id).events[0]);
    assert_eq!(lead(&session, FIRST), json!(30));
    assert_eq!(lead(&session, FIRST + 1), lead(&made, default));
    assert_eq!(lead(&session, FIRST + 2), json!("day_before_12"));
    assert_eq!(lead(&session, FIRST + 3), lead(&made, default), "further than a week");
    assert_eq!(lead(&session, FIRST + 4), lead(&made, all_day));
    assert_eq!(lead(&made, default), json!(10));
}

#[test]
fn guests_are_not_carried_and_nobody_is_mailed() {
    let (mut session, _) = session();
    let invited = "UID:g@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Meeting\r\nORGANIZER:mailto:dora@example.com\r\n\
                   ATTENDEE;CN=Anna:mailto:anna@example.org\r\nATTENDEE:mailto:carl@example.net";
    import(&mut session, &[invited]);
    let jobs = calendar_jobs(&mut session);
    let [CalendarJob::Put { ical, .. }] = jobs.as_slice() else { panic!("{jobs:?}") };
    assert!(!ical.contains("ATTENDEE") && !ical.contains("ORGANIZER"), "{ical}");
    assert!(session.take_jobs().is_empty(), "no mail");
}

#[test]
fn each_event_that_cannot_be_written_is_an_error_of_its_own() {
    let (mut session, _) = session();
    let start = "DTSTART:20261112T080000Z";
    let long_uid = format!("UID:{}\r\n{start}", "u".repeat(256));
    let bell = format!("UID:bell\u{7}@example.org\r\n{start}");
    let summary = format!("UID:s@example.org\r\n{start}\r\nSUMMARY:{}", "s".repeat(1001));
    let location = format!("UID:l@example.org\r\n{start}\r\nLOCATION:{}", "l".repeat(1001));
    let description = format!("UID:x@example.org\r\n{start}\r\nDESCRIPTION:{}", "d".repeat(20_001));
    let events = [
        start.to_owned(),
        long_uid,
        bell,
        format!("UID:b@example.org\r\n{start}\r\nDTEND:20261112T070000Z"),
        format!("UID:t@example.org\r\n{start}\r\nDTEND:20271218T080000Z"),
        summary,
        description,
        "UID:u@example.org\r\nDTSTART:20261340T990000Z".to_owned(),
        location,
        DENTIST.to_owned(),
    ];
    let events: Vec<&str> = events.iter().map(String::as_str).collect();
    let answer = import(&mut session, &events);
    let reasons = ["uid", "uid", "uid", "times", "times", "summary", "description", "unreadable", "location"];
    let expected: Vec<Value> = reasons.iter().enumerate().map(|(index, reason)| json!({ "index": index, "reason": reason })).collect();
    assert_eq!(answer["errors"], json!(expected));
    assert_eq!((answer["added"].clone(), answer["errors_count"].clone()), (json!(1), json!(9)));
    assert_eq!(object(&session, FIRST).events[0].uid, "dentist@example.org");
    // A UID of 255 bytes is taken.
    let longest = format!("UID:{}\r\n{start}", "u".repeat(255));
    assert_eq!(import(&mut session, &[&longest])["added"], json!(1));
}

#[test]
fn the_lists_name_the_first_hundred_and_the_counts_all() {
    let (mut session, _) = session();
    let events: Vec<String> = (0..120).map(|n| format!("UID:{n}@example.org\r\nDTSTART:20261112T080000Z\r\nRRULE:FREQ=HOURLY")).collect();
    let mut events: Vec<&str> = events.iter().map(String::as_str).collect();
    events.extend(std::iter::repeat_n("DTSTART:20261112T080000Z", 101));
    let answer = import(&mut session, &events);
    assert_eq!((answer["not_supported"].as_array().unwrap().len(), answer["not_supported_count"].clone()), (100, json!(120)));
    assert_eq!((answer["errors"].as_array().unwrap().len(), answer["errors_count"].clone()), (100, json!(101)));
    assert_eq!(answer["errors"][0], json!({ "index": 120, "reason": "uid" }));
}

#[test]
fn a_start_that_cannot_be_placed_is_an_error_not_a_guess() {
    let (mut session, _) = session();
    let nowhere = "UID:n@example.org\r\nDTSTART;TZID=Nowhere/Land:20261112T080000\r\nSUMMARY:Somewhere";
    let file = Calendar::parse(ics(&[nowhere]).as_bytes()).unwrap();
    let offsets = Offset::from_minutes(60).unwrap();
    // Where the calendar screen shows the same event after a sync.
    let shown = file.zones.resolve(&file.events[0].start, &offsets);
    let answer = import(&mut session, &[nowhere]);
    match shown {
        Some(at) => assert_eq!(object(&session, FIRST).events[0].start, When::Utc(at)),
        None => assert_eq!(answer["errors"], json!([{ "index": 0, "reason": "times" }])),
    }
    // A zone the file does not describe is read on the computer's clock: 8:00 at +01:00.
    assert_eq!(shown, crate::params::parse_date("2026-11-12T07:00:00Z"));
    // A moment before or after the range of time has no place at all: midnight of the year 0 at +01:00, an end
    // 3 000 000 days on.
    let before = "UID:v@example.org\r\nDTSTART:00000101T000000\r\nSUMMARY:Too early";
    let after = "UID:w@example.org\r\nDTSTART:20261112T080000Z\r\nDURATION:P3000000D";
    let days = "UID:x@example.org\r\nDTSTART;VALUE=DATE:20261112\r\nDURATION:P3000000D";
    let reasons: Vec<Value> = (0..3).map(|index| json!({ "index": index, "reason": "times" })).collect();
    assert_eq!(import(&mut session, &[before, after, days])["errors"], json!(reasons));
}

#[test]
fn what_is_too_big_or_no_calendar_is_refused_whole() {
    let (mut session, _) = session();
    let event = |n: usize| format!("UID:{n}@example.org\r\nDTSTART:20261112T080000Z");
    let refused = |session: &mut Session, calendar: u64, ics: String| {
        let before = (session.history_len(), session.store().calendar_events(CalendarId::new(1)).unwrap().len());
        let error = session.run("calendar.import", &json!({ "calendar": calendar, "ics": ics })).unwrap_err();
        assert!(calendar_jobs(session).is_empty());
        assert_eq!((session.history_len(), session.store().calendar_events(CalendarId::new(1)).unwrap().len()), before);
        error
    };
    let long = format!("{}{}", ics(&[DENTIST]), " ".repeat(5_000_001 - ics(&[DENTIST]).len()));
    let error = refused(&mut session, 1, long);
    assert_eq!((error.code(), error.to_string().contains("longer than 5000000 bytes")), ("bad_params", true), "{error}");
    let many: Vec<String> = (0..5001).map(event).collect();
    let error = refused(&mut session, 1, ics(&many.iter().map(String::as_str).collect::<Vec<_>>()));
    assert!(error.to_string().contains("more than 5000 events"), "{error}");
    assert_eq!(refused(&mut session, 1, "BEGIN:VCARD\r\nFN:Dora\r\nEND:VCARD\r\n".to_owned()).code(), "bad_params");
    let deep = format!("{}{}", "BEGIN:VCALENDAR\r\n".repeat(9), "END:VCALENDAR\r\n".repeat(9));
    assert_eq!(refused(&mut session, 1, deep).code(), "bad_params");
    assert_eq!(refused(&mut session, 1, "not a calendar".to_owned()).code(), "bad_params");
    assert_eq!(refused(&mut session, 99, ics(&[DENTIST])).code(), "not_found");
    let preview = session.run("calendar.import.preview", &json!({ "calendar": 99, "ics": ics(&[DENTIST]) })).unwrap_err();
    assert_eq!(preview.code(), "not_found");

    let most: Vec<String> = (0..5000).map(event).collect();
    let answer = import(&mut session, &most.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(answer["added"], json!(5000));
    assert_eq!(session.history_len(), 1);
}

#[test]
fn text_from_the_file_cannot_add_a_line() {
    let (mut session, _) = session();
    let hostile = "UID:h@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:One\\nATTENDEE:mailto:x@example.com\rTwo END:VEVENT\\nBEGIN:VEVENT\\nUID:x\r\n\
                   LOCATION:Here\\r\\nORGANIZER:mailto:x@example.com";
    assert_eq!(import(&mut session, &[hostile])["added"], json!(1));
    let calendar = object(&session, FIRST);
    assert_eq!(calendar.events.len(), 1);
    let event = &calendar.events[0];
    assert!(event.other.is_empty() && event.attendees.is_empty() && event.organizer.is_none(), "{event:?}");
    assert!(event.summary.as_deref().unwrap().starts_with("One\nATTENDEE"));
    let held = session.store().calendar_event(CalendarEventId::new(FIRST)).unwrap().unwrap();
    assert_eq!(held.ical.lines().filter(|line| line.trim_end() == "BEGIN:VEVENT").count(), 1, "{}", held.ical);
    assert!(held.ical.lines().all(|line| !line.starts_with("ATTENDEE") && !line.starts_with("ORGANIZER")), "{}", held.ical);
}

#[test]
fn a_write_the_server_refuses_brings_the_server_s_copy() {
    let (mut session, _) = session();
    import(&mut session, &[DENTIST]);
    let job = session.store().queued_jobs().unwrap()[0].id;
    calendar_jobs(&mut session);
    // The server holds an event at that URL already (`If-None-Match: *` failed): its copy wins.
    let conflict = CalendarReport::Conflict { calendar: CalendarId::new(1), event: CalendarEventId::new(FIRST) };
    session.apply(&Event::new(AccountId::new(1), Some(job), EventKind::Calendar(conflict))).unwrap();
    let jobs = calendar_jobs(&mut session);
    assert!(matches!(jobs.as_slice(), [CalendarJob::Sync { token: None, calendar, .. }] if *calendar == CalendarId::new(1)), "{jobs:?}");
}

#[test]
fn the_preview_says_the_name_the_counts_and_the_first_five() {
    let (mut session, _) = session();
    let events: Vec<String> = (1..=6).map(|day| format!("UID:{day}@example.org\r\nDTSTART:2026111{day}T080000Z\r\nSUMMARY:Event {day}")).collect();
    let mut events: Vec<&str> = events.iter().map(String::as_str).collect();
    events.insert(1, HOLIDAY);
    events.push("UID:lunch@example.org\r\nDTSTART:20261106T110000Z");
    events.push("DTSTART:20261106T110000Z");
    events.push("UID:f@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Fortnightly\r\nRRULE:FREQ=WEEKLY;INTERVAL=2");
    let params = json!({ "calendar": 1, "ics": ics(&events) });
    let preview = session.run("calendar.import.preview", &params).unwrap();
    assert_eq!(
        preview,
        json!({
            "calendar": 1, "name": "Work", "add": 8, "skipped": 1, "single": 1, "left_out": 1,
            "first": [
                { "summary": "Event 1", "start": "2026-11-11T08:00:00Z", "all_day": false },
                { "summary": "Winter break", "start": "2026-12-24", "all_day": true },
                { "summary": "Event 2", "start": "2026-11-12T08:00:00Z", "all_day": false },
                { "summary": "Event 3", "start": "2026-11-13T08:00:00Z", "all_day": false },
                { "summary": "Event 4", "start": "2026-11-14T08:00:00Z", "all_day": false },
            ],
        })
    );
    // The import does what the preview said.
    let answer = session.run("calendar.import", &params).unwrap();
    assert_eq!((answer["added"].clone(), answer["skipped"].clone(), answer["single"].clone()), (json!(8), json!(1), json!(1)));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn junk_as_ics_never_panics(text in any::<String>(), bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let (mut session, _) = session();
        for ics in [text, String::from_utf8_lossy(&bytes).into_owned(), format!("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\n{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", String::from_utf8_lossy(&bytes))] {
            for id in ["calendar.import", "calendar.import.preview"] {
                let result = session.run(id, &json!({ "calendar": 1, "ics": ics }));
                prop_assert!(!matches!(result, Err(EngineError::Panicked(_))), "{result:?}");
            }
        }
    }
}
