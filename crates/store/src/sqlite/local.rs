// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The local records in SQLite: the job queue, drafts, outbox, schedule, senders, message state and calendars.

use inboxcraft_model::{AccountId, Alert, Calendar, CalendarEvent, CalendarId, Draft, Job, MessageId, MessageState, Outgoing, Scheduled, Sender};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{exists, json, key};
use crate::StoreError;
use crate::memory::sender_key;

/// Whether the held row `id` of `table` is there.
fn held(tx: &Transaction<'_>, table: &str, id: i64) -> Result<bool, StoreError> {
    Ok(tx.query_row(&format!("SELECT 1 FROM {table} WHERE id = ?1"), [id], |_| Ok(())).optional()?.is_some())
}

/// Whether `account` is held, or the error that names it.
fn account(tx: &Transaction<'_>, account: AccountId) -> Result<i64, StoreError> {
    let id = key(account.get())?;
    if held(tx, "account", id)? { Ok(id) } else { Err(StoreError::UnknownAccount(account)) }
}

pub(super) fn queue_job(tx: &Transaction<'_>, job: &Job) -> Result<(), StoreError> {
    let account = account(tx, job.account)?;
    tx.execute(
        "INSERT INTO job (id, account, record) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET account = excluded.account, record = excluded.record",
        params![key(job.id.get())?, account, json(job)?],
    )?;
    Ok(())
}

pub(super) fn put_draft(tx: &Transaction<'_>, draft: &Draft) -> Result<(), StoreError> {
    let account = account(tx, draft.account)?;
    tx.execute(
        "INSERT INTO draft (id, account, record) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET account = excluded.account, record = excluded.record",
        params![key(draft.id.get())?, account, json(draft)?],
    )?;
    Ok(())
}

pub(super) fn put_outgoing(tx: &Transaction<'_>, outgoing: &Outgoing) -> Result<(), StoreError> {
    let draft = key(outgoing.draft.get())?;
    if !held(tx, "draft", draft)? {
        return Err(StoreError::UnknownDraft(outgoing.draft));
    }
    tx.execute(
        "INSERT INTO outbox (draft, record) VALUES (?1, ?2) ON CONFLICT (draft) DO UPDATE SET record = excluded.record",
        params![draft, json(outgoing)?],
    )?;
    Ok(())
}

pub(super) fn put_scheduled(tx: &Transaction<'_>, entry: &Scheduled) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO schedule (id, due, record) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET due = excluded.due, record = excluded.record",
        params![key(entry.id.get())?, entry.due.unix_seconds(), json(entry)?],
    )?;
    Ok(())
}

pub(super) fn put_sender(tx: &Transaction<'_>, sender: &Sender) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO sender (address, record) VALUES (?1, ?2) ON CONFLICT (address) DO UPDATE SET record = excluded.record",
        params![sender_key(&sender.address), json(sender)?],
    )?;
    Ok(())
}

pub(super) fn set_message_state(tx: &Transaction<'_>, id: MessageId, state: &MessageState) -> Result<(), StoreError> {
    let message = exists(tx, id)?;
    if state.is_empty() {
        tx.execute("DELETE FROM message_state WHERE message = ?1", [message])?;
    } else {
        tx.execute(
            "INSERT INTO message_state (message, record) VALUES (?1, ?2) ON CONFLICT (message) DO UPDATE SET record = excluded.record",
            params![message, json(state)?],
        )?;
    }
    Ok(())
}

pub(super) fn put_calendar(tx: &Transaction<'_>, calendar: &Calendar) -> Result<(), StoreError> {
    let account = account(tx, calendar.account)?;
    tx.execute(
        "INSERT INTO calendar (id, account, record) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET account = excluded.account, record = excluded.record",
        params![key(calendar.id.get())?, account, json(calendar)?],
    )?;
    Ok(())
}

pub(super) fn set_sync_token(tx: &Transaction<'_>, calendar: CalendarId, token: Option<&str>) -> Result<(), StoreError> {
    if tx.execute("UPDATE calendar SET sync_token = ?2 WHERE id = ?1", params![key(calendar.get())?, token])? == 0 {
        return Err(StoreError::UnknownCalendar(calendar));
    }
    Ok(())
}

pub(super) fn put_calendar_event(tx: &Transaction<'_>, event: &CalendarEvent) -> Result<(), StoreError> {
    let calendar = key(event.calendar.get())?;
    if !held(tx, "calendar", calendar)? {
        return Err(StoreError::UnknownCalendar(event.calendar));
    }
    tx.execute(
        "INSERT INTO calendar_event (id, calendar, record) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET calendar = excluded.calendar, record = excluded.record",
        params![key(event.id.get())?, calendar, json(event)?],
    )?;
    Ok(())
}

pub(super) fn put_alert(tx: &Transaction<'_>, alert: &Alert) -> Result<(), StoreError> {
    let event = key(alert.event.get())?;
    if !held(tx, "calendar_event", event)? {
        return Err(StoreError::UnknownCalendarEvent(alert.event));
    }
    tx.execute(
        "INSERT INTO alert (id, event, at, record) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (id) DO UPDATE SET event = excluded.event, at = excluded.at, record = excluded.record",
        params![key(alert.id.get())?, event, alert.at.unix_seconds(), json(alert)?],
    )?;
    Ok(())
}

/// Removes the row `id` of `table` (by its column `column`); nothing when it is not there.
pub(super) fn remove(tx: &Transaction<'_>, table: &str, column: &str, id: u64) -> Result<(), StoreError> {
    tx.execute(&format!("DELETE FROM {table} WHERE {column} = ?1"), [key(id)?])?;
    Ok(())
}
