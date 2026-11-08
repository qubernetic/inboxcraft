// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writing events: `event.create`, `event.update` and `event.delete`, the delete held for the undo window on a fake
//! clock, and `history.undo` on each.

use std::time::Duration;

use inboxcraft_ical::{Calendar, Class, Frequency, PartStat, When};
use inboxcraft_model::{CalendarEventId, CalendarId, CalendarJob, CalendarReport, Event, EventKind, JobKind};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::calendar_tests::{WORK, session_at};
use crate::test_support::{NOW, at};
use crate::{Caller, EngineError, Session};

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (session_at(&clock), clock)
}

/// The moment an ISO 8601 text names.
fn utc(text: &str) -> Timestamp {
    crate::params::parse_date(text).unwrap()
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

fn create(session: &mut Session, params: Value) -> u64 {
    session.run("event.create", &params).unwrap()["id"].as_u64().unwrap()
}

fn queued(session: &Session) -> usize {
    session.store().queued_jobs().unwrap().len()
}

#[test]
fn event_create_keeps_the_event_and_writes_it_to_the_server() {
    let (mut session, _) = session();
    let answer = session
        .run("event.create", &json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-12T08:00:00Z", "location": "Main Street 1" }))
        .unwrap();
    assert_eq!(answer["id"], json!(5));
    assert_eq!(answer["calendar"], json!(1));
    let uid = answer["uid"].as_str().unwrap();
    let held = session.store().calendar_event(CalendarEventId::new(5)).unwrap().unwrap();
    assert_eq!(held.etag, None);
    assert!(held.href.starts_with(WORK) && held.href.ends_with(".ics"), "{}", held.href);
    let calendar = object(&session, 5);
    let event = &calendar.events[0];
    assert_eq!(event.uid, uid);
    assert_eq!(event.summary.as_deref(), Some("Dentist"));
    assert_eq!(event.location.as_deref(), Some("Main Street 1"));
    // A single timed event is a moment in UTC, an hour long when no end is given.
    assert_eq!(event.start, When::Utc(utc("2026-11-12T08:00:00Z")));
    assert_eq!(event.end, Some(When::Utc(utc("2026-11-12T09:00:00Z"))));
    assert!(!event.free);
    assert_eq!(event.stamp, Some(at(NOW)));
    let jobs = calendar_jobs(&mut session);
    assert_eq!(
        jobs,
        [CalendarJob::Put { calendar: CalendarId::new(1), event: CalendarEventId::new(5), url: held.href, etag: None, ical: held.ical }]
    );
    // Kept in the store's queue until the server answers.
    assert_eq!(queued(&session), 1);
    let written = CalendarReport::Written { calendar: CalendarId::new(1), event: CalendarEventId::new(5), etag: Some("\"d1\"".to_owned()) };
    let job = session.store().queued_jobs().unwrap()[0].id;
    session.apply(&Event::new(inboxcraft_model::AccountId::new(1), Some(job), EventKind::Calendar(written))).unwrap();
    assert_eq!(queued(&session), 0);
    assert_eq!(session.store().calendar_event(CalendarEventId::new(5)).unwrap().unwrap().etag.as_deref(), Some("\"d1\""));
}

#[test]
fn a_new_event_shows_in_the_list_at_once() {
    let (mut session, _) = session();
    create(&mut session, json!({ "calendar": 2, "summary": "Haircut", "start": "2026-11-07T15:00:00+01:00", "end": "2026-11-07T15:30:00+01:00" }));
    let list = session.run("event.list", &json!({ "from": "2026-11-07", "to": "2026-11-08" })).unwrap();
    assert_eq!(list["events"][0]["summary"], json!("Haircut"));
    assert_eq!(list["events"][0]["local_start"], json!("2026-11-07T15:00:00+01:00"));
    assert_eq!(list["events"][0]["local_end"], json!("2026-11-07T15:30:00+01:00"));
}

#[test]
fn an_all_day_event_takes_whole_days_its_end_the_last_day() {
    let (mut session, _) = session();
    let id = create(&mut session, json!({ "calendar": 2, "summary": "Conference", "all_day": true, "start": "2026-11-20", "end": "2026-11-21" }));
    let event = &object(&session, id).events[0];
    assert_eq!(event.start, When::Date(inboxcraft_ical::Date::new(2026, 11, 20).unwrap()));
    assert_eq!(event.end, Some(When::Date(inboxcraft_ical::Date::new(2026, 11, 22).unwrap())));
    let list = session.run("event.list", &json!({ "from": "2026-11-19", "to": "2026-11-23", "calendar": 2 })).unwrap();
    assert_eq!(list["events"][0]["start_date"], json!("2026-11-20"));
    assert_eq!(list["events"][0]["end_date"], json!("2026-11-21"));
    let one = create(&mut session, json!({ "calendar": 2, "summary": "Holiday", "all_day": true, "start": "2026-12-24" }));
    assert_eq!(object(&session, one).events[0].end, Some(When::Date(inboxcraft_ical::Date::new(2026, 12, 25).unwrap())));
}

#[test]
fn a_repeating_event_keeps_the_computers_wall_clock_time() {
    let (mut session, _) = session();
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Gym", "start": "2026-11-10T18:00:00+01:00", "repeat": "weekdays" }));
    let event = &object(&session, id).events[0];
    assert_eq!(event.start, When::Floating(inboxcraft_time::CivilDateTime::new(2026, 11, 10, 18, 0, 0).unwrap()));
    assert_eq!(event.rules.len(), 1);
    assert_eq!(event.rules[0].frequency, Frequency::Weekly);
    let list = session.run("event.list", &json!({ "from": "2026-11-10", "to": "2026-11-17", "calendar": 1 })).unwrap();
    let gym: Vec<&Value> = list["events"].as_array().unwrap().iter().filter(|row| row["summary"] == "Gym").collect();
    assert_eq!(gym.len(), 5);
    assert!(gym.iter().all(|row| row["repeat"] == "weekdays" && row["local_start"].as_str().unwrap().ends_with("T18:00:00+01:00")));
}

#[test]
fn free_private_and_guests_are_written() {
    let (mut session, _) = session();
    let id = create(
        &mut session,
        json!({ "calendar": 1, "summary": "Planning", "start": "2026-11-12T10:00:00Z", "busy": false, "private": true,
                "guests": ["dora@example.com", " Dora@Example.com ", "carl@example.net"] }),
    );
    let event = &object(&session, id).events[0];
    assert!(event.free);
    assert_eq!(event.class, Some(Class::Private));
    assert_eq!(event.organizer.as_ref().map(|organizer| organizer.address.as_str()), Some("mailto:anna@example.org"));
    let guests: Vec<(&str, bool, &PartStat)> = event.attendees.iter().map(|guest| (guest.address.as_str(), guest.rsvp, &guest.status)).collect();
    assert_eq!(guests, [("mailto:dora@example.com", true, &PartStat::NeedsAction), ("mailto:carl@example.net", true, &PartStat::NeedsAction)]);
}

#[test]
fn event_create_refuses_what_it_cannot_write() {
    let (mut session, _) = session();
    let long = "x".repeat(1001);
    for params in [
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12" }),
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z", "end": "2026-11-12T09:00:00Z" }),
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z", "end": "2028-11-12T09:00:00Z" }),
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z", "repeat": "fortnightly" }),
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z", "guests": ["not an address"] }),
        json!({ "calendar": 1, "summary": long, "start": "2026-11-12T10:00:00Z" }),
        json!({ "calendar": 1, "summary": "A", "all_day": true, "start": "2026-11-12T10:00:00Z" }),
        json!({ "calendar": 1, "summary": "A", "all_day": true, "start": "2026-11-12", "end": "2026-11-11" }),
        json!({ "calendar": 1, "summary": "A", "all_day": true, "start": "2026-02-30" }),
        json!({ "calendar": 1, "summary": "A", "start": "soon" }),
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z", "description": "y".repeat(20_001) }),
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z", "guests": vec!["a@example.org"; 101] }),
        json!({ "calendar": 1, "summary": "A", "all_day": true, "start": "2026-11-12", "end": "soon" }),
        json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z", "end": "soon" }),
    ] {
        assert_eq!(session.run("event.create", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    let missing = session.run("event.create", &json!({ "calendar": 8, "summary": "A", "start": "2026-11-12T10:00:00Z" })).unwrap_err();
    assert_eq!(missing, EngineError::NotFound { what: "calendar", id: 8 });
    assert!(session.take_jobs().is_empty());
}

#[test]
fn the_writing_commands_are_the_owners() {
    let (mut session, _) = session();
    for id in ["event.create", "event.update", "event.delete"] {
        let refused = session.run_as(Caller::Agent, id, &json!({})).unwrap_err();
        assert_eq!(refused.code(), "not_allowed", "{id}");
    }
}

#[test]
fn event_update_changes_what_it_is_given_and_writes_over_the_copy_seen() {
    let (mut session, _) = session();
    let answer = session.run("event.update", &json!({ "id": 2, "summary": "Long lunch", "end": "2026-11-06T13:00:00Z", "busy": true })).unwrap();
    assert_eq!(answer, json!({ "id": 2, "guests": 1, "notified": false }));
    let event = &object(&session, 2).events[0];
    assert_eq!(event.summary.as_deref(), Some("Long lunch"));
    assert_eq!(event.start, When::Utc(utc("2026-11-06T11:00:00Z")));
    assert_eq!(event.end, Some(When::Utc(utc("2026-11-06T13:00:00Z"))));
    assert!(!event.free);
    // What it was not given stays: the place, the class, the guest, the sequence.
    assert_eq!(event.location.as_deref(), Some("Corner café"));
    assert_eq!(event.class, Some(Class::Private));
    assert_eq!(event.attendees.len(), 1);
    assert_eq!(event.sequence, 0);
    let jobs = calendar_jobs(&mut session);
    assert!(matches!(jobs.as_slice(), [CalendarJob::Put { etag: Some(etag), .. }] if etag == "\"l1\""), "{jobs:?}");
}

#[test]
fn editing_an_event_with_guests_tells_them_only_when_asked() {
    let (mut session, _) = session();
    let answer = session.run("event.update", &json!({ "id": 2, "start": "2026-11-06T11:30:00Z", "notify": true })).unwrap();
    assert_eq!(answer, json!({ "id": 2, "guests": 1, "notified": true }));
    let event = &object(&session, 2).events[0];
    assert_eq!(event.sequence, 1);
    // The length stays when only the start moves.
    assert_eq!(event.end, Some(When::Utc(utc("2026-11-06T12:30:00Z"))));
    // Without guests there is nobody to tell.
    let alone = session.run("event.update", &json!({ "id": 3, "summary": "Call", "notify": true })).unwrap();
    assert_eq!(alone, json!({ "id": 3, "guests": 0, "notified": false }));
    assert_eq!(object(&session, 3).events[0].sequence, 0);
}

#[test]
fn editing_a_zoned_event_keeps_its_zone() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 3, "start": "2026-11-06T15:00:00Z", "end": "2026-11-06T15:45:00Z" })).unwrap();
    let event = &object(&session, 3).events[0];
    let civil = |hour, minute| inboxcraft_time::CivilDateTime::new(2026, 11, 6, hour, minute, 0).unwrap();
    assert_eq!(event.start, When::Zoned(civil(10, 0), "America/New_York".to_owned()));
    assert_eq!(event.end, Some(When::Zoned(civil(10, 45), "America/New_York".to_owned())));
    let list = session.run("event.list", &json!({ "from": "2026-11-06", "to": "2026-11-07", "calendar": 1 })).unwrap();
    let call = list["events"].as_array().unwrap().iter().find(|row| row["id"] == 3).unwrap();
    assert_eq!(call["zone"]["start"], json!("2026-11-06T10:00:00-05:00"));
}

#[test]
fn editing_a_series_changes_its_repeat_and_can_end_it() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 1, "repeat": "daily" })).unwrap();
    assert_eq!(object(&session, 1).events[0].rules[0].frequency, Frequency::Daily);
    session.run("event.update", &json!({ "id": 1, "repeat": "none" })).unwrap();
    assert!(object(&session, 1).events[0].rules.is_empty());
    // A one-off moment in UTC that starts repeating moves to the computer's wall clock.
    session.run("event.update", &json!({ "id": 2, "repeat": "weekly" })).unwrap();
    let lunch = &object(&session, 2).events[0];
    assert_eq!(lunch.start, When::Floating(inboxcraft_time::CivilDateTime::new(2026, 11, 6, 12, 0, 0).unwrap()));
}

