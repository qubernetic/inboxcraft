// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Calendars in step with the server: what the CalDAV worker reports lands in the store's calendar tables, and
//! `calendar.sync`, `calendar.server` and the start ask for it.

use inboxcraft_model::{
    AccountId, CalendarEventId, CalendarId, CalendarJob, CalendarListing, CalendarReport, ErrorKind, Event, EventKind, FetchedEvent, JobKind,
    KnownEvent,
};
use inboxcraft_store::Change;
use serde_json::json;

use super::calendar_tests::{WORK, event, lunch, session, standup};
use crate::test_support::change;
use crate::{Applied, Caller, EngineError, Session};

fn report(session: &mut Session, account: u64, report: CalendarReport) -> Applied {
    session.apply(&Event::new(AccountId::new(account), None, EventKind::Calendar(report))).unwrap()
}

fn listing(url: &str, name: Option<&str>, events: bool) -> CalendarListing {
    CalendarListing { url: url.to_owned(), name: name.map(str::to_owned), colour: None, ctag: None, sync_token: None, events }
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

fn names(session: &Session, account: u64) -> Vec<(u64, String, String)> {
    session.store().calendars(AccountId::new(account)).unwrap().into_iter().map(|held| (held.id.get(), held.href, held.name)).collect()
}

#[test]
fn found_calendars_are_kept_by_url_new_ones_added_and_gone_ones_removed() {
    let mut session = session();
    change(&mut session, Change::SetSyncToken(CalendarId::new(1), Some("token-1".to_owned())));
    let found = CalendarReport::Found {
        home: "https://dav.example.org/calendars/anna/".to_owned(),
        calendars: vec![
            listing(WORK, Some("Work (renamed)"), true),
            listing("https://dav.example.org/calendars/anna/tasks/", Some("Tasks"), false),
            listing("https://dav.example.org/calendars/anna/trips/", None, true),
            listing(WORK, Some("Work twice"), true),
        ],
    };
    assert_eq!(report(&mut session, 1, found), Applied::Changed);
    assert_eq!(
        names(&session, 1),
        [(1, WORK.to_owned(), "Work (renamed)".to_owned()), (4, "https://dav.example.org/calendars/anna/trips/".to_owned(), "trips".to_owned())]
    );
    // Home is gone with its event; Bob's calendar of the other account stays.
    assert_eq!(session.store().calendar_event(CalendarEventId::new(4)).unwrap(), None);
    assert_eq!(names(&session, 2).len(), 1);
    let jobs = calendar_jobs(&mut session);
    let known = |etag: &str, name: &str| KnownEvent { url: format!("{WORK}{name}.ics"), etag: etag.to_owned() };
    assert_eq!(
        jobs,
        [
            CalendarJob::Sync {
                calendar: CalendarId::new(1),
                url: WORK.to_owned(),
                token: Some("token-1".to_owned()),
                known: vec![known("\"s1\"", "standup"), known("\"l1\"", "lunch"), known("\"c1\"", "call")],
            },
            CalendarJob::Sync {
                calendar: CalendarId::new(4),
                url: "https://dav.example.org/calendars/anna/trips/".to_owned(),
                token: None,
                known: vec![]
            },
        ]
    );
    // The token survives the rename.
    assert_eq!(session.store().sync_token(CalendarId::new(1)).unwrap(), Some("token-1".to_owned()));
}

#[test]
fn a_new_calendar_takes_an_id_past_every_id_held_and_the_next_one_counts_on() {
    let mut session = session();
    let found = |url: &str| CalendarReport::Found { home: String::new(), calendars: vec![listing(url, Some("One"), true)] };
    report(&mut session, 2, found("https://dav.example.net/calendars/bob/a/"));
    report(&mut session, 2, found("https://dav.example.net/calendars/bob/b/"));
    assert_eq!(names(&session, 2).iter().map(|(id, ..)| *id).collect::<Vec<_>>(), [5]);
    let all = session.run("calendar.list", &json!({})).unwrap();
    assert_eq!(all["calendars"].as_array().unwrap().len(), 3);
}

#[test]
fn a_sync_adds_changes_and_removes_events_by_url_and_keeps_the_token() {
    let mut session = session();
    let fetched = |name: &str, etag: &str, ical: String| FetchedEvent { url: format!("{WORK}{name}.ics"), etag: Some(etag.to_owned()), ical };
    let synced = CalendarReport::Synced {
        calendar: CalendarId::new(1),
        changed: vec![fetched("standup", "\"s2\"", standup().replace("Stand-up", "Daily stand-up")), fetched("new", "\"n1\"", lunch())],
        removed: vec![format!("{WORK}call.ics"), format!("{WORK}never-held.ics")],
        token: Some("token-2".to_owned()),
        full: false,
    };
    assert_eq!(report(&mut session, 1, synced), Applied::Changed);
    let events = session.store().calendar_events(CalendarId::new(1)).unwrap();
    let held: Vec<(u64, Option<&str>)> = events.iter().map(|held| (held.id.get(), held.etag.as_deref())).collect();
    assert_eq!(held, [(1, Some("\"s2\"")), (2, Some("\"l1\"")), (5, Some("\"n1\""))]);
    assert!(events[0].ical.contains("Daily stand-up"));
    assert_eq!(session.store().sync_token(CalendarId::new(1)).unwrap(), Some("token-2".to_owned()));
}

#[test]
fn a_sync_of_a_calendar_not_held_or_of_another_account_is_dropped() {
    let mut session = session();
    let synced =
        |calendar: u64| CalendarReport::Synced { calendar: CalendarId::new(calendar), changed: vec![], removed: vec![], token: None, full: true };
    assert_eq!(report(&mut session, 1, synced(9)), Applied::Dropped);
    assert_eq!(report(&mut session, 2, synced(1)), Applied::Dropped);
    assert_eq!(report(&mut session, 7, synced(1)), Applied::Dropped);
}

#[test]
fn a_written_event_takes_its_new_entity_tag_or_asks_for_a_sync_without_one() {
    let mut session = session();
    let written =
        |etag: Option<&str>| CalendarReport::Written { calendar: CalendarId::new(1), event: CalendarEventId::new(2), etag: etag.map(str::to_owned) };
    assert_eq!(report(&mut session, 1, written(Some("\"l2\""))), Applied::Changed);
    assert_eq!(session.store().calendar_event(CalendarEventId::new(2)).unwrap().unwrap().etag.as_deref(), Some("\"l2\""));
    assert_eq!(report(&mut session, 1, written(None)), Applied::Changed);
    let jobs = calendar_jobs(&mut session);
    assert!(matches!(jobs.as_slice(), [CalendarJob::Sync { calendar, token: None, .. }] if *calendar == CalendarId::new(1)), "{jobs:?}");
    let gone = CalendarReport::Written { calendar: CalendarId::new(1), event: CalendarEventId::new(99), etag: None };
    assert_eq!(report(&mut session, 1, gone), Applied::Dropped);
}

#[test]
fn a_conflict_starts_the_calendar_afresh_and_compares_everything() {
    let mut session = session();
    change(&mut session, Change::SetSyncToken(CalendarId::new(1), Some("token-1".to_owned())));
    let conflict = CalendarReport::Conflict { calendar: CalendarId::new(1), event: CalendarEventId::new(2) };
    assert_eq!(report(&mut session, 1, conflict), Applied::Changed);
    assert_eq!(session.store().sync_token(CalendarId::new(1)).unwrap(), None);
    let jobs = calendar_jobs(&mut session);
    assert!(matches!(jobs.as_slice(), [CalendarJob::Sync { token: None, known, .. }] if known.len() == 3), "{jobs:?}");
}

#[test]
fn a_deleted_event_leaves_the_store_and_a_failure_changes_nothing() {
    let mut session = session();
    let deleted = CalendarReport::Deleted { calendar: CalendarId::new(1), event: CalendarEventId::new(3) };
    assert_eq!(report(&mut session, 1, deleted.clone()), Applied::Changed);
    assert_eq!(session.store().calendar_event(CalendarEventId::new(3)).unwrap(), None);
    assert_eq!(report(&mut session, 1, deleted), Applied::Dropped);
    let failed = CalendarReport::Failed { calendar: Some(CalendarId::new(1)), kind: ErrorKind::ConnectionDropped };
    assert_eq!(report(&mut session, 1, failed), Applied::Dropped);
    assert_eq!(session.store().calendar_events(CalendarId::new(1)).unwrap().len(), 2);
}

#[test]
fn an_event_without_an_entity_tag_is_not_named_as_known() {
    let mut session = session();
    change(&mut session, Change::PutCalendarEvent(event(9, 1, "local", None, &lunch())));
    session.run("calendar.sync", &json!({ "account": 1 })).unwrap();
    let found = CalendarReport::Found { home: String::new(), calendars: vec![listing(WORK, Some("Work"), true)] };
    report(&mut session, 1, found);
    let jobs = calendar_jobs(&mut session);
    let Some(CalendarJob::Sync { known, .. }) = jobs.last() else { panic!("{jobs:?}") };
    assert_eq!(known.len(), 3);
}

#[test]
fn calendar_sync_asks_each_account_with_a_caldav_address_for_its_calendars() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "calendar.sync", &json!({})).unwrap();
    assert_eq!(answer, json!({ "accounts": [1] }));
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].account, AccountId::new(1));
    assert_eq!(jobs[0].kind, JobKind::Calendar(CalendarJob::Discover { url: None }));
    let refused = session.run("calendar.sync", &json!({ "account": 2 })).unwrap_err();
    assert_eq!(refused.code(), "bad_params");
    assert_eq!(session.run("calendar.sync", &json!({ "account": 8 })).unwrap_err(), EngineError::NotFound { what: "account", id: 8 });
}

