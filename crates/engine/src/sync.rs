// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Events in: what a `net` worker reports, turned into store changes, session state and the next jobs.
//!
//! Like a command, planning an event only reads; the session makes the plan afterwards. An event about an account
//! or a folder the session does not hold, or with UIDs of an older UIDVALIDITY, is dropped.

use std::collections::BTreeMap;

use inboxcraft_model::{
    AccountId, Event, EventKind, FlagChange, Folder, FolderId, FolderListing, FolderPath, JobKind, Message, Role, Summary, Uid, UidSet, UidValidity,
};
use inboxcraft_store::{Batch, Body, Change, Storage};

use crate::session::{Pending, State};
use crate::{Context, EngineError};

/// What an event changes: the session's new state, the store changes and the jobs that follow.
pub(crate) struct Plan {
    pub(crate) state: State,
    pub(crate) batch: Batch,
    pub(crate) jobs: Vec<Pending>,
}

impl Plan {
    fn change(&mut self, change: Change) {
        self.batch.push(change);
    }

    fn job(&mut self, account: AccountId, kind: JobKind) {
        self.jobs.push(Pending { account, kind });
    }
}

/// The plan for `event`, or `None` when it is dropped.
pub(crate) fn plan(context: &Context<'_>, event: &Event) -> Result<Option<Plan>, EngineError> {
    let account = event.account;
    if !context.state().accounts.contains_key(&account) {
        return Ok(None);
    }
    let store = context.store();
    if let Some(folder) = event.kind.folder()
        && !store.folders(account)?.iter().any(|held| held.id() == folder)
    {
        return Ok(None);
    }
    let mut plan = Plan { state: context.state().clone(), batch: Batch::new(), jobs: Vec::new() };
    let kept = match &event.kind {
        EventKind::Folders { listings } => folders(store, &mut plan, account, listings)?,
        EventKind::MessagesAdded { folder, uid_validity, messages } => added(store, &mut plan, account, *folder, *uid_validity, messages)?,
        EventKind::MessagesChanged { folder, uid_validity, changes } => changed(store, &mut plan, *folder, *uid_validity, changes)?,
        EventKind::MessagesExpunged { folder, uid_validity, uids } => expunged(store, &mut plan, *folder, *uid_validity, uids)?,
        EventKind::UidValidityReset { folder, new, .. } => reset(store, &mut plan, *folder, *new)?,
        EventKind::BodyFetched { folder, uid_validity, uid, raw } => fetched(store, &mut plan, *folder, *uid_validity, *uid, raw)?,
        EventKind::FolderSynced { folder, state } => {
            let kept = current(&mut plan.state, *folder, state.uid_validity());
            if kept {
                plan.change(Change::SetFolderState(*folder, *state));
            }
            kept
        }
        EventKind::Connection(connection) => {
            let held = plan.state.accounts.entry(account).or_default();
            held.connection = Some(*connection);
            if matches!(connection, inboxcraft_model::ConnectionState::Online | inboxcraft_model::ConnectionState::Watching) {
                held.failure = None;
            }
            true
        }
        EventKind::Failed { kind, folder } => {
            plan.state.accounts.entry(account).or_default().failure = Some((*kind, *folder));
            true
        }
    };
    Ok(kept.then_some(plan))
}

/// Whether UIDs of `uid_validity` belong to what we hold of `folder`; the first UIDVALIDITY seen is taken.
fn current(state: &mut State, folder: FolderId, uid_validity: UidValidity) -> bool {
    *state.validity.entry(folder).or_insert(uid_validity) == uid_validity
}

fn same_path(a: &FolderPath, b: &FolderPath) -> bool {
    a.name() == b.name() || (a.is_inbox() && b.is_inbox())
}

