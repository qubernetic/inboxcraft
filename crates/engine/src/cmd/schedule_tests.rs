// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `schedule.presets`, `schedule.list` and `schedule.cancel`.

use inboxcraft_model::{AccountId, Draft, DraftId, Due, MessageId, ScheduleId, Scheduled};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{FakeClock, Offset, OffsetTable};
use serde_json::{Value, json};

use crate::test_support::{NOW, at, mailbox};
use crate::{AgentClass, Caller, EngineError, Session};

fn session() -> Session {
    let mut store = mailbox();
    let batch = Batch::new()
        .with(Change::PutDraft(Draft::new(DraftId::new(3), AccountId::new(1), at(NOW))))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(2), due: at(NOW + 3600), what: Due::Send(DraftId::new(3)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(1), due: at(NOW - 60), what: Due::Unsnooze(MessageId::new(2)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(4), due: at(NOW + 60), what: Due::FollowUp(MessageId::new(5)) }));
    store.apply(&batch).unwrap();
    Session::new(Box::new(store), Box::new(FakeClock::new(at(NOW)))).unwrap()
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

#[test]
fn the_classes_are_safe_for_reading_and_owner_for_cancelling() {
    let session = session();
    for (id, class) in [("schedule.presets", AgentClass::Safe), ("schedule.list", AgentClass::Safe), ("schedule.cancel", AgentClass::Owner)] {
        assert_eq!(session.registry().get(id).map(|spec| spec.class), Some(Some(class)), "{id}");
    }
}

#[test]
fn presets_answers_the_presets_ahead_in_utc_until_the_offsets_are_given() {
    // NOW is 09:00 UTC on 5 November 2026.
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "schedule.presets", &json!({})).unwrap();
    assert_eq!(
        answer,
        json!({ "presets": [
            { "preset": "later_today", "at": "2026-11-05T12:00:00Z" },
            { "preset": "this_evening", "at": "2026-11-05T20:00:00Z" },
            { "preset": "tomorrow_morning", "at": "2026-11-06T08:00:00Z" },
            { "preset": "this_weekend", "at": "2026-11-07T09:00:00Z" },
            { "preset": "next_week", "at": "2026-11-09T09:00:00Z" },
        ] })
    );
    session.set_offsets(Box::new(OffsetTable::new(Offset::from_minutes(60).unwrap())));
    let answer = session.run("schedule.presets", &json!({})).unwrap();
    assert_eq!(answer["presets"][1], json!({ "preset": "this_evening", "at": "2026-11-05T19:00:00Z" }));
    assert!(bad(session.run("schedule.presets", &json!({ "at": "now" }))));
}

#[test]
fn next_week_follows_the_first_day_of_the_week_the_owner_set() {
    // NOW is Thursday 5 November 2026: with weeks from Sunday, next week starts on Sunday 8 November.
    let mut session = session();
    session.run("week.set", &json!({ "first_day": "sunday" })).unwrap();
    let answer = session.run("schedule.presets", &json!({})).unwrap();
    assert_eq!(answer["presets"][4], json!({ "preset": "next_week", "at": "2026-11-08T09:00:00Z" }));
    assert_eq!(answer["presets"][3], json!({ "preset": "this_weekend", "at": "2026-11-07T09:00:00Z" }));
}

#[test]
fn list_answers_every_entry_by_its_time_with_what_it_does() {
    let mut session = session();
    let mut answer = session.run_as(Caller::Agent, "schedule.list", &json!({})).unwrap();
    // The fields a row shows (account, subject, who) are the next test's; these are the old ones, kept as they were.
    for entry in answer["entries"].as_array_mut().unwrap() {
        let entry = entry.as_object_mut().unwrap();
        for added in ["account", "subject", "who"] {
            assert!(entry.remove(added).is_some(), "{added}");
        }
    }
    assert_eq!(
        answer,
        json!({ "entries": [
            { "id": 1, "due": "2026-11-05T08:59:00Z", "what": "unsnooze", "message": 2 },
            { "id": 4, "due": "2026-11-05T09:01:00Z", "what": "follow_up", "message": 5 },
            { "id": 2, "due": "2026-11-05T10:00:00Z", "what": "send", "draft": 3 },
        ], "skipped": 0 })
    );
    assert!(bad(session.run("schedule.list", &json!({ "id": 1 }))));
}

#[test]
fn list_names_an_alert_by_its_id() {
    assert_eq!(super::schedule::what(&Due::Alert(inboxcraft_model::AlertId::new(9))), json!({ "what": "alert", "alert": 9 }));
}