#[test]
fn an_event_turns_all_day_and_back() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "all_day": true, "start": "2026-11-06" })).unwrap();
    let lunch = &object(&session, 2).events[0];
    assert!(lunch.all_day());
    assert_eq!(lunch.end, Some(When::Date(inboxcraft_ical::Date::new(2026, 11, 7).unwrap())));
    session.run("event.update", &json!({ "id": 2, "all_day": false, "start": "2026-11-06T12:00:00Z" })).unwrap();
    let lunch = &object(&session, 2).events[0];
    assert_eq!(lunch.start, When::Utc(utc("2026-11-06T12:00:00Z")));
    assert_eq!(lunch.end, Some(When::Utc(utc("2026-11-06T13:00:00Z"))));
    let refused = session.run("event.update", &json!({ "id": 2, "all_day": true })).unwrap_err();
    assert_eq!(refused.code(), "bad_params");
}

#[test]
fn guests_change_and_keep_their_answers() {
    let (mut session, _) = session();
    let lunch = session.store().calendar_event(CalendarEventId::new(2)).unwrap().unwrap();
    let answered = lunch.ical.replace("ATTENDEE;CN=Dora:", "ATTENDEE;CN=Dora;PARTSTAT=ACCEPTED:");
    crate::test_support::change(
        &mut session,
        inboxcraft_store::Change::PutCalendarEvent(inboxcraft_model::CalendarEvent { ical: answered, ..lunch }),
    );
    session.run("event.update", &json!({ "id": 2, "guests": ["dora@example.com", "erik@example.org"] })).unwrap();
    let event = &object(&session, 2).events[0];
    let guests: Vec<(&str, &PartStat)> = event.attendees.iter().map(|guest| (guest.address.as_str(), &guest.status)).collect();
    assert_eq!(guests, [("mailto:dora@example.com", &PartStat::Accepted), ("mailto:erik@example.org", &PartStat::NeedsAction)]);
    session.run("event.update", &json!({ "id": 2, "guests": [], "location": "", "description": "Bring the plans" })).unwrap();
    let event = &object(&session, 2).events[0];
    assert!(event.attendees.is_empty());
    assert_eq!(event.location, None);
    assert_eq!(event.description.as_deref(), Some("Bring the plans"));
}

