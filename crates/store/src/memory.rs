// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The in-memory backend: always built, used on wasm and in tests.

use std::collections::BTreeMap;
use std::sync::Arc;

use inboxcraft_model::{
    Account, AccountId, Alert, AlertId, Calendar, CalendarEvent, CalendarEventId, CalendarId, Draft, DraftId, Due, EmailAddress, Flags, Folder,
    FolderId, FolderState, Job, Message, MessageId, MessageState, Outgoing, ScheduleId, Scheduled, Sender, Uid,
};
use inboxcraft_time::Timestamp;

use crate::search::{Fields, has_phrase, order_key, words};
use crate::{Batch, Body, Change, Entry, Search, Storage, StoreConfig, StoreError};

/// A store that lives in memory and is gone with the process.
///
/// A batch is applied to a copy of the state that replaces it only when every change went through; bodies are shared
/// between the copies, so the copy costs the rows, not the sources.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    config: StoreConfig,
    state: State,
}

#[derive(Debug, Clone, Default)]
struct State {
    accounts: BTreeMap<AccountId, Account>,
    folders: BTreeMap<FolderId, (Folder, Option<FolderState>)>,
    messages: BTreeMap<MessageId, Held>,
    uids: BTreeMap<FolderId, BTreeMap<Uid, MessageId>>,
    /// The job queue, in order.
    jobs: Vec<Job>,
    drafts: BTreeMap<DraftId, Draft>,
    outbox: BTreeMap<DraftId, Outgoing>,
    schedule: BTreeMap<ScheduleId, Scheduled>,
    /// By address in lower case.
    senders: BTreeMap<String, Sender>,
    states: BTreeMap<MessageId, MessageState>,
    calendars: BTreeMap<CalendarId, (Calendar, Option<String>)>,
    events: BTreeMap<CalendarEventId, CalendarEvent>,
    alerts: BTreeMap<AlertId, Alert>,
}

/// The key a sender is kept under: its address in lower case.
pub(crate) fn sender_key(address: &EmailAddress) -> String {
    address.as_str().to_lowercase()
}

#[derive(Debug, Clone)]
struct Held {
    message: Message,
    fields: Fields,
    opened: bool,
    body: Option<Arc<(Body, Vec<String>)>>,
}

impl Held {
    fn entry(&self) -> Entry {
        Entry::new(self.message.clone(), self.opened, self.body.is_some())
    }

    fn is_stale(&self, cutoff: i64) -> bool {
        !self.opened && self.fields.date.is_some_and(|date| date < cutoff)
    }
}

impl Memory {
    /// An empty store.
    #[must_use]
    pub fn new(config: StoreConfig) -> Self {
        Self { config, state: State::default() }
    }
}

