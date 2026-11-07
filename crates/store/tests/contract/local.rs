// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The contract of the local tables: the job queue, drafts, outbox, schedule, senders, message state and calendars.

use inboxcraft_model::{
    AccountId, Alert, AlertId, Calendar, CalendarEvent, CalendarEventId, CalendarId, Draft, DraftId, Due, EmailAddress, FolderId, Group, Job, JobId,
    JobKind, MessageState, OutboxState, Outgoing, ScheduleId, Scheduled, Screening, Sender,
};
use inboxcraft_store::{Change, Storage, StoreError};

use super::{at, m, message, put, two_accounts};

pub fn job(id: u64, account: u64) -> Job {
    Job::new(JobId::new(id), AccountId::new(account), JobKind::SyncFolderList)
}

pub fn draft(id: u64, account: u64, subject: &str) -> Draft {
    Draft { subject: subject.to_owned(), text: format!("text of {subject}"), ..Draft::new(DraftId::new(id), AccountId::new(account), at(1)) }
}

pub fn outgoing(draft: u64, state: OutboxState) -> Outgoing {
    Outgoing { draft: DraftId::new(draft), state }
}

pub fn scheduled(id: u64, days: i64, what: Due) -> Scheduled {
    Scheduled { id: ScheduleId::new(id), due: at(days), what }
}

pub fn sender(address: &str) -> Sender {
    Sender::new(EmailAddress::new(address).unwrap())
}

pub fn calendar(id: u64, account: u64, name: &str) -> Calendar {
    Calendar { id: CalendarId::new(id), account: AccountId::new(account), href: format!("/dav/{name}/"), name: name.to_owned(), colour: None }
}

pub fn event(id: u64, calendar: u64) -> CalendarEvent {
    CalendarEvent {
        id: CalendarEventId::new(id),
        calendar: CalendarId::new(calendar),
        href: format!("/dav/{id}.ics"),
        etag: Some(format!("\"{id}\"")),
        ical: format!("BEGIN:VCALENDAR\r\nUID:{id}@example.org\r\nEND:VCALENDAR\r\n"),
    }
}

pub fn alert(id: u64, event: u64, days: i64) -> Alert {
    Alert { id: AlertId::new(id), event: CalendarEventId::new(event), at: at(days), dismissed: false }
}

fn c(id: u64) -> CalendarId {
    CalendarId::new(id)
}

fn d(id: u64) -> DraftId {
    DraftId::new(id)
}

fn e(id: u64) -> CalendarEventId {
    CalendarEventId::new(id)
}

fn job_ids<S: Storage>(store: &S) -> Vec<u64> {
    store.queued_jobs().unwrap().iter().map(|job| job.id.get()).collect()
}

pub fn the_local_tables_start_empty<S: Storage>(store: S) {
    assert!(store.queued_jobs().unwrap().is_empty());
    assert!(store.drafts(AccountId::new(1)).unwrap().is_empty());
    assert_eq!(store.draft(d(1)).unwrap(), None);
    assert!(store.outbox().unwrap().is_empty());
    assert!(store.schedule().unwrap().is_empty());
    assert!(store.senders().unwrap().is_empty());
    assert_eq!(store.sender(&EmailAddress::new("anna@example.org").unwrap()).unwrap(), None);
    assert_eq!(store.message_state(m(1)).unwrap(), MessageState::default());
    assert!(store.message_states().unwrap().is_empty());
    assert!(store.calendars(AccountId::new(1)).unwrap().is_empty());
    assert_eq!(store.sync_token(c(1)).unwrap(), None);
    assert!(store.calendar_events(c(1)).unwrap().is_empty());
    assert_eq!(store.calendar_event(e(1)).unwrap(), None);
    assert!(store.alerts(e(1)).unwrap().is_empty());
}

pub fn jobs_wait_in_the_order_they_came<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(&mut store, vec![Change::QueueJob(job(5, 1)), Change::QueueJob(job(2, 2)), Change::QueueJob(job(9, 1))]).unwrap();
    assert_eq!(job_ids(&store), [5, 2, 9]);
    let moved = Job::new(JobId::new(2), AccountId::new(1), JobKind::Check { by_owner: true });
    put(&mut store, vec![Change::QueueJob(moved.clone()), Change::RemoveJob(JobId::new(5)), Change::RemoveJob(JobId::new(77))]).unwrap();
    assert_eq!(store.queued_jobs().unwrap(), [moved, job(9, 1)], "a job queued again keeps its place");
    put(&mut store, vec![Change::QueueJob(job(5, 2))]).unwrap();
    assert_eq!(job_ids(&store), [2, 9, 5], "a job that left and came again is last");
}

