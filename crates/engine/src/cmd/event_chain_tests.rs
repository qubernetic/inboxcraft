// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One write at a time for each event: a second write to an event (an edit, an undo, a delete) waits until the
//! server has taken the first, and goes out with the entity tag the first one got, so the owner's own changes never
//! conflict with each other.

use std::time::Duration;

use inboxcraft_model::{
    AccountId, CalendarEventId, CalendarId, CalendarJob, CalendarReport, ErrorKind, Event, EventKind, FetchedEvent, JobId, JobKind,
};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::FakeClock;
use serde_json::json;

use super::calendar_tests::{WORK, lunch, session_at};
use crate::Session;
use crate::test_support::{NOW, at};

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (session_at(&clock), clock)
}

/// The calendar jobs handed out since the last call, with their ids.
fn out(session: &mut Session) -> Vec<(JobId, CalendarJob)> {
    session
        .take_jobs()
        .into_iter()
        .filter_map(|job| match job.kind {
            JobKind::Calendar(calendar @ (CalendarJob::Put { .. } | CalendarJob::Delete { .. })) => Some((job.id, calendar)),
            _ => None,
        })
        .collect()
}

/// The one write handed out, with its id.
fn one(session: &mut Session) -> (JobId, CalendarJob) {
    let mut jobs = out(session);
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    jobs.remove(0)
}

fn report(session: &mut Session, job: JobId, report: CalendarReport) {
    session.apply(&Event::new(AccountId::new(1), Some(job), EventKind::Calendar(report))).unwrap();
}

fn written(session: &mut Session, job: JobId, event: u64, etag: Option<&str>) {
    let calendar = if event == 4 { 2 } else { 1 };
    let report_ = CalendarReport::Written { calendar: CalendarId::new(calendar), event: CalendarEventId::new(event), etag: etag.map(str::to_owned) };
    report(session, job, report_);
}

fn etag(job: &CalendarJob) -> Option<&str> {
    match job {
        CalendarJob::Put { etag, .. } | CalendarJob::Delete { etag, .. } => etag.as_deref(),
        _ => None,
    }
}

#[test]
fn an_undo_of_an_edit_waits_for_the_edit_and_takes_its_tag() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (edit, job) = one(&mut session);
    assert_eq!(etag(&job), Some("\"l1\""));
    session.run("history.undo", &json!({})).unwrap();
    assert!(out(&mut session).is_empty(), "the undo waits for the edit");
    written(&mut session, edit, 2, Some("\"l2\""));
    let (_, undo) = one(&mut session);
    assert!(matches!(&undo, CalendarJob::Put { ical, .. } if ical.contains("SUMMARY:Lunch")), "{undo:?}");
    assert_eq!(etag(&undo), Some("\"l2\""));
}

#[test]
fn a_second_edit_waits_for_the_first_and_takes_its_tag() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (first, _) = one(&mut session);
    session.run("event.update", &json!({ "id": 2, "location": "Garden" })).unwrap();
    assert!(out(&mut session).is_empty());
    written(&mut session, first, 2, Some("\"l2\""));
    let (_, second) = one(&mut session);
    assert!(matches!(&second, CalendarJob::Put { ical, .. } if ical.contains("SUMMARY:Long lunch") && ical.contains("LOCATION:Garden")));
    assert_eq!(etag(&second), Some("\"l2\""));
    // The store's copy of the job carries the tag too, so a restart sends it so.
    let queued = session.store().queued_jobs().unwrap();
    assert!(queued.iter().all(|job| matches!(&job.kind, JobKind::Calendar(written) if etag(written) == Some("\"l2\""))), "{queued:?}");
}

