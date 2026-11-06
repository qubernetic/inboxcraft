// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The in-memory backend: always built, used on wasm and in tests.

use std::collections::BTreeMap;
use std::sync::Arc;

use inboxcraft_model::{Account, AccountId, Flags, Folder, FolderId, FolderState, Message, MessageId, Uid};
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
        }
        Ok(())
    }

    fn held(&mut self, id: MessageId) -> Result<&mut Held, StoreError> {
        self.messages.get_mut(&id).ok_or(StoreError::UnknownMessage(id))
    }

    fn put_folder(&mut self, folder: &Folder) -> Result<(), StoreError> {
        if !self.accounts.contains_key(&folder.account()) {
            return Err(StoreError::UnknownAccount(folder.account()));
        }
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