pub fn a_job_needs_its_account<S: Storage>(mut store: S) {
    assert_eq!(put(&mut store, vec![Change::QueueJob(job(1, 3))]), Err(StoreError::UnknownAccount(AccountId::new(3))));
    assert!(store.queued_jobs().unwrap().is_empty());
}

pub fn drafts_read_back_per_account<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(&mut store, vec![Change::PutDraft(draft(3, 1, "three")), Change::PutDraft(draft(1, 1, "one")), Change::PutDraft(draft(2, 2, "two"))])
        .unwrap();
    assert_eq!(store.drafts(AccountId::new(1)).unwrap(), [draft(1, 1, "one"), draft(3, 1, "three")]);
    assert_eq!(store.draft(d(2)).unwrap(), Some(draft(2, 2, "two")));
    put(&mut store, vec![Change::PutDraft(draft(3, 2, "moved")), Change::RemoveDraft(d(1)), Change::RemoveDraft(d(8))]).unwrap();
    assert!(store.drafts(AccountId::new(1)).unwrap().is_empty());
    assert_eq!(store.drafts(AccountId::new(2)).unwrap(), [draft(2, 2, "two"), draft(3, 2, "moved")], "the from account changed");
}

pub fn a_draft_needs_its_account<S: Storage>(mut store: S) {
    assert_eq!(put(&mut store, vec![Change::PutDraft(draft(1, 4, "x"))]), Err(StoreError::UnknownAccount(AccountId::new(4))));
}

pub fn the_outbox_follows_its_draft<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    assert_eq!(put(&mut store, vec![Change::PutOutgoing(outgoing(1, OutboxState::Waiting))]), Err(StoreError::UnknownDraft(d(1))));
    put(&mut store, vec![Change::PutDraft(draft(2, 1, "b")), Change::PutDraft(draft(1, 2, "a"))]).unwrap();
    let refused = OutboxState::Refused { reply: "550 5.1.1 no such user".to_owned() };
    put(&mut store, vec![Change::PutOutgoing(outgoing(2, OutboxState::Sending)), Change::PutOutgoing(outgoing(1, OutboxState::Waiting))]).unwrap();
    put(&mut store, vec![Change::PutOutgoing(outgoing(2, refused.clone()))]).unwrap();
    assert_eq!(store.outbox().unwrap(), [outgoing(1, OutboxState::Waiting), outgoing(2, refused)]);
    put(&mut store, vec![Change::RemoveOutgoing(d(1)), Change::RemoveOutgoing(d(9)), Change::RemoveDraft(d(2))]).unwrap();
    assert!(store.outbox().unwrap().is_empty(), "a removed draft takes its outbox item");
}

/// Draft 4, messages 2 and 3, and alert 1 (of event 1 in calendar 1): what schedule entries name.
fn schedule_targets<S: Storage>(store: &mut S) {
    two_accounts(store);
    put(
        store,
        vec![
            Change::PutDraft(draft(4, 1, "later")),
            Change::PutMessage(message(2, 1, 1, 2, "two", Some(1))),
            Change::PutMessage(message(3, 1, 1, 3, "three", Some(1))),
            Change::PutCalendar(calendar(1, 1, "home")),
            Change::PutCalendarEvent(event(1, 1)),
            Change::PutAlert(alert(1, 1, 1)),
        ],
    )
    .unwrap();
}

pub fn the_schedule_reads_back_by_due_time<S: Storage>(mut store: S) {
    schedule_targets(&mut store);
    put(
        &mut store,
        vec![
            Change::PutScheduled(scheduled(1, 30, Due::Send(d(4)))),
            Change::PutScheduled(scheduled(3, 10, Due::Unsnooze(m(2)))),
            Change::PutScheduled(scheduled(2, 30, Due::FollowUp(m(3)))),
            Change::PutScheduled(scheduled(4, -5, Due::Alert(AlertId::new(1)))),
        ],
    )
    .unwrap();
    let ids = |store: &S| store.schedule().unwrap().iter().map(|entry| entry.id.get()).collect::<Vec<_>>();
    assert_eq!(ids(&store), [4, 3, 1, 2], "by due time, then id");
    put(&mut store, vec![Change::PutScheduled(scheduled(4, 40, Due::Alert(AlertId::new(1)))), Change::RemoveScheduled(ScheduleId::new(3))]).unwrap();
    assert_eq!(ids(&store), [1, 2, 4]);
    assert_eq!(store.schedule().unwrap().last(), Some(&scheduled(4, 40, Due::Alert(AlertId::new(1)))));
    put(&mut store, vec![Change::RemoveScheduled(ScheduleId::new(99))]).unwrap();
}