#[test]
fn an_edit_of_a_new_event_waits_for_its_creation_and_takes_its_tag() {
    let (mut session, _) = session();
    let id = session.run("event.create", &json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-12T08:00:00Z" })).unwrap()["id"]
        .as_u64()
        .unwrap();
    let (create, job) = one(&mut session);
    assert_eq!(etag(&job), None, "a first write: If-None-Match");
    session.run("event.update", &json!({ "id": id, "summary": "Dentist, early" })).unwrap();
    assert!(out(&mut session).is_empty());
    written(&mut session, create, id, Some("\"d1\""));
    let (_, edit) = one(&mut session);
    assert_eq!(etag(&edit), Some("\"d1\""));
}

#[test]
fn an_undo_of_a_new_event_waits_for_its_creation_and_takes_its_tag() {
    let (mut session, _) = session();
    let id = session.run("event.create", &json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-12T08:00:00Z" })).unwrap()["id"]
        .as_u64()
        .unwrap();
    let (create, _) = one(&mut session);
    session.run("history.undo", &json!({})).unwrap();
    assert!(out(&mut session).is_empty());
    written(&mut session, create, id, Some("\"d1\""));
    let (_, delete) = one(&mut session);
    assert!(matches!(delete, CalendarJob::Delete { .. }));
    assert_eq!(etag(&delete), Some("\"d1\""));
}

#[test]
fn a_delete_after_an_edit_waits_for_the_edit_and_takes_its_tag() {
    let (mut session, clock) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (edit, _) = one(&mut session);
    session.run("event.delete", &json!({ "id": 2 })).unwrap();
    clock.advance(Duration::from_secs(6));
    session.tick().unwrap();
    assert!(out(&mut session).is_empty(), "the window is over, but the edit is still out");
    written(&mut session, edit, 2, Some("\"l2\""));
    let (_, delete) = one(&mut session);
    assert!(matches!(delete, CalendarJob::Delete { .. }));
    assert_eq!(etag(&delete), Some("\"l2\""));
}

#[test]
fn a_held_delete_takes_the_tag_of_an_edit_written_during_its_window() {
    let (mut session, clock) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (edit, _) = one(&mut session);
    session.run("event.delete", &json!({ "id": 2 })).unwrap();
    written(&mut session, edit, 2, Some("\"l2\""));
    assert!(out(&mut session).is_empty(), "still in its window");
    clock.advance(Duration::from_secs(6));
    session.tick().unwrap();
    let (_, delete) = one(&mut session);
    assert_eq!(etag(&delete), Some("\"l2\""));
}

#[test]
fn a_delete_waiting_for_an_edit_can_still_be_undone() {
    let (mut session, clock) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (edit, _) = one(&mut session);
    session.run("event.delete", &json!({ "id": 2 })).unwrap();
    clock.advance(Duration::from_secs(6));
    session.tick().unwrap();
    let undone = session.run("history.undo", &json!({})).unwrap();
    assert_eq!(undone["undone"], json!(true));
    written(&mut session, edit, 2, Some("\"l2\""));
    assert!(out(&mut session).is_empty());
    assert_eq!(session.store().queued_jobs().unwrap().len(), 0);
}

#[test]
fn a_write_without_a_tag_back_waits_for_the_sync_that_brings_it() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (first, _) = one(&mut session);
    session.run("event.update", &json!({ "id": 2, "location": "Garden" })).unwrap();
    written(&mut session, first, 2, None);
    assert!(out(&mut session).is_empty(), "the tag is not known yet");
    let server = lunch().replace("SUMMARY:Lunch", "SUMMARY:Long lunch");
    let synced = CalendarReport::Synced {
        calendar: CalendarId::new(1),
        changed: vec![FetchedEvent { url: format!("{WORK}lunch.ics"), etag: Some("\"l2\"".to_owned()), ical: server }],
        removed: vec![],
        token: Some("t2".to_owned()),
        full: false,
    };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
    let (_, second) = one(&mut session);
    assert_eq!(etag(&second), Some("\"l2\""));
    // The sync did not put the server's older copy over the owner's newer one.
    let held = session.store().calendar_event(CalendarEventId::new(2)).unwrap().unwrap();
    assert!(held.ical.contains("LOCATION:Garden"), "{}", held.ical);
}

#[test]
fn a_write_without_a_tag_back_lets_the_next_go_when_the_account_fails() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (first, _) = one(&mut session);
    session.run("event.update", &json!({ "id": 2, "location": "Garden" })).unwrap();
    written(&mut session, first, 2, None);
    let failed = CalendarReport::Failed { calendar: Some(CalendarId::new(1)), kind: ErrorKind::ConnectionDropped };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(failed))).unwrap();
    let (_, second) = one(&mut session);
    assert_eq!(etag(&second), Some("\"l1\""), "the last tag known");
}

#[test]
fn a_failed_or_conflicting_write_lets_the_next_go_as_it_is() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (first, _) = one(&mut session);
    session.run("event.update", &json!({ "id": 2, "location": "Garden" })).unwrap();
    let failed = CalendarReport::Failed { calendar: Some(CalendarId::new(1)), kind: ErrorKind::ConnectionDropped };
    report(&mut session, first, failed);
    let (second, job) = one(&mut session);
    assert_eq!(etag(&job), Some("\"l1\""));
    session.run("event.update", &json!({ "id": 2, "location": "Roof" })).unwrap();
    assert!(out(&mut session).is_empty());
    report(&mut session, second, CalendarReport::Conflict { calendar: CalendarId::new(1), event: CalendarEventId::new(2) });
    assert_eq!(out(&mut session).len(), 1);
}

#[test]
fn writes_to_different_events_do_not_wait_for_each_other() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    session.run("event.update", &json!({ "id": 3, "summary": "Call" })).unwrap();
    assert_eq!(out(&mut session).len(), 2);
}

#[test]
fn removing_the_account_drops_its_waiting_writes() {
    let (mut session, _) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    let (first, _) = one(&mut session);
    session.run("event.update", &json!({ "id": 2, "location": "Garden" })).unwrap();
    session.run("account.remove", &json!({ "account": 1 })).unwrap();
    written(&mut session, first, 2, Some("\"l2\""));
    assert!(out(&mut session).is_empty());
}

#[test]
fn a_restart_hands_the_kept_writes_out_one_at_a_time() {
    let (mut session, clock) = session();
    session.run("event.update", &json!({ "id": 2, "summary": "Long lunch" })).unwrap();
    session.run("event.update", &json!({ "id": 2, "location": "Garden" })).unwrap();
    // The store as a restart finds it: the two writes in its queue.
    let mut store = super::calendar_tests::store();
    let queue = session.store().queued_jobs().unwrap();
    assert_eq!(queue.len(), 2);
    store.apply(&queue.into_iter().fold(Batch::new(), |batch, job| batch.with(Change::QueueJob(job)))).unwrap();
    let mut again = Session::with_registry(Box::new(store), Box::new(clock.clone()), crate::Registry::builtin().unwrap()).unwrap();
    again.start();
    let (first, _) = one(&mut again);
    written(&mut again, first, 2, Some("\"l2\""));
    let (_, second) = one(&mut again);
    assert_eq!(etag(&second), Some("\"l2\""));
}
