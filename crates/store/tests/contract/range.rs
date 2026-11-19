// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The contract for ids no store can hold: past `i64::MAX`, the largest key SQLite takes. Such an id names nothing,
//! so every kind of id gives what an unknown id of its kind gives: nothing to read, the kind's `Unknown…` error for a
//! change that needs the thing, and nothing removed. Never a database failure.

use inboxcraft_model::{
    AccountId, AlertId, CalendarEventId, CalendarId, DraftId, Due, Flags, FolderId, FolderState, JobId, MessageState, OutboxState, ScheduleId,
    UidValidity,
};
use inboxcraft_store::{Change, Search, Storage, StoreError};

use super::{alert, at, body, calendar, draft, event, folder, job, m, message, outgoing, put, scheduled, two_accounts};

/// `u64::MAX`, the first id past `i64::MAX`, and `i64::MAX` itself, which a store can hold but this one does not.
pub const PAST: [u64; 3] = [u64::MAX, i64::MAX.unsigned_abs() + 1, i64::MAX.unsigned_abs()];

/// Two accounts, message 1 with a body, draft 1 in the outbox and scheduled, job 1, calendar 1 with event 1 and alert 1.
fn held<S: Storage>(store: &mut S) {
    two_accounts(store);
    put(
        store,
        vec![
            Change::PutMessage(message(1, 1, 1, 1, "one", Some(1))),
            Change::PutBody(m(1), body("hello")),
            Change::QueueJob(job(1, 1)),
            Change::PutDraft(draft(1, 1, "a")),
            Change::PutOutgoing(outgoing(1, OutboxState::Waiting)),
            Change::PutScheduled(scheduled(1, 1, Due::Send(DraftId::new(1)))),
            Change::PutCalendar(calendar(1, 1, "home")),
            Change::PutCalendarEvent(event(1, 1)),
            Change::PutAlert(alert(1, 1, 1)),
        ],
    )
    .unwrap();
}

/// Everything the store holds, written out, to see that nothing changed.
pub fn everything<S: Storage>(store: &S) -> String {
    let mut all =
        format!("{:?} {:?} {:?} {:?} {:?}", store.accounts(), store.queued_jobs(), store.outbox(), store.schedule(), store.message_states());
    for account in store.accounts().unwrap() {
        let account = account.id();
        all.push_str(&format!(" {:?} {:?}", store.drafts(account), store.calendars(account)));
        for folder in store.folders(account).unwrap() {
            all.push_str(&format!(" {folder:?} {:?}", store.folder_state(folder.id())));
            for entry in store.messages(folder.id()).unwrap() {
                let id = entry.message().id();
                let text = store.body(id).map(|body| body.map(|body| body.text().to_owned()));
                all.push_str(&format!(" {entry:?} {text:?} {:?}", store.message_state(id)));
            }
        }
        for calendar in store.calendars(account).unwrap() {
            all.push_str(&format!(" {:?}", store.sync_token(calendar.id)));
            for event in store.calendar_events(calendar.id).unwrap() {
                all.push_str(&format!(" {event:?} {:?}", store.alerts(event.id)));
            }
        }
    }
    all
}

pub fn an_id_past_what_a_store_holds_reads_as_unknown<S: Storage>(mut store: S) {
    held(&mut store);
    for id in PAST {
        reads_nothing(&store, id);
    }
}