impl State {
    fn change(&mut self, change: &Change) -> Result<(), StoreError> {
        match change {
            Change::PutAccount(account) => {
                self.accounts.insert(account.id(), account.clone());
            }
            Change::RemoveAccount(id) => {
                self.accounts.remove(id);
                let folders: Vec<FolderId> =
                    self.folders.values().filter(|(folder, _)| folder.account() == *id).map(|(folder, _)| folder.id()).collect();
                for folder in folders {
                    self.remove_folder(folder);
                }
                self.jobs.retain(|job| job.account != *id);
                let drafts: Vec<DraftId> = self.drafts.values().filter(|draft| draft.account == *id).map(|draft| draft.id).collect();
                for draft in drafts {
                    self.remove_draft(draft);
                }
                let calendars: Vec<CalendarId> =
                    self.calendars.values().filter(|(calendar, _)| calendar.account == *id).map(|(calendar, _)| calendar.id).collect();
                for calendar in calendars {
                    self.remove_calendar(calendar);
                }
            }
            Change::PutFolder(folder) => self.put_folder(folder)?,
            Change::RemoveFolder(id) => self.remove_folder(*id),
            Change::SetFolderState(id, state) => {
                let (_, held) = self.folders.get_mut(id).ok_or(StoreError::UnknownFolder(*id))?;
                *held = Some(*state);
            }
            Change::PutMessage(message) => self.put_message(message)?,
            Change::SetFlags(id, flags) => {
                let held = self.held(*id)?;
                held.message = with_flags(&held.message, flags);
            }
            Change::RemoveMessage(id) => self.remove_message(*id),
            Change::PutBody(id, body) => {
                let words = words(body.text());
                self.held(*id)?.body = Some(Arc::new((body.clone(), words)));
            }
            Change::RemoveBody(id) => {
                if let Some(held) = self.messages.get_mut(id) {
                    held.body = None;
                }
            }
            Change::MarkOpened(id) => self.held(*id)?.opened = true,
            Change::QueueJob(job) => {
                self.account(job.account)?;
                match self.jobs.iter_mut().find(|queued| queued.id == job.id) {
                    Some(queued) => *queued = job.clone(),
                    None => self.jobs.push(job.clone()),
                }
            }
            Change::RemoveJob(id) => self.jobs.retain(|job| job.id != *id),
            Change::PutDraft(draft) => {
                self.account(draft.account)?;
                self.drafts.insert(draft.id, draft.clone());
            }
            Change::RemoveDraft(id) => self.remove_draft(*id),
            Change::PutOutgoing(outgoing) => {
                if !self.drafts.contains_key(&outgoing.draft) {
                    return Err(StoreError::UnknownDraft(outgoing.draft));
                }
                self.outbox.insert(outgoing.draft, outgoing.clone());
            }
            Change::RemoveOutgoing(id) => {
                self.outbox.remove(id);
            }
            Change::PutScheduled(entry) => {
                match entry.what {
                    Due::Send(id) if !self.drafts.contains_key(&id) => return Err(StoreError::UnknownDraft(id)),
                    Due::Unsnooze(id) | Due::FollowUp(id) if !self.messages.contains_key(&id) => return Err(StoreError::UnknownMessage(id)),
                    Due::Alert(id) if !self.alerts.contains_key(&id) => return Err(StoreError::UnknownAlert(id)),
                    _ => {}
                }
                self.schedule.insert(entry.id, entry.clone());
            }
            Change::RemoveScheduled(id) => {
                self.schedule.remove(id);
            }
            Change::PutSender(sender) => {
                self.senders.insert(sender_key(&sender.address), sender.clone());
            }
            Change::RemoveSender(address) => {
                self.senders.remove(&sender_key(address));
            }
            Change::SetMessageState(id, state) => {
                self.held(*id)?;
                if state.is_empty() {
                    self.states.remove(id);
                } else {
                    self.states.insert(*id, state.clone());
                }
            }
            Change::PutCalendar(calendar) => {
                self.account(calendar.account)?;
                let token = self.calendars.get(&calendar.id).and_then(|(_, token)| token.clone());
                self.calendars.insert(calendar.id, (calendar.clone(), token));
            }
            Change::RemoveCalendar(id) => self.remove_calendar(*id),
            Change::SetSyncToken(id, token) => {
                let (_, held) = self.calendars.get_mut(id).ok_or(StoreError::UnknownCalendar(*id))?;
                held.clone_from(token);
            }
            Change::PutCalendarEvent(event) => {
                if !self.calendars.contains_key(&event.calendar) {
                    return Err(StoreError::UnknownCalendar(event.calendar));
                }
                self.events.insert(event.id, event.clone());
            }
            Change::RemoveCalendarEvent(id) => self.remove_event(*id),
            Change::PutAlert(alert) => {
                if !self.events.contains_key(&alert.event) {
                    return Err(StoreError::UnknownCalendarEvent(alert.event));
                }
                self.alerts.insert(alert.id, alert.clone());
            }
            Change::RemoveAlert(id) => self.remove_alert(*id),
        }
        Ok(())
    }

    fn account(&self, id: AccountId) -> Result<(), StoreError> {
        if self.accounts.contains_key(&id) { Ok(()) } else { Err(StoreError::UnknownAccount(id)) }
    }

    fn remove_draft(&mut self, id: DraftId) {
        self.drafts.remove(&id);
        self.outbox.remove(&id);
        self.schedule.retain(|_, entry| entry.what != Due::Send(id));
    }

    fn remove_alert(&mut self, id: AlertId) {
        self.alerts.remove(&id);
        self.schedule.retain(|_, entry| entry.what != Due::Alert(id));
    }

    fn remove_calendar(&mut self, id: CalendarId) {
        self.calendars.remove(&id);
        let events: Vec<CalendarEventId> = self.events.values().filter(|event| event.calendar == id).map(|event| event.id).collect();
        for event in events {
            self.remove_event(event);
        }
    }

