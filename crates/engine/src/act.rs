// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Acting on messages: the change in the store at once, so the owner sees it, and a kept job that makes it on the
//! server.
//!
//! A moved message keeps its id. It goes to the folder it was moved to under a UID of ours, taken from the top of
//! the UID range down, with [`Moving`] noting where the server keeps it; the server's answer gives it its UID there
//! ([`crate::write`]). An act on a message whose move is on its way changes only the store: the server follows once
//! the answer names the message's place. The sample mailbox, and a folder not synced yet, get the change and no job.
//!
//! A message can also be replaced by a new copy of its own ([`rewrite`]): the server gets the copy (`APPEND`), and the
//! old copy goes only once the answer says the new one is there.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use inboxcraft_model::{AccountId, Flag, Flags, Folder, FolderId, FolderPath, JobKind, Message, Moving, Uid, UidSet, UidValidity};
use inboxcraft_store::{Change, Storage};

use crate::EngineError;
use crate::session::State;

/// What an act changes: the store changes, then the kept jobs for the server.
#[derive(Debug, Default)]
pub(crate) struct Effects {
    pub(crate) changes: Vec<Change>,
    pub(crate) jobs: Vec<(AccountId, JobKind)>,
}

impl Effects {
    pub(crate) fn change(&mut self, change: Change) {
        self.changes.push(change);
    }

    pub(crate) fn job(&mut self, account: AccountId, kind: JobKind) {
        self.jobs.push((account, kind));
    }

    /// Adds `more` after these.
    pub(crate) fn extend(&mut self, more: Self) {
        self.changes.extend(more.changes);
        self.jobs.extend(more.jobs);
    }
}

/// The UIDVALIDITY a job names the messages of `folder` by, or `None` when no job goes out: in the sample mailbox,
/// or before the folder's first sync.
pub(crate) fn writable(state: &State, folder: FolderId) -> Option<UidValidity> {
    if state.sample { None } else { state.validity.get(&folder).copied() }
}

/// The folder `id` of `account`.
pub(crate) fn folder(store: &dyn Storage, account: AccountId, id: FolderId) -> Result<Option<Folder>, EngineError> {
    Ok(store.folders(account)?.into_iter().find(|folder| folder.id() == id))
}

/// The server path of the folder `id` of `account`.
pub(crate) fn path(store: &dyn Storage, account: AccountId, id: FolderId) -> Result<FolderPath, EngineError> {
    folder(store, account, id)?.map(|folder| folder.listing().path().clone()).ok_or(EngineError::NotFound { what: "folder", id: id.get() })
}

/// The highest UID not `taken`: ours for a message whose server UID is not known yet. Servers count up from 1, so
/// from the top down they do not meet.
pub(crate) fn free_uid(taken: &BTreeSet<Uid>) -> Option<Uid> {
    let mut candidate = u32::MAX;
    for held in taken.iter().rev() {
        if held.get() != candidate {
            break;
        }
        candidate = candidate.saturating_sub(1);
    }
    Uid::new(candidate)
}

/// The message as it now is in `folder` under `uid`, with `moving` noting where the server keeps it.
pub(crate) fn placed(message: &Message, folder: FolderId, uid: Uid, moving: Option<Moving>) -> Message {
    Message::new(message.id(), message.account(), folder, message.summary().clone().with_uid(uid)).with_thread(message.thread()).with_moving(moving)
}

/// `message` with `flags`.
pub(crate) fn flagged(message: &Message, flags: Flags) -> Message {
    Message::new(message.id(), message.account(), message.folder(), message.summary().clone().with_flags(flags))
        .with_thread(message.thread())
        .with_moving(message.moving().cloned())
}

/// Sets or clears the read mark on `message`: the changes into `effects`, the message as it now is back.
pub(crate) fn mark_seen(store: &dyn Storage, state: &State, message: Message, seen: bool, effects: &mut Effects) -> Result<Message, EngineError> {
    match mark(store, state, &message, &Flag::Seen, seen)? {
        Some(marked) => {
            let mut flags = message.summary().flags().clone();
            if seen {
                flags.insert(Flag::Seen);
            } else {
                flags.remove(&Flag::Seen);
            }
            effects.extend(marked);
            Ok(flagged(&message, flags))
        }
        None => Ok(message),
    }
}

/// Sets (`set`) or clears `flag` on `message`; `None` when it already is so.
pub(crate) fn mark(store: &dyn Storage, state: &State, message: &Message, flag: &Flag, set: bool) -> Result<Option<Effects>, EngineError> {
    let mut flags = message.summary().flags().clone();
    let changed = if set { flags.insert(flag.clone()) } else { flags.remove(flag) };
    if !changed {
        return Ok(None);
    }
    let mut effects = Effects::default();
    effects.change(Change::SetFlags(message.id(), flags));
    let one: Flags = std::iter::once(flag.clone()).collect();
    let (add, remove) = if set { (one, Flags::default()) } else { (Flags::default(), one) };
    if message.moving().is_none()
        && let Some(uid_validity) = writable(state, message.folder())
    {
        let path = path(store, message.account(), message.folder())?;
        let uids: UidSet = std::iter::once(message.summary().uid()).collect();
        effects.job(message.account(), JobKind::StoreFlags { folder: message.folder(), path, uid_validity, uids, add, remove });
    }
    Ok(Some(effects))
}

