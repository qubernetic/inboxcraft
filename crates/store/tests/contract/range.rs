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
fn everything<S: Storage>(store: &S) -> String {
    let mut all = format!("{:?} {:?} {:?} {:?}", store.accounts(), store.queued_jobs(), store.outbox(), store.schedule());
    for account in [1, 2] {
        let account = AccountId::new(account);
        all.push_str(&format!(" {:?} {:?} {:?}", store.folders(account), store.drafts(account), store.calendars(account)));
    }
    for folder in [1, 2, 3] {
        all.push_str(&format!(" {:?} {:?}", store.folder_state(FolderId::new(folder)), store.messages(FolderId::new(folder))));
    }
    all.push_str(&format!(
        " {:?} {:?} {:?} {:?}",
        store.body(m(1)).map(|body| body.map(|body| body.text().to_owned())),
        store.message_states(),
        store.calendar_events(CalendarId::new(1)),
        store.alerts(CalendarEventId::new(1))
    ));
    all
}

pub fn an_id_past_what_a_store_holds_reads_as_unknown<S: Storage>(mut store: S) {
    held(&mut store);
    for id in PAST {
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
}

pub fn a_change_that_needs_an_id_past_what_a_store_holds_is_refused_as_unknown<S: Storage>(mut store: S) {
    held(&mut store);
    let before = everything(&store);
    for id in PAST {
        let (account, folder, message, draft_id) = (AccountId::new(id), FolderId::new(id), m(id), DraftId::new(id));
        let state = FolderState::new(UidValidity::new(7).unwrap());
        let pinned = MessageState { pinned: true, ..MessageState::default() };
        for (change, refused) in [
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
        ] {
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
        let removals = vec![
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
        ];
        assert_eq!(put(&mut store, removals), Ok(()), "{id}");
    }
    assert_eq!(everything(&store), before, "a removal of nothing removed something");
}

fn folder_of(account: AccountId) -> inboxcraft_model::Folder {
    folder(9, account.get(), "Elsewhere")
}

fn message_in(folder: FolderId) -> inboxcraft_model::Message {
    message(9, 1, folder.get(), 9, "x", None)
}