pub fn a_schedule_entry_needs_what_it_names<S: Storage>(mut store: S) {
    schedule_targets(&mut store);
    let refused = |store: &mut S, what| put(store, vec![Change::PutScheduled(scheduled(1, 1, what))]);
    assert_eq!(refused(&mut store, Due::Send(d(9))), Err(StoreError::UnknownDraft(d(9))));
    assert_eq!(refused(&mut store, Due::Unsnooze(m(9))), Err(StoreError::UnknownMessage(m(9))));
    assert_eq!(refused(&mut store, Due::FollowUp(m(9))), Err(StoreError::UnknownMessage(m(9))));
    assert_eq!(refused(&mut store, Due::Alert(AlertId::new(9))), Err(StoreError::UnknownAlert(AlertId::new(9))));
    assert!(store.schedule().unwrap().is_empty());
    put(&mut store, vec![Change::PutScheduled(scheduled(1, 1, Due::Send(d(4)))), Change::PutScheduled(scheduled(1, 2, Due::Unsnooze(m(2))))])
        .unwrap();
    put(&mut store, vec![Change::RemoveDraft(d(4))]).unwrap();
    assert_eq!(store.schedule().unwrap(), [scheduled(1, 2, Due::Unsnooze(m(2)))], "a replaced entry names its new target only");
}

/// An id can be handed out again once the record holding it is gone: an entry left behind would act on another one.
pub fn a_schedule_entry_leaves_with_what_it_names<S: Storage>(mut store: S) {
    schedule_targets(&mut store);
    put(
        &mut store,
        vec![
            Change::PutScheduled(scheduled(1, 1, Due::Send(d(4)))),
            Change::PutScheduled(scheduled(2, 1, Due::Unsnooze(m(2)))),
            Change::PutScheduled(scheduled(3, 1, Due::FollowUp(m(2)))),
            Change::PutScheduled(scheduled(4, 1, Due::FollowUp(m(3)))),
            Change::PutScheduled(scheduled(5, 1, Due::Alert(AlertId::new(1)))),
        ],
    )
    .unwrap();
    let ids = |store: &S| store.schedule().unwrap().iter().map(|entry| entry.id.get()).collect::<Vec<_>>();
    put(&mut store, vec![Change::RemoveMessage(m(2))]).unwrap();
    assert_eq!(ids(&store), [1, 4, 5], "a removed message takes its snooze and its follow-up");
    put(&mut store, vec![Change::RemoveAlert(AlertId::new(1))]).unwrap();
    assert_eq!(ids(&store), [1, 4], "a removed alert takes its entry");
    put(&mut store, vec![Change::RemoveDraft(d(4))]).unwrap();
    assert_eq!(ids(&store), [4], "a removed draft is not sent later");
    put(&mut store, vec![Change::RemoveFolder(FolderId::new(1))]).unwrap();
    assert!(store.schedule().unwrap().is_empty(), "and what goes with a folder takes its entries too");
}

pub fn a_removed_event_takes_the_entries_of_its_alerts<S: Storage>(mut store: S) {
    schedule_targets(&mut store);
    put(&mut store, vec![Change::PutScheduled(scheduled(1, 1, Due::Alert(AlertId::new(1))))]).unwrap();
    put(&mut store, vec![Change::RemoveCalendarEvent(e(1))]).unwrap();
    assert!(store.schedule().unwrap().is_empty());
}