/// Every read that takes an id finds nothing for `id`.
pub fn reads_nothing<S: Storage>(store: &S, id: u64) {
    let (account, folder, message) = (AccountId::new(id), FolderId::new(id), m(id));
    assert_eq!(store.folders(account), Ok(vec![]), "{id}");
    assert_eq!(store.folder_state(folder), Ok(None), "{id}");
    assert_eq!(store.messages(folder).map(|entries| entries.len()), Ok(0), "{id}");
    assert_eq!(store.message(message).map(|entry| entry.is_none()), Ok(true), "{id}");
    assert_eq!(store.body(message).map(|body| body.is_none()), Ok(true), "{id}");
    assert_eq!(store.message_state(message), Ok(MessageState::default()), "{id}");
    assert_eq!(store.search(&Search::new().with_account(account)), Ok(vec![]), "{id}: a filter, not everything");
    assert_eq!(store.search(&Search::new().with_folder(folder)), Ok(vec![]), "{id}: a filter, not everything");
    assert_eq!(store.missing_bodies(folder, at(0)), Ok(vec![]), "{id}");
    assert_eq!(store.drafts(account), Ok(vec![]), "{id}");
    assert_eq!(store.draft(DraftId::new(id)), Ok(None), "{id}");
    assert_eq!(store.calendars(account), Ok(vec![]), "{id}");
    assert_eq!(store.sync_token(CalendarId::new(id)), Ok(None), "{id}");
    assert_eq!(store.calendar_events(CalendarId::new(id)), Ok(vec![]), "{id}");
    assert_eq!(store.calendar_event(CalendarEventId::new(id)), Ok(None), "{id}");
    assert_eq!(store.alerts(CalendarEventId::new(id)), Ok(vec![]), "{id}");
    assert_eq!(store.alert(AlertId::new(id)), Ok(None), "{id}");
}

pub fn a_change_that_needs_an_id_past_what_a_store_holds_is_refused_as_unknown<S: Storage>(mut store: S) {
    held(&mut store);
    let before = everything(&store);
    for id in PAST {
        for (change, refused) in refusals(id) {
            let what = format!("{change:?}");
            assert_eq!(put(&mut store, vec![change]), Err(refused), "{what}");
        }
    }
    assert_eq!(everything(&store), before, "a refused change changed something");
}

pub fn a_removal_of_an_id_past_what_a_store_holds_removes_nothing<S: Storage>(mut store: S) {
    held(&mut store);
    let before = everything(&store);
    for id in PAST {
        let removals = removals(id);
        assert_eq!(put(&mut store, removals), Ok(()), "{id}");
    }
    assert_eq!(everything(&store), before, "a removal of nothing removed something");
}

/// Every change that needs a thing named by `id`, and the unknown error it is refused with.
pub fn refusals(id: u64) -> Vec<(Change, StoreError)> {
    let (account, folder, message, draft_id) = (AccountId::new(id), FolderId::new(id), m(id), DraftId::new(id));
    let state = FolderState::new(UidValidity::new(7).unwrap());
    let pinned = MessageState { pinned: true, ..MessageState::default() };
    vec![
        (Change::SetFolderState(folder, state), StoreError::UnknownFolder(folder)),
        (Change::SetFlags(message, Flags::default()), StoreError::UnknownMessage(message)),
        (Change::PutBody(message, body("x")), StoreError::UnknownMessage(message)),
        (Change::MarkOpened(message), StoreError::UnknownMessage(message)),
        (Change::SetMessageState(message, pinned), StoreError::UnknownMessage(message)),
        (Change::PutFolder(folder_of(account)), StoreError::UnknownAccount(account)),
        (Change::PutMessage(message_in(folder)), StoreError::UnknownFolder(folder)),
        (Change::PutMessage(super::message(9, id, 1, 9, "x", None)), StoreError::WrongAccount { folder: FolderId::new(1), account }),
        (Change::QueueJob(job(9, id)), StoreError::UnknownAccount(account)),
        (Change::PutDraft(draft(9, id, "x")), StoreError::UnknownAccount(account)),
        (Change::PutOutgoing(outgoing(id, OutboxState::Waiting)), StoreError::UnknownDraft(draft_id)),
        (Change::PutScheduled(scheduled(9, 1, Due::Send(draft_id))), StoreError::UnknownDraft(draft_id)),
        (Change::PutScheduled(scheduled(9, 1, Due::Unsnooze(message))), StoreError::UnknownMessage(message)),
        (Change::PutScheduled(scheduled(9, 1, Due::Alert(AlertId::new(id)))), StoreError::UnknownAlert(AlertId::new(id))),
        (Change::PutCalendar(calendar(9, id, "x")), StoreError::UnknownAccount(account)),
        (Change::SetSyncToken(CalendarId::new(id), Some("t".to_owned())), StoreError::UnknownCalendar(CalendarId::new(id))),
        (Change::PutCalendarEvent(event(9, id)), StoreError::UnknownCalendar(CalendarId::new(id))),
        (Change::PutAlert(alert(9, id, 1)), StoreError::UnknownCalendarEvent(CalendarEventId::new(id))),
    ]
}