#[test]
fn calendar_server_sets_the_address_and_finds_the_calendars_there() {
    let mut session = session();
    let answer = session.run("calendar.server", &json!({ "account": 2, "url": " https://dav.example.net/ " })).unwrap();
    assert_eq!(answer, json!({ "account": 2, "url": "https://dav.example.net/" }));
    let account = session.store().accounts().unwrap().into_iter().find(|held| held.id() == AccountId::new(2)).unwrap();
    assert_eq!(account.caldav(), Some("https://dav.example.net/"));
    assert_eq!(calendar_jobs(&mut session), [CalendarJob::Discover { url: None }]);
}

#[test]
fn calendar_server_without_an_address_forgets_the_calendars() {
    let mut session = session();
    let answer = session.run("calendar.server", &json!({ "account": 1, "url": null })).unwrap();
    assert_eq!(answer, json!({ "account": 1, "url": null }));
    assert!(session.store().calendars(AccountId::new(1)).unwrap().is_empty());
    assert!(session.take_jobs().is_empty());
}

#[test]
fn calendar_server_refuses_an_address_that_is_not_https_without_repeating_it() {
    let mut session = session();
    let error = session.run("calendar.server", &json!({ "account": 1, "url": "http://user:secret@dav.example.org/" })).unwrap_err();
    assert_eq!(error.code(), "bad_params");
    assert!(!error.to_string().contains("secret"), "{error}");
    assert_eq!(session.run("calendar.server", &json!({ "account": 9, "url": null })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
    let agent = session.run_as(Caller::Agent, "calendar.server", &json!({ "account": 1, "url": null })).unwrap_err();
    assert_eq!(agent.code(), "not_allowed");
}

#[test]
fn the_start_asks_for_the_calendars_of_each_account_with_a_caldav_address() {
    let mut session = session();
    session.start();
    let discover: Vec<AccountId> = session
        .take_jobs()
        .into_iter()
        .filter(|job| matches!(job.kind, JobKind::Calendar(CalendarJob::Discover { .. })))
        .map(|job| job.account)
        .collect();
    assert_eq!(discover, [AccountId::new(1)]);
}

#[test]
fn a_discovery_keeps_at_most_a_hundred_calendars() {
    let mut session = session();
    let calendars = (0..150).map(|n| listing(&format!("https://dav.example.org/calendars/anna/c{n}/"), None, true)).collect();
    report(&mut session, 1, CalendarReport::Found { home: String::new(), calendars });
    let held = names(&session, 1);
    assert_eq!(held.len(), crate::calendar::MAX_CALENDARS);
    assert_eq!(held[0].2, "c0");
}