pub fn senders_are_one_per_address_without_case<S: Storage>(mut store: S) {
    let mut anna = sender("Anna@Example.org");
    anna.known = true;
    put(&mut store, vec![Change::PutSender(sender("zoe@example.org")), Change::PutSender(anna.clone())]).unwrap();
    assert_eq!(store.sender(&EmailAddress::new("anna@example.ORG").unwrap()).unwrap(), Some(anna));
    let blocked = Sender { screening: Some(Screening::Blocked), group: Some(Group::Updates), ..sender("ANNA@example.org") };
    put(&mut store, vec![Change::PutSender(blocked.clone())]).unwrap();
    assert_eq!(store.senders().unwrap(), [blocked, sender("zoe@example.org")]);
    put(&mut store, vec![Change::RemoveSender(EmailAddress::new("Zoe@example.org").unwrap())]).unwrap();
    put(&mut store, vec![Change::RemoveSender(EmailAddress::new("nobody@example.org").unwrap())]).unwrap();
    assert_eq!(store.senders().unwrap().len(), 1);
}

pub fn message_state_is_kept_with_its_message<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let pinned = MessageState { pinned: true, ..MessageState::default() };
    assert_eq!(put(&mut store, vec![Change::SetMessageState(m(1), pinned.clone())]), Err(StoreError::UnknownMessage(m(1))));
    put(
        &mut store,
        vec![
            Change::PutMessage(message(1, 1, 1, 1, "one", Some(1))),
            Change::PutMessage(message(2, 1, 1, 2, "two", Some(1))),
            Change::PutMessage(message(3, 1, 2, 1, "three", Some(1))),
        ],
    )
    .unwrap();
    let done = MessageState { done: Some(FolderId::new(1)), snoozed_until: Some(at(3)), follow_up: Some(at(4)), parked: true, pinned: false };
    put(&mut store, vec![Change::SetMessageState(m(2), done.clone()), Change::SetMessageState(m(1), pinned.clone())]).unwrap();
    assert_eq!(store.message_state(m(2)).unwrap(), done);
    put(&mut store, vec![Change::PutMessage(message(2, 1, 2, 7, "two, archived", Some(1)))]).unwrap();
    assert_eq!(store.message_states().unwrap(), [(m(1), pinned), (m(2), done)], "a moved message keeps its state");
    put(&mut store, vec![Change::SetMessageState(m(1), MessageState::default()), Change::SetMessageState(m(3), MessageState::default())]).unwrap();
    assert_eq!(store.message_states().unwrap().len(), 1, "no state is no row");
    put(&mut store, vec![Change::RemoveFolder(FolderId::new(2))]).unwrap();
    assert!(store.message_states().unwrap().is_empty(), "a removed message takes its state");
}

pub fn calendars_hold_events_and_alerts<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    assert_eq!(put(&mut store, vec![Change::PutCalendar(calendar(1, 7, "home"))]), Err(StoreError::UnknownAccount(AccountId::new(7))));
    assert_eq!(put(&mut store, vec![Change::SetSyncToken(c(1), Some("t".to_owned()))]), Err(StoreError::UnknownCalendar(c(1))));
    assert_eq!(put(&mut store, vec![Change::PutCalendarEvent(event(1, 1))]), Err(StoreError::UnknownCalendar(c(1))));
    assert_eq!(put(&mut store, vec![Change::PutAlert(alert(1, 1, 1))]), Err(StoreError::UnknownCalendarEvent(e(1))));
    put(
        &mut store,
        vec![
            Change::PutCalendar(calendar(2, 1, "work")),
            Change::PutCalendar(calendar(1, 1, "home")),
            Change::PutCalendar(calendar(3, 2, "other")),
            Change::SetSyncToken(c(1), Some("http://example.org/sync/1".to_owned())),
            Change::PutCalendarEvent(event(5, 1)),
            Change::PutCalendarEvent(event(4, 1)),
            Change::PutCalendarEvent(event(6, 2)),
            Change::PutAlert(alert(2, 4, 9)),
            Change::PutAlert(alert(3, 4, 8)),
            Change::PutAlert(alert(1, 5, 8)),
        ],
    )
    .unwrap();
    assert_eq!(store.calendars(AccountId::new(1)).unwrap(), [calendar(1, 1, "home"), calendar(2, 1, "work")]);
    let renamed = Calendar { colour: Some("#ff8800".to_owned()), ..calendar(1, 1, "Home") };
    put(&mut store, vec![Change::PutCalendar(renamed.clone())]).unwrap();
    assert_eq!(store.calendars(AccountId::new(1)).unwrap().first(), Some(&renamed));
    assert_eq!(store.sync_token(c(1)).unwrap().as_deref(), Some("http://example.org/sync/1"), "a replaced calendar keeps its token");
    assert_eq!(store.calendar_events(c(1)).unwrap(), [event(4, 1), event(5, 1)]);
    assert_eq!(store.calendar_event(e(6)).unwrap(), Some(event(6, 2)));
    let ids = |store: &S, event| store.alerts(e(event)).unwrap().iter().map(|alert| alert.id.get()).collect::<Vec<_>>();
    assert_eq!(ids(&store, 4), [3, 2], "by time");
    let changed = CalendarEvent { etag: Some("\"5b\"".to_owned()), ..event(5, 1) };
    put(&mut store, vec![Change::PutCalendarEvent(changed.clone()), Change::SetSyncToken(c(1), None)]).unwrap();
    assert_eq!(store.calendar_event(e(5)).unwrap(), Some(changed));
    assert_eq!(store.sync_token(c(1)).unwrap(), None);
    put(&mut store, vec![Change::RemoveAlert(AlertId::new(3)), Change::RemoveAlert(AlertId::new(30))]).unwrap();
    assert_eq!(ids(&store, 4), [2]);
    put(&mut store, vec![Change::RemoveCalendarEvent(e(4)), Change::RemoveCalendarEvent(e(40))]).unwrap();
    assert!(store.alerts(e(4)).unwrap().is_empty(), "a removed event takes its alerts");
    put(&mut store, vec![Change::RemoveCalendar(c(1)), Change::RemoveCalendar(c(10))]).unwrap();
    assert_eq!(store.calendar_event(e(5)).unwrap(), None, "a removed calendar takes its events");
    assert!(store.alerts(e(5)).unwrap().is_empty());
    assert_eq!(store.calendar_events(c(2)).unwrap(), [event(6, 2)]);
}