/// Every removal of a thing named by `id`.
pub fn removals(id: u64) -> Vec<Change> {
    vec![
        Change::RemoveAccount(AccountId::new(id)),
        Change::RemoveFolder(FolderId::new(id)),
        Change::RemoveMessage(m(id)),
        Change::RemoveBody(m(id)),
        Change::RemoveJob(JobId::new(id)),
        Change::RemoveDraft(DraftId::new(id)),
        Change::RemoveOutgoing(DraftId::new(id)),
        Change::RemoveScheduled(ScheduleId::new(id)),
        Change::RemoveCalendar(CalendarId::new(id)),
        Change::RemoveCalendarEvent(CalendarEventId::new(id)),
        Change::RemoveAlert(AlertId::new(id)),
    ]
}

fn folder_of(account: AccountId) -> inboxcraft_model::Folder {
    folder(9, account.get(), "Elsewhere")
}

fn message_in(folder: FolderId) -> inboxcraft_model::Message {
    message(9, 1, folder.get(), 9, "x", None)
}

/// The ids at the edges of what a store can hold: `i64::MAX`, the largest key, 0, the smallest, and 1.
const EDGES: [u64; 3] = [i64::MAX.unsigned_abs(), 0, 1];

/// The ids past what a store can hold, which must reach none of the [`EDGES`].
pub const FAR: [u64; 2] = [u64::MAX, i64::MAX.unsigned_abs() + 1];

/// For each of the [`EDGES`]: an account with a folder, a message with a body and a state, a job, a draft in the
/// outbox and scheduled, and a calendar with a sync token, an event and an alert, each with the edge as its id.
fn edges<S: Storage>(store: &mut S) {
    for id in EDGES {
        let pinned = MessageState { pinned: true, ..MessageState::default() };
        put(
            store,
            vec![
                Change::PutAccount(super::account(id)),
                Change::PutFolder(folder(id, id, "INBOX")),
                Change::PutMessage(message(id, id, id, 1, "edge", Some(1))),
                Change::PutBody(m(id), body("edge")),
                Change::SetMessageState(m(id), pinned),
                Change::QueueJob(job(id, id)),
                Change::PutDraft(draft(id, id, "edge")),
                Change::PutOutgoing(outgoing(id, OutboxState::Waiting)),
                Change::PutScheduled(scheduled(id, 1, Due::Send(DraftId::new(id)))),
                Change::PutCalendar(calendar(id, id, "edge")),
                Change::SetSyncToken(CalendarId::new(id), Some(format!("token {id}"))),
                Change::PutCalendarEvent(event(id, id)),
                Change::PutAlert(alert(id, id, 1)),
            ],
        )
        .unwrap();
    }
}

/// An id past what a store can hold reaches no row at the edge of what it can: not `i64::MAX` (the key a saturating
/// conversion would give), not 0 and not 1. Nothing is found, changed or removed.
pub fn an_id_past_what_a_store_holds_reaches_no_edge_row<S: Storage>(mut store: S) {
    edges(&mut store);
    let before = everything(&store);
    for edge in EDGES {
        assert!(store.message(m(edge)).unwrap().is_some() && store.draft(DraftId::new(edge)).unwrap().is_some(), "{edge}");
    }
    for id in FAR {
        reads_nothing(&store, id);
        for (change, refused) in refusals(id) {
            let what = format!("{change:?}");
            assert_eq!(put(&mut store, vec![change]), Err(refused), "{what}");
        }
        assert_eq!(put(&mut store, removals(id)), Ok(()), "{id}");
    }
    assert_eq!(everything(&store), before, "a far id reached a row at the edge");
}