/// The account's whole folder list: held folders keep their ids, new ones get one, missing ones go. Then every
/// folder that holds mail is synced, the inbox first, and the inbox is watched.
fn folders(store: &dyn Storage, plan: &mut Plan, account: AccountId, listings: &[FolderListing]) -> Result<bool, EngineError> {
    let held = store.folders(account)?;
    let mut next: Vec<Folder> = Vec::new();
    for listing in listings {
        if next.iter().any(|folder| same_path(folder.listing().path(), listing.path())) {
            continue;
        }
        let known = held.iter().find(|folder| same_path(folder.listing().path(), listing.path()));
        let id = known.map_or_else(|| plan.state.folder_id(), Folder::id);
        let folder = Folder::new(id, account, listing.clone());
        if known != Some(&folder) {
            plan.change(Change::PutFolder(folder.clone()));
        }
        next.push(folder);
    }
    for gone in held.iter().filter(|folder| next.iter().all(|kept| kept.id() != folder.id())) {
        plan.change(Change::RemoveFolder(gone.id()));
        plan.state.validity.remove(&gone.id());
    }
    let held_state = plan.state.accounts.entry(account).or_default();
    let questions = held_state.roles.detect(&next);
    held_state.ask.extend(questions);
    held_state.ask.retain(|role| held_state.roles.folder(*role).is_none());
    let inbox = held_state.roles.folder(Role::Inbox);
    let mut order: Vec<&Folder> = next.iter().filter(|folder| folder.listing().is_selectable()).collect();
    order.sort_by_key(|folder| Some(folder.id()) != inbox);
    for folder in order {
        let known = if held.iter().any(|old| old.id() == folder.id()) { store.folder_state(folder.id())? } else { None };
        plan.job(account, JobKind::SyncFolder { folder: folder.id(), path: folder.listing().path().clone(), known });
    }
    if let Some(inbox) = next.iter().find(|folder| Some(folder.id()) == inbox) {
        plan.job(account, JobKind::Idle { folder: inbox.id(), path: inbox.listing().path().clone() });
    }
    Ok(true)
}

/// The messages of `folder` we hold, by UID.
fn held(store: &dyn Storage, folder: FolderId) -> Result<BTreeMap<Uid, Message>, EngineError> {
    Ok(store.messages(folder)?.into_iter().map(|entry| (entry.message().summary().uid(), entry.message().clone())).collect())
}

fn added(
    store: &dyn Storage,
    plan: &mut Plan,
    account: AccountId,
    folder: FolderId,
    uid_validity: UidValidity,
    messages: &[Summary],
) -> Result<bool, EngineError> {
    if !current(&mut plan.state, folder, uid_validity) {
        return Ok(false);
    }
    let held = held(store, folder)?;
    let mut given: BTreeMap<Uid, inboxcraft_model::MessageId> = BTreeMap::new();
    for summary in messages {
        let uid = summary.uid();
        let old = held.get(&uid);
        let id = match (old, given.get(&uid)) {
            (_, Some(id)) => *id,
            (Some(old), None) => old.id(),
            (None, None) => plan.state.message_id(),
        };
        given.insert(uid, id);
        let message = Message::new(id, account, folder, summary.clone()).with_thread(old.and_then(Message::thread));
        plan.change(Change::PutMessage(message));
    }
    Ok(true)
}

fn changed(store: &dyn Storage, plan: &mut Plan, folder: FolderId, uid_validity: UidValidity, changes: &[FlagChange]) -> Result<bool, EngineError> {
    if !current(&mut plan.state, folder, uid_validity) {
        return Ok(false);
    }
    let held = held(store, folder)?;
    for change in changes {
        if let Some(message) = held.get(&change.uid) {
            plan.change(Change::SetFlags(message.id(), change.flags.clone()));
        }
    }
    Ok(true)
}

fn expunged(store: &dyn Storage, plan: &mut Plan, folder: FolderId, uid_validity: UidValidity, uids: &UidSet) -> Result<bool, EngineError> {
    if !current(&mut plan.state, folder, uid_validity) {
        return Ok(false);
    }
    for (uid, message) in held(store, folder)? {
        if uids.contains(uid) {
            plan.change(Change::RemoveMessage(message.id()));
        }
    }
    Ok(true)
}

/// Every UID we hold of `folder` is void: its messages go, and the next ones belong to `new`.
fn reset(store: &dyn Storage, plan: &mut Plan, folder: FolderId, new: UidValidity) -> Result<bool, EngineError> {
    for message in held(store, folder)?.values() {
        plan.change(Change::RemoveMessage(message.id()));
    }
    plan.state.validity.insert(folder, new);
    Ok(true)
}

/// A whole message: kept with its plain text for the search. A message we do not hold, or one too large to keep, is
/// dropped.
fn fetched(store: &dyn Storage, plan: &mut Plan, folder: FolderId, uid_validity: UidValidity, uid: Uid, raw: &[u8]) -> Result<bool, EngineError> {
    if !current(&mut plan.state, folder, uid_validity) {
        return Ok(false);
    }
    let Some(message) = held(store, folder)?.remove(&uid) else { return Ok(false) };
    let Ok(body) = Body::new(raw.to_vec(), &plain_text(raw)) else { return Ok(false) };
    plan.change(Change::PutBody(message.id(), body));
    Ok(true)
}

/// The text a search looks at: the message's plain text part, or nothing when it has none or cannot be read.
pub(crate) fn plain_text(raw: &[u8]) -> String {
    inboxcraft_mime::parse(raw).ok().and_then(|message| message.text_body().and_then(|part| part.text().ok())).unwrap_or_default()
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