pub fn a_removed_account_takes_its_local_records<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    put(
        &mut store,
        vec![
            Change::QueueJob(job(1, 1)),
            Change::QueueJob(job(2, 2)),
            Change::PutDraft(draft(1, 1, "mine")),
            Change::PutOutgoing(outgoing(1, OutboxState::Waiting)),
            Change::PutDraft(draft(2, 2, "other")),
            Change::PutMessage(message(1, 1, 1, 1, "x", Some(1))),
            Change::SetMessageState(m(1), MessageState { pinned: true, ..MessageState::default() }),
            Change::PutCalendar(calendar(1, 1, "home")),
            Change::PutCalendarEvent(event(1, 1)),
            Change::PutAlert(alert(1, 1, 1)),
            Change::PutScheduled(scheduled(1, 1, Due::Send(d(1)))),
            Change::PutSender(sender("anna@example.org")),
        ],
    )
    .unwrap();
    put(&mut store, vec![Change::RemoveAccount(AccountId::new(1))]).unwrap();
    assert_eq!(job_ids(&store), [2]);
    assert_eq!(store.draft(d(1)).unwrap(), None);
    assert_eq!(store.draft(d(2)).unwrap(), Some(draft(2, 2, "other")));
    assert!(store.outbox().unwrap().is_empty());
    assert!(store.message_states().unwrap().is_empty());
    assert!(store.calendars(AccountId::new(1)).unwrap().is_empty());
    assert_eq!(store.calendar_event(e(1)).unwrap(), None);
    assert!(store.alerts(e(1)).unwrap().is_empty());
    assert!(store.schedule().unwrap().is_empty(), "the account's draft took its schedule entry");
    assert_eq!(store.senders().unwrap().len(), 1, "senders are across accounts");
}

pub fn a_refused_batch_changes_no_local_record<S: Storage>(mut store: S) {
    two_accounts(&mut store);
    let refused = put(
        &mut store,
        vec![
            Change::QueueJob(job(1, 1)),
            Change::PutDraft(draft(1, 1, "x")),
            Change::PutOutgoing(outgoing(1, OutboxState::Waiting)),
            Change::PutScheduled(scheduled(1, 1, Due::Send(d(1)))),
            Change::PutSender(sender("anna@example.org")),
            Change::PutCalendar(calendar(1, 1, "home")),
            Change::PutCalendarEvent(event(1, 1)),
            Change::PutAlert(alert(1, 1, 1)),
            Change::PutAlert(alert(2, 9, 1)),
        ],
    );
    assert_eq!(refused, Err(StoreError::UnknownCalendarEvent(e(9))));
    the_local_tables_start_empty(store);
}