#[test]
fn cancel_takes_one_entry_off_and_only_for_the_owner() {
    let mut session = session();
    let refused = session.run_as(Caller::Agent, "schedule.cancel", &json!({ "id": 2 }));
    assert!(matches!(refused, Err(EngineError::NotAllowed { .. })));
    assert_eq!(session.run("schedule.cancel", &json!({ "id": 2 })).unwrap(), json!({ "cancelled": 2 }));
    let left: Vec<u64> = session.store().schedule().unwrap().into_iter().map(|entry| entry.id.get()).collect();
    assert_eq!(left, vec![1, 4]);
    assert!(session.store().draft(DraftId::new(3)).unwrap().is_some(), "the draft stays");
}

#[test]
fn cancel_refuses_an_entry_that_is_not_there_and_junk() {
    let mut session = session();
    let missing = session.run("schedule.cancel", &json!({ "id": 99 })).unwrap_err();
    assert_eq!(missing.code(), "not_found");
    for junk in [json!({}), json!({ "id": "2" }), json!({ "id": -1 }), json!({ "id": 2, "more": true })] {
        assert!(bad(session.run("schedule.cancel", &junk)), "{junk}");
    }
    assert_eq!(session.store().schedule().unwrap().len(), 3);
}

/// A session whose send waits for a draft with recipients and a subject, beside the snooze and follow-up of [`session`].
fn rich_session(subject: &str, to: Vec<String>) -> Session {
    let mut store = mailbox();
    let mut draft = Draft::new(DraftId::new(3), AccountId::new(2), at(NOW));
    draft.subject = subject.to_owned();
    draft.to = to;
    let batch = Batch::new()
        .with(Change::PutDraft(draft))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(2), due: at(NOW + 3600), what: Due::Send(DraftId::new(3)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(1), due: at(NOW - 60), what: Due::Unsnooze(MessageId::new(2)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(4), due: at(NOW + 60), what: Due::FollowUp(MessageId::new(5)) }));
    store.apply(&batch).unwrap();
    Session::new(Box::new(store), Box::new(FakeClock::new(at(NOW)))).unwrap()
}

#[test]
fn list_says_what_each_entry_is_about_its_account_subject_and_who() {
    let to = vec!["Greta Lindqvist <greta@example.org>".to_owned(), "pavel@example.org".to_owned()];
    let mut session = rich_session("Thank-you cards: the proof", to);
    let answer = session.run_as(Caller::Agent, "schedule.list", &json!({})).unwrap();
    assert_eq!(
        answer,
        json!({ "entries": [
            { "id": 1, "due": "2026-11-05T08:59:00Z", "what": "unsnooze", "message": 2,
              "account": 1, "subject": "Lunch on Friday", "who": "Anna Example" },
            { "id": 4, "due": "2026-11-05T09:01:00Z", "what": "follow_up", "message": 5,
              "account": 1, "subject": "Re: Budget for 2026", "who": "Bob" },
            { "id": 2, "due": "2026-11-05T10:00:00Z", "what": "send", "draft": 3,
              "account": 2, "subject": "Thank-you cards: the proof", "who": "Greta Lindqvist, pavel@example.org" },
        ], "skipped": 0 })
    );
}