    fn remove_event(&mut self, id: CalendarEventId) {
        self.events.remove(&id);
        let alerts: Vec<AlertId> = self.alerts.values().filter(|alert| alert.event == id).map(|alert| alert.id).collect();
        for alert in alerts {
            self.remove_alert(alert);
        }
    }

    fn held(&mut self, id: MessageId) -> Result<&mut Held, StoreError> {
        self.messages.get_mut(&id).ok_or(StoreError::UnknownMessage(id))
    }

    fn put_folder(&mut self, folder: &Folder) -> Result<(), StoreError> {
        self.account(folder.account())?;
        let state = match self.folders.get(&folder.id()) {
            Some((old, _)) if old.account() != folder.account() => {
                return Err(StoreError::WrongAccount { folder: folder.id(), account: folder.account() });
            }
            Some((_, state)) => *state,
            None => None,
        };
        self.folders.insert(folder.id(), (folder.clone(), state));
        Ok(())
    }

    fn remove_folder(&mut self, id: FolderId) {
        self.folders.remove(&id);
        let messages: Vec<MessageId> = self.messages.iter().filter(|(_, held)| held.message.folder() == id).map(|(id, _)| *id).collect();
        for message in messages {
            self.remove_message(message);
        }
    }

    fn put_message(&mut self, message: &Message) -> Result<(), StoreError> {
        let (folder, _) = self.folders.get(&message.folder()).ok_or(StoreError::UnknownFolder(message.folder()))?;
        if folder.account() != message.account() {
            return Err(StoreError::WrongAccount { folder: message.folder(), account: message.account() });
        }
        let (folder, uid) = (message.folder(), message.summary().uid());
        if self.uids.get(&folder).and_then(|uids| uids.get(&uid)).is_some_and(|other| *other != message.id()) {
            return Err(StoreError::DuplicateUid { folder, uid });
        }
        let old = self.messages.get(&message.id()).map(|old| (old.opened, old.body.clone(), old.message.clone()));
        let (opened, body) = match old {
            Some((opened, body, old)) => {
                self.forget_uid(&old);
                (opened, body)
            }
            None => (false, None),
        };
        self.uids.entry(folder).or_default().insert(uid, message.id());
        self.messages.insert(message.id(), Held { message: message.clone(), fields: Fields::of(message), opened, body });
        Ok(())
    }

    fn remove_message(&mut self, id: MessageId) {
        if let Some(held) = self.messages.remove(&id) {
            self.forget_uid(&held.message);
        }
        self.states.remove(&id);
        self.schedule.retain(|_, entry| !matches!(entry.what, Due::Unsnooze(message) | Due::FollowUp(message) if message == id));
    }

    fn forget_uid(&mut self, message: &Message) {
        if let Some(uids) = self.uids.get_mut(&message.folder()) {
            uids.remove(&message.summary().uid());
        }
    }
}

/// `message` with its flags replaced.
pub(crate) fn with_flags(message: &Message, flags: &Flags) -> Message {
    let summary = message.summary().clone().with_flags(flags.clone());
    Message::new(message.id(), message.account(), message.folder(), summary).with_thread(message.thread())
}

impl Storage for Memory {
    fn config(&self) -> StoreConfig {
        self.config
    }

    fn apply(&mut self, batch: &Batch) -> Result<(), StoreError> {
        let mut next = self.state.clone();
        for change in batch.changes() {
            next.change(change)?;
        }
        self.state = next;
        Ok(())
    }

    fn accounts(&self) -> Result<Vec<Account>, StoreError> {
        Ok(self.state.accounts.values().cloned().collect())
    }

    fn folders(&self, account: AccountId) -> Result<Vec<Folder>, StoreError> {
        Ok(self.state.folders.values().filter(|(folder, _)| folder.account() == account).map(|(folder, _)| folder.clone()).collect())
    }

    fn folder_state(&self, folder: FolderId) -> Result<Option<FolderState>, StoreError> {
        Ok(self.state.folders.get(&folder).and_then(|(_, state)| *state))
    }

    fn messages(&self, folder: FolderId) -> Result<Vec<Entry>, StoreError> {
        let ids = self.state.uids.get(&folder).into_iter().flat_map(BTreeMap::values);
        Ok(ids.filter_map(|id| self.state.messages.get(id)).map(Held::entry).collect())
    }

    fn message(&self, id: MessageId) -> Result<Option<Entry>, StoreError> {
        Ok(self.state.messages.get(&id).map(Held::entry))
    }

