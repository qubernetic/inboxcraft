// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{CalendarJob, CalendarListing, CalendarReport, FetchedEvent, KnownEvent};
use crate::{AccountId, CalendarEventId, CalendarId, ErrorKind, Event, EventKind, Job, JobId, JobKind};

const WORK: &str = "https://dav.example.org/calendars/ada/work/";
const EVENT: &str = "https://dav.example.org/calendars/ada/work/e1.ics";
const ICAL: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e1@example.org\r\nSUMMARY:Lunch with Bea\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

fn calendar() -> CalendarId {
    CalendarId::new(3)
}

fn event() -> CalendarEventId {
    CalendarEventId::new(9)
}

#[test]
fn every_calendar_job_names_its_calendar_and_round_trips() {
    let jobs = [
        (CalendarJob::Discover { url: None }, None),
        (CalendarJob::Discover { url: Some("https://dav.example.org/".to_owned()) }, None),
        (
            CalendarJob::Sync {
                calendar: calendar(),
                url: WORK.to_owned(),
                token: Some("http://example.org/ns/sync/4".to_owned()),
                known: vec![KnownEvent { url: EVENT.to_owned(), etag: "\"1\"".to_owned() }],
            },
            Some(calendar()),
        ),
        (CalendarJob::Put { calendar: calendar(), event: event(), url: EVENT.to_owned(), etag: None, ical: ICAL.to_owned() }, Some(calendar())),
        (CalendarJob::Delete { calendar: calendar(), event: event(), url: EVENT.to_owned(), etag: Some("\"2\"".to_owned()) }, Some(calendar())),
    ];
    for (job, expected) in jobs {
        assert_eq!(job.calendar(), expected, "{job:?}");
        let kind = JobKind::Calendar(job);
        assert_eq!(kind.folder(), None);
        assert!(!kind.writes(), "a calendar job runs at once and is never queued with the IMAP writes");
        let job = Job::new(JobId::new(1), AccountId::new(1), kind);
        let json = serde_json::to_string(&job).unwrap();
        assert_eq!(serde_json::from_str::<Job>(&json).unwrap(), job);
    }
}

#[test]
fn every_calendar_report_names_its_calendar_and_round_trips() {
    let listing = CalendarListing {
        url: WORK.to_owned(),
        name: Some("Work".to_owned()),
        colour: Some("#3366CCFF".to_owned()),
        ctag: Some("\"7\"".to_owned()),
        sync_token: Some("http://example.org/ns/sync/4".to_owned()),
        events: true,
    };
    let reports = [
        (CalendarReport::Found { home: "https://dav.example.org/calendars/ada/".to_owned(), calendars: vec![listing] }, None),
        (
            CalendarReport::Synced {
                calendar: calendar(),
                changed: vec![FetchedEvent { url: EVENT.to_owned(), etag: Some("\"2\"".to_owned()), ical: ICAL.to_owned() }],
                removed: vec!["https://dav.example.org/calendars/ada/work/gone.ics".to_owned()],
                token: Some("http://example.org/ns/sync/5".to_owned()),
                full: false,
            },
            Some(calendar()),
        ),
        (CalendarReport::Written { calendar: calendar(), event: event(), etag: Some("\"3\"".to_owned()) }, Some(calendar())),
        (CalendarReport::Deleted { calendar: calendar(), event: event() }, Some(calendar())),
        (CalendarReport::Conflict { calendar: calendar(), event: event() }, Some(calendar())),
        (CalendarReport::Failed { calendar: Some(calendar()), kind: ErrorKind::LoginRefused }, Some(calendar())),
        (CalendarReport::Failed { calendar: None, kind: ErrorKind::NoCalendarServer }, None),
    ];
    for (report, expected) in reports {
        assert_eq!(report.calendar(), expected, "{report:?}");
        let kind = EventKind::Calendar(report);
        assert_eq!(kind.folder(), None);
        let event = Event::new(AccountId::new(1), Some(JobId::new(4)), kind);
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
    }
}

#[test]
fn debug_shows_the_length_of_an_event_never_its_text() {
    let put = CalendarJob::Put { calendar: calendar(), event: event(), url: EVENT.to_owned(), etag: None, ical: ICAL.to_owned() };
    let fetched = FetchedEvent { url: EVENT.to_owned(), etag: None, ical: ICAL.to_owned() };
    for printed in [format!("{put:?}"), format!("{fetched:?}"), format!("{:?}", JobKind::Calendar(put.clone()))] {
        assert!(!printed.contains("Lunch"), "{printed}");
        assert!(printed.contains(&ICAL.len().to_string()), "{printed}");
    }
}

#[test]
fn a_listing_read_back_is_cleaned() {
    let json = format!(r#"{{"url":"{WORK}","name":"Work\r\nX-Injected: yes","colour":"\u0000#336","ctag":null,"sync_token":null,"events":true}}"#);
    let listing: CalendarListing = serde_json::from_str(&json).unwrap();
    assert_eq!(listing.name.as_deref(), Some("Work X-Injected: yes"));
    assert_eq!(listing.colour.as_deref(), Some("#336"));
    let blank = format!(r#"{{"url":"{WORK}","name":"\r\n","colour":null,"ctag":null,"sync_token":null,"events":false}}"#);
    assert_eq!(serde_json::from_str::<CalendarListing>(&blank).unwrap().name, None);
}

#[test]
fn the_new_error_kinds_read_as_sentences() {
    for kind in [ErrorKind::NotEncrypted, ErrorKind::OtherServer, ErrorKind::NoCalendarServer] {
        assert!(kind.to_string().starts_with("the "), "{kind}");
        assert!(!kind.is_retryable());
    }
}