#[test]
fn list_gives_hostile_subjects_and_recipients_as_one_short_clean_line() {
    let long = "x".repeat(10_000);
    let subject = format!("Line one\r\nBcc: evil@example.org\0\u{202E}gnp.exe{long}");
    let to = vec![format!("\u{2066}Mallory\r\n{long} <m@example.org>"), "\0\r\n".to_owned()];
    let mut session = rich_session(&subject, to);
    let answer = session.run("schedule.list", &json!({})).unwrap();
    let send = answer["entries"].as_array().unwrap().iter().find(|entry| entry["what"] == "send").unwrap().clone();
    for field in ["subject", "who"] {
        let text = send[field].as_str().unwrap();
        assert!(text.chars().count() <= crate::notify::MAX_TEXT, "{field} is capped");
        assert!(!text.chars().any(|c| c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')), "{field}: {text:?}");
    }
    assert!(send["subject"].as_str().unwrap().starts_with("Line one Bcc: evil@example.org"));
    assert!(send["who"].as_str().unwrap().starts_with("Mallory x"));
}

/// A store that cannot read draft 3; everything else is the memory store's.
#[derive(Debug)]
struct OneBadDraft(inboxcraft_store::Memory);

#[rustfmt::skip]
impl Storage for OneBadDraft {
    fn config(&self) -> inboxcraft_store::StoreConfig { self.0.config() }
    fn apply(&mut self, batch: &Batch) -> Result<(), inboxcraft_store::StoreError> { self.0.apply(batch) }
    fn accounts(&self) -> Result<Vec<inboxcraft_model::Account>, inboxcraft_store::StoreError> { self.0.accounts() }
    fn folders(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Folder>, inboxcraft_store::StoreError> { self.0.folders(a) }
    fn folder_state(&self, f: inboxcraft_model::FolderId) -> Result<Option<inboxcraft_model::FolderState>, inboxcraft_store::StoreError> { self.0.folder_state(f) }
    fn messages(&self, f: inboxcraft_model::FolderId) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.messages(f) }
    fn message(&self, m: MessageId) -> Result<Option<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.message(m) }
    fn body(&self, m: MessageId) -> Result<Option<inboxcraft_store::Body>, inboxcraft_store::StoreError> { self.0.body(m) }
    fn search(&self, s: &inboxcraft_store::Search) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.search(s) }
    fn queued_jobs(&self) -> Result<Vec<inboxcraft_model::Job>, inboxcraft_store::StoreError> { self.0.queued_jobs() }
    fn drafts(&self, a: AccountId) -> Result<Vec<Draft>, inboxcraft_store::StoreError> { self.0.drafts(a) }
    fn newest_drafts(&self, a: AccountId, n: usize) -> Result<(u64, Vec<Draft>), inboxcraft_store::StoreError> { self.0.newest_drafts(a, n) }
    fn draft(&self, d: DraftId) -> Result<Option<Draft>, inboxcraft_store::StoreError> {
        if d == DraftId::new(3) { Err(inboxcraft_store::StoreError::Corrupt("draft 3")) } else { self.0.draft(d) }
    }
    fn outbox(&self) -> Result<Vec<inboxcraft_model::Outgoing>, inboxcraft_store::StoreError> { self.0.outbox() }
    fn schedule(&self) -> Result<Vec<Scheduled>, inboxcraft_store::StoreError> { self.0.schedule() }
    fn sender(&self, e: &inboxcraft_model::EmailAddress) -> Result<Option<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.0.sender(e) }
    fn senders(&self) -> Result<Vec<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.0.senders() }
    fn message_state(&self, m: MessageId) -> Result<inboxcraft_model::MessageState, inboxcraft_store::StoreError> { self.0.message_state(m) }
    fn message_states(&self) -> Result<Vec<(MessageId, inboxcraft_model::MessageState)>, inboxcraft_store::StoreError> { self.0.message_states() }
    fn calendars(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Calendar>, inboxcraft_store::StoreError> { self.0.calendars(a) }
    fn sync_token(&self, c: inboxcraft_model::CalendarId) -> Result<Option<String>, inboxcraft_store::StoreError> { self.0.sync_token(c) }
    fn calendar_events(&self, c: inboxcraft_model::CalendarId) -> Result<Vec<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_events(c) }
    fn calendar_event(&self, e: inboxcraft_model::CalendarEventId) -> Result<Option<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_event(e) }
    fn alerts(&self, e: inboxcraft_model::CalendarEventId) -> Result<Vec<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alerts(e) }
    fn alert(&self, a: inboxcraft_model::AlertId) -> Result<Option<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alert(a) }
    fn stale_bodies(&self, t: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.stale_bodies(t) }
    fn missing_bodies(&self, f: inboxcraft_model::FolderId, t: inboxcraft_time::Timestamp) -> Result<Vec<MessageId>, inboxcraft_store::StoreError> { self.0.missing_bodies(f, t) }
}

#[test]
fn list_skips_an_entry_it_cannot_read_and_counts_it() {
    let mut store = mailbox();
    let batch = Batch::new()
        .with(Change::PutDraft(Draft::new(DraftId::new(3), AccountId::new(1), at(NOW))))
        .with(Change::PutDraft(Draft::new(DraftId::new(6), AccountId::new(1), at(NOW))))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(2), due: at(NOW + 3600), what: Due::Send(DraftId::new(3)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(5), due: at(NOW + 7200), what: Due::Send(DraftId::new(6)) }))
        .with(Change::PutScheduled(Scheduled { id: ScheduleId::new(1), due: at(NOW - 60), what: Due::Unsnooze(MessageId::new(2)) }));
    store.apply(&batch).unwrap();
    let mut bad = Session::new(Box::new(OneBadDraft(store)), Box::new(FakeClock::new(at(NOW)))).unwrap();
    let answer = bad.run("schedule.list", &json!({})).unwrap();
    let ids: Vec<u64> = answer["entries"].as_array().unwrap().iter().map(|entry| entry["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, [1, 5], "the entry whose draft cannot be read is left out, the rest stand: {answer}");
    assert_eq!(answer["skipped"], 1);
    // With every entry read, nothing is skipped.
    assert_eq!(session().run("schedule.list", &json!({})).unwrap()["skipped"], 0);
}