#[test]
fn event_update_refuses_nothing_to_change_and_what_it_cannot_read() {
    let (mut session, _) = session();
    assert_eq!(session.run("event.update", &json!({ "id": 2 })).unwrap_err().code(), "bad_params");
    assert_eq!(session.run("event.update", &json!({ "id": 2, "notify": true })).unwrap_err().code(), "bad_params");
    assert_eq!(session.run("event.update", &json!({ "id": 2, "start": "later" })).unwrap_err().code(), "bad_params");
    assert_eq!(session.run("event.update", &json!({ "id": 2, "end": "2026-11-06T10:00:00Z" })).unwrap_err().code(), "bad_params");
    assert_eq!(session.run("event.update", &json!({ "id": 42, "summary": "A" })).unwrap_err(), EngineError::NotFound { what: "event", id: 42 });
    crate::test_support::change(
        &mut session,
        inboxcraft_store::Change::PutCalendarEvent(super::calendar_tests::event(9, 1, "broken", None, "BEGIN:VCALENDAR\r\nbroken")),
    );
    assert_eq!(session.run("event.update", &json!({ "id": 9, "summary": "A" })).unwrap_err().code(), "bad_params");
}

#[test]
fn event_delete_waits_out_the_undo_window_then_deletes_on_the_server() {
    let (mut session, clock) = session();
    let answer = session.run("event.delete", &json!({ "id": 2 })).unwrap();
    assert_eq!(answer, json!({ "id": 2, "undo_until": "2026-11-05T09:00:06Z" }));
    // Gone from the store and the list at once, the server not asked yet, the delete kept in the queue.
    assert_eq!(session.store().calendar_event(CalendarEventId::new(2)).unwrap(), None);
    assert!(calendar_jobs(&mut session).is_empty());
    assert_eq!(queued(&session), 1);
    assert_eq!(session.next_due().unwrap(), Some(at(NOW + 6)));
    clock.advance(Duration::from_secs(5));
    session.tick().unwrap();
    assert!(calendar_jobs(&mut session).is_empty());
    clock.advance(Duration::from_secs(1));
    session.tick().unwrap();
    let jobs = calendar_jobs(&mut session);
    assert_eq!(
        jobs,
        [CalendarJob::Delete {
            calendar: CalendarId::new(1),
            event: CalendarEventId::new(2),
            url: format!("{WORK}lunch.ics"),
            etag: Some("\"l1\"".to_owned()),
        }]
    );
    assert_eq!(session.next_due().unwrap(), None);
    // Too late to undo now.
    let late = session.run("history.undo", &json!({})).unwrap();
    assert_eq!(late["undone"], json!(false));
    assert_eq!(session.store().calendar_event(CalendarEventId::new(2)).unwrap(), None);
}