/// The UIDs the messages of `folder` in the store hold.
pub(crate) fn taken(store: &dyn Storage, folder: FolderId) -> Result<BTreeSet<Uid>, EngineError> {
    Ok(store.messages(folder)?.iter().map(|entry| entry.message().summary().uid()).collect())
}

/// Moves `message` to `to`, a folder of its account that holds mail; `None` when it is there already.
pub(crate) fn relocate(store: &dyn Storage, state: &State, message: &Message, to: &Folder) -> Result<Option<Effects>, EngineError> {
    relocate_beside(store, state, message, to, &mut taken(store, to.id())?)
}

/// Moves several messages in one go: each takes a UID of ours in its folder that no earlier one took.
#[derive(Debug, Default)]
pub(crate) struct Mover {
    taken: BTreeMap<FolderId, BTreeSet<Uid>>,
}

impl Mover {
    /// As [`relocate`].
    pub(crate) fn relocate(&mut self, store: &dyn Storage, state: &State, message: &Message, to: &Folder) -> Result<Option<Effects>, EngineError> {
        let taken = match self.taken.entry(to.id()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(taken(store, to.id())?),
        };
        relocate_beside(store, state, message, to, taken)
    }
}

/// As [`relocate`], beside the UIDs `taken` in `to`; the one it takes is added.
pub(crate) fn relocate_beside(
    store: &dyn Storage,
    state: &State,
    message: &Message,
    to: &Folder,
    taken: &mut BTreeSet<Uid>,
) -> Result<Option<Effects>, EngineError> {
    let from = message.folder();
    if from == to.id() {
        return Ok(None);
    }
    let uid = free_uid(taken).ok_or(EngineError::NotFound { what: "free UID in folder", id: to.id().get() })?;
    taken.insert(uid);
    let mut effects = Effects::default();
    let moving = match message.moving() {
        Some(moving) => Some(moving.clone()),
        None => match writable(state, from) {
            Some(uid_validity) => {
                let held = message.summary().uid();
                let path = path(store, message.account(), from)?;
                let uids: UidSet = std::iter::once(held).collect();
                let to_path = to.listing().path().clone();
                effects.job(message.account(), JobKind::Move { folder: from, path, uid_validity, uids, to: to.id(), to_path });
                Some(Moving { folder: from, uid_validity, uid: held, flags: message.summary().flags().clone() })
            }
            None => None,
        },
    };
    effects.change(Change::PutMessage(placed(message, to.id(), uid, moving)));
    Ok(Some(effects))
}

/// Replaces `message` by the new copy `raw` in `to`, a folder of its account that holds mail (it may be the folder
/// the message is in): the store has it in `to` at once under a UID of ours, with [`Moving`] noting where the server
/// keeps the old copy, and the server gets the new one; the answer ([`crate::write`]) takes the old copy away. `None`
/// when no copy can go out: in the sample mailbox, before the folder's first sync, or while a move is on its way.
pub(crate) fn rewrite(store: &dyn Storage, state: &State, message: &Message, to: &Folder, raw: Vec<u8>) -> Result<Option<Effects>, EngineError> {
    if message.moving().is_some() {
        return Ok(None);
    }
    let Some(uid_validity) = writable(state, message.folder()) else { return Ok(None) };
    let uid = free_uid(&taken(store, to.id())?).ok_or(EngineError::NotFound { what: "free UID in folder", id: to.id().get() })?;
    let flags = message.summary().flags().clone();
    let mut effects = Effects::default();
    let path = to.listing().path().clone();
    effects.job(message.account(), JobKind::Append { folder: to.id(), path, flags: flags.clone(), raw, draft: None, message: Some(message.id()) });
    let place = Moving { folder: message.folder(), uid_validity, uid: message.summary().uid(), flags };
    effects.change(Change::PutMessage(placed(message, to.id(), uid, Some(place))));
    Ok(Some(effects))
}

/// Removes `message` for good.
pub(crate) fn expunge(store: &dyn Storage, state: &State, message: &Message) -> Result<Effects, EngineError> {
    if message.moving().is_some() {
        return Err(EngineError::StillMoving { id: message.id().get() });
    }
    let mut effects = Effects::default();
    effects.change(Change::RemoveMessage(message.id()));
    if let Some(uid_validity) = writable(state, message.folder()) {
        let path = path(store, message.account(), message.folder())?;
        let uids: UidSet = std::iter::once(message.summary().uid()).collect();
        effects.job(message.account(), JobKind::Expunge { folder: message.folder(), path, uid_validity, uids });
    }
    Ok(effects)
}

#[cfg(test)]
#[path = "act_tests.rs"]
mod tests;