    fn body(&self, id: MessageId) -> Result<Option<Body>, StoreError> {
        Ok(self.state.messages.get(&id).and_then(|held| held.body.as_ref()).map(|body| body.0.clone()))
    }

    fn search(&self, search: &Search) -> Result<Vec<MessageId>, StoreError> {
        let mut found: Vec<(Option<i64>, MessageId)> = self
            .state
            .messages
            .values()
            .filter(|held| search.matches_fields(&held.message, &held.fields))
            .filter(|held| search.text().is_empty() || held.body.as_ref().is_some_and(|body| has_phrase(&body.1, search.text())))
            .map(|held| (held.fields.date, held.message.id()))
            .collect();
        found.sort_by_key(|(date, id)| order_key(*date, *id));
        Ok(found.into_iter().take(search.limit()).map(|(_, id)| id).collect())
    }

    fn queued_jobs(&self) -> Result<Vec<Job>, StoreError> {
        Ok(self.state.jobs.clone())
    }

    fn drafts(&self, account: AccountId) -> Result<Vec<Draft>, StoreError> {
        Ok(self.state.drafts.values().filter(|draft| draft.account == account).cloned().collect())
    }

    fn draft(&self, id: DraftId) -> Result<Option<Draft>, StoreError> {
        Ok(self.state.drafts.get(&id).cloned())
    }

    fn outbox(&self) -> Result<Vec<Outgoing>, StoreError> {
        Ok(self.state.outbox.values().cloned().collect())
    }

    fn schedule(&self) -> Result<Vec<Scheduled>, StoreError> {
        let mut entries: Vec<Scheduled> = self.state.schedule.values().cloned().collect();
        entries.sort_by_key(|entry| (entry.due.unix_seconds(), entry.id));
        Ok(entries)
    }

    fn sender(&self, address: &EmailAddress) -> Result<Option<Sender>, StoreError> {
        Ok(self.state.senders.get(&sender_key(address)).cloned())
    }

    fn senders(&self) -> Result<Vec<Sender>, StoreError> {
        Ok(self.state.senders.values().cloned().collect())
    }

    fn message_state(&self, id: MessageId) -> Result<MessageState, StoreError> {
        Ok(self.state.states.get(&id).cloned().unwrap_or_default())
    }

    fn message_states(&self) -> Result<Vec<(MessageId, MessageState)>, StoreError> {
        Ok(self.state.states.iter().map(|(id, state)| (*id, state.clone())).collect())
    }

    fn calendars(&self, account: AccountId) -> Result<Vec<Calendar>, StoreError> {
        Ok(self.state.calendars.values().filter(|(calendar, _)| calendar.account == account).map(|(calendar, _)| calendar.clone()).collect())
    }

    fn sync_token(&self, calendar: CalendarId) -> Result<Option<String>, StoreError> {
        Ok(self.state.calendars.get(&calendar).and_then(|(_, token)| token.clone()))
    }

    fn calendar_events(&self, calendar: CalendarId) -> Result<Vec<CalendarEvent>, StoreError> {
        Ok(self.state.events.values().filter(|event| event.calendar == calendar).cloned().collect())
    }

    fn calendar_event(&self, id: CalendarEventId) -> Result<Option<CalendarEvent>, StoreError> {
        Ok(self.state.events.get(&id).cloned())
    }

    fn alerts(&self, event: CalendarEventId) -> Result<Vec<Alert>, StoreError> {
        let mut alerts: Vec<Alert> = self.state.alerts.values().filter(|alert| alert.event == event).cloned().collect();
        alerts.sort_by_key(|alert| (alert.at.unix_seconds(), alert.id));
        Ok(alerts)
    }

    fn stale_bodies(&self, cutoff: Timestamp) -> Result<Vec<MessageId>, StoreError> {
        let cutoff = cutoff.unix_seconds();
        Ok(self.state.messages.values().filter(|held| held.body.is_some() && held.is_stale(cutoff)).map(|held| held.message.id()).collect())
    }

    fn missing_bodies(&self, folder: FolderId, cutoff: Timestamp) -> Result<Vec<MessageId>, StoreError> {
        let cutoff = cutoff.unix_seconds();
        Ok(self
            .messages(folder)?
            .into_iter()
            .filter_map(|entry| self.state.messages.get(&entry.message().id()))
            .filter(|held| held.body.is_none() && !held.is_stale(cutoff))
            .map(|held| held.message.id())
            .collect())
    }
}