#[test]
fn undo_within_the_window_brings_the_event_back_and_nothing_reaches_the_server() {
    let (mut session, clock) = session();
    session.run("event.delete", &json!({ "id": 2 })).unwrap();
    clock.advance(Duration::from_secs(3));
    let undone = session.run("history.undo", &json!({})).unwrap();
    assert_eq!(undone, json!({ "command": "event.delete", "id": 2, "undone": true }));
    let lunch = session.store().calendar_event(CalendarEventId::new(2)).unwrap().unwrap();
    assert_eq!(lunch.etag.as_deref(), Some("\"l1\""));
    assert_eq!(queued(&session), 0);
    clock.advance(Duration::from_secs(10));
    session.tick().unwrap();
    assert!(calendar_jobs(&mut session).is_empty());
    // No held delete is left: only the lunch's alert, ten minutes before it, is due.
    assert_eq!(session.next_due().unwrap(), Some(utc("2026-11-06T10:50:00Z")));
}

#[test]
fn a_sync_during_the_window_does_not_bring_the_deleted_event_back() {
    let (mut session, _) = session();
    session.run("event.delete", &json!({ "id": 2 })).unwrap();
    let lunch = super::calendar_tests::lunch();
    let synced = CalendarReport::Synced {
        calendar: CalendarId::new(1),
        changed: vec![inboxcraft_model::FetchedEvent { url: format!("{WORK}lunch.ics"), etag: Some("\"l1\"".to_owned()), ical: lunch }],
        removed: vec![],
        token: None,
        full: true,
    };
    session.apply(&Event::new(inboxcraft_model::AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    assert!(session.store().calendar_events(CalendarId::new(1)).unwrap().iter().all(|event| !event.href.ends_with("lunch.ics")));
}

#[test]
fn undo_takes_a_new_event_off_again_and_an_edit_back() {
    let (mut session, _) = session();
    let written = |session: &mut Session, job, event: u64, etag: &str| {
        let report = CalendarReport::Written { calendar: CalendarId::new(1), event: CalendarEventId::new(event), etag: Some(etag.to_owned()) };
        session.apply(&Event::new(inboxcraft_model::AccountId::new(1), Some(job), EventKind::Calendar(report))).unwrap();
    };
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-12T08:00:00Z" }));
    let made = session.take_jobs()[0].id;
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let edited = session.take_jobs()[0].id;
    let undone = session.run("history.undo", &json!({})).unwrap();
    assert_eq!(undone, json!({ "command": "event.update", "id": 2, "undone": true }));
    assert_eq!(object(&session, 2).events[0].summary.as_deref(), Some("Lunch"));
    // Each undo goes out once the server has taken the write it takes back, with the tag that write got.
    written(&mut session, edited, 2, "\"l2\"");
    let jobs = calendar_jobs(&mut session);
    assert!(
        matches!(jobs.as_slice(), [CalendarJob::Put { etag: Some(etag), ical, .. }] if etag == "\"l2\"" && ical.contains("SUMMARY:Lunch")),
        "{jobs:?}"
    );
    let undone = session.run("history.undo", &json!({})).unwrap();
    assert_eq!(undone, json!({ "command": "event.create", "id": id, "undone": true }));
    assert_eq!(session.store().calendar_event(CalendarEventId::new(id)).unwrap(), None);
    written(&mut session, made, id, "\"d1\"");
    let jobs = calendar_jobs(&mut session);
    assert!(matches!(jobs.as_slice(), [CalendarJob::Delete { etag: Some(etag), .. }] if etag == "\"d1\""), "{jobs:?}");
}

#[test]
fn undo_of_an_event_gone_meanwhile_changes_nothing() {
    let (mut session, _) = session();
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-12T08:00:00Z" }));
    let gone = CalendarReport::Deleted { calendar: CalendarId::new(1), event: CalendarEventId::new(id) };
    session.apply(&Event::new(inboxcraft_model::AccountId::new(1), None, EventKind::Calendar(gone))).unwrap();
    assert_eq!(session.run("history.undo", &json!({})).unwrap()["undone"], json!(false));
    session.run("event.update", &json!({ "id": 3, "summary": "Call" })).unwrap();
    let gone = CalendarReport::Deleted { calendar: CalendarId::new(1), event: CalendarEventId::new(3) };
    session.apply(&Event::new(inboxcraft_model::AccountId::new(1), None, EventKind::Calendar(gone))).unwrap();
    assert_eq!(session.run("history.undo", &json!({})).unwrap()["undone"], json!(false));
}

#[test]
fn an_agent_cannot_undo_an_event_write() {
    let (mut session, _) = session();
    session.run("event.delete", &json!({ "id": 2 })).unwrap();
    assert_eq!(session.run_as(Caller::Agent, "history.undo", &json!({})).unwrap_err().code(), "not_allowed");
}

#[test]
fn removing_the_account_drops_its_held_delete() {
    let (mut session, clock) = session();
    session.run("event.delete", &json!({ "id": 2 })).unwrap();
    session.run("account.remove", &json!({ "account": 1 })).unwrap();
    clock.advance(Duration::from_secs(10));
    session.tick().unwrap();
    assert!(calendar_jobs(&mut session).is_empty());
}

#[test]
fn event_delete_of_an_event_not_held_is_not_found() {
    let (mut session, _) = session();
    assert_eq!(session.run("event.delete", &json!({ "id": 77 })).unwrap_err(), EngineError::NotFound { what: "event", id: 77 });
}

#[test]
fn moving_an_all_day_event_keeps_its_number_of_days() {
    let (mut session, _) = session();
    let day = |month, day| When::Date(inboxcraft_ical::Date::new(2026, month, day).unwrap());
    session.run("event.update", &json!({ "id": 4, "start": "2026-11-20" })).unwrap();
    let off = &object(&session, 4).events[0];
    assert_eq!((off.start.clone(), off.end.clone()), (day(11, 20), Some(day(11, 22))));
    session.run("event.update", &json!({ "id": 4, "end": "2026-11-24" })).unwrap();
    assert_eq!(object(&session, 4).events[0].end, Some(day(11, 25)));
    assert_eq!(session.run("event.update", &json!({ "id": 4, "end": "the 24th" })).unwrap_err().code(), "bad_params");
    assert_eq!(session.run("event.update", &json!({ "id": 4, "start": "2026-11-20T10:00:00Z" })).unwrap_err().code(), "bad_params");
}

#[test]
fn an_event_without_an_end_keeps_its_length() {
    let (mut session, _) = session();
    let instant =
        super::calendar_tests::object("BEGIN:VEVENT\r\nUID:i@example.org\r\nDTSTART:20261112T080000Z\r\nSUMMARY:Call back\r\nEND:VEVENT\r\n");
    let day = super::calendar_tests::object("BEGIN:VEVENT\r\nUID:d@example.org\r\nDTSTART;VALUE=DATE:20261113\r\nSUMMARY:Bin day\r\nEND:VEVENT\r\n");
    let put = |id, ical: &str| inboxcraft_store::Change::PutCalendarEvent(super::calendar_tests::event(id, 2, &format!("e{id}"), None, ical));
    crate::test_support::change(&mut session, put(9, &instant));
    crate::test_support::change(&mut session, put(10, &day));
    session.run("event.update", &json!({ "id": 9, "summary": "Call Dora back" })).unwrap();
    let event = &object(&session, 9).events[0];
    assert_eq!(event.end, Some(event.start.clone()));
    session.run("event.update", &json!({ "id": 10, "summary": "Bins out" })).unwrap();
    assert_eq!(object(&session, 10).events[0].end, Some(When::Date(inboxcraft_ical::Date::new(2026, 11, 14).unwrap())));
}

#[test]
fn editing_a_repeating_event_keeps_it_on_the_wall_clock() {
    let (mut session, _) = session();
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Gym", "start": "2026-11-10T18:00:00+01:00", "repeat": "weekly" }));
    session.run("event.update", &json!({ "id": id, "start": "2026-11-11T19:00:00+01:00" })).unwrap();
    let event = &object(&session, id).events[0];
    assert_eq!(event.start, When::Floating(inboxcraft_time::CivilDateTime::new(2026, 11, 11, 19, 0, 0).unwrap()));
    assert_eq!(event.end, Some(When::Floating(inboxcraft_time::CivilDateTime::new(2026, 11, 11, 20, 0, 0).unwrap())));
    assert_eq!(session.run("event.update", &json!({ "id": id, "repeat": "hourly" })).unwrap_err().code(), "bad_params");
}

#[test]
fn a_sync_brings_events_whose_delete_is_held_in_another_calendar() {
    let (mut session, _) = session();
    session.run("event.delete", &json!({ "id": 4 })).unwrap();
    let lunch = super::calendar_tests::lunch().replace("SUMMARY:Lunch", "SUMMARY:Late lunch");
    let synced = CalendarReport::Synced {
        calendar: CalendarId::new(1),
        changed: vec![inboxcraft_model::FetchedEvent { url: format!("{WORK}lunch.ics"), etag: Some("\"l2\"".to_owned()), ical: lunch }],
        removed: vec![],
        token: None,
        full: false,
    };
    session.apply(&Event::new(inboxcraft_model::AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    assert_eq!(object(&session, 2).events[0].summary.as_deref(), Some("Late lunch"));
}

/// The `ORGANIZER` and `ATTENDEE` lines of event `id`, unfolded.
fn people(session: &Session, id: u64) -> Vec<String> {
    let held = session.store().calendar_event(CalendarEventId::new(id)).unwrap().unwrap();
    held.ical.replace("\r\n ", "").lines().filter(|line| line.starts_with("ORGANIZER") || line.starts_with("ATTENDEE")).map(str::to_owned).collect()
}

#[test]
fn the_server_is_told_not_to_mail_the_guests() {
    let (mut session, _) = session();
    let id = create(&mut session, json!({ "calendar": 1, "summary": "Planning", "start": "2026-11-12T10:00:00Z", "guests": ["dora@example.com"] }));
    let people = people(&session, id);
    assert_eq!(people.len(), 2, "{people:?}");
    assert!(people.iter().all(|line| line.contains(";SCHEDULE-AGENT=CLIENT")), "{people:?}");
    let event = &object(&session, id).events[0];
    assert!(event.attendees.iter().chain(&event.organizer).all(|person| person.param("SCHEDULE-AGENT") == Some("CLIENT")));
}

#[test]
fn an_edit_keeps_what_the_guests_carry_and_tells_the_server_not_to_mail_them() {
    let (mut session, _) = session();
    let lunch = session.store().calendar_event(CalendarEventId::new(2)).unwrap().unwrap();
    let marked = lunch.ical.replace("ATTENDEE;CN=Dora:", "ATTENDEE;CN=Dora;SCHEDULE-AGENT=SERVER;X-SEEN=yes:");
    crate::test_support::change(&mut session, inboxcraft_store::Change::PutCalendarEvent(inboxcraft_model::CalendarEvent { ical: marked, ..lunch }));
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let people = people(&session, 2);
    assert!(people.iter().any(|line| line == "ATTENDEE;CN=Dora;SCHEDULE-AGENT=CLIENT;X-SEEN=yes:mailto:dora@example.com"), "{people:?}");
    assert!(people.iter().any(|line| line == "ORGANIZER;SCHEDULE-AGENT=CLIENT:mailto:anna@example.org"), "{people:?}");
    let jobs = calendar_jobs(&mut session);
    assert!(matches!(jobs.as_slice(), [CalendarJob::Put { ical, .. }] if ical.contains("X-SEEN=yes")), "{jobs:?}");
}
