// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The answers to the write jobs, and their failures, so the store ends as the server has it.
//!
//! - A moved message takes the UID the server gave it in the folder it went to (`COPYUID`). If the owner acted on it
//!   meanwhile, the jobs follow now that its place is known: the flags it got, then a move on to the folder it is in
//!   by now. A server that names no UID gets the message from the next sync of that folder, under a new id.
//! - A failed write is taken back: flags as they were, a moved message back where the server keeps it, and the folders
//!   synced again; a failed delete for good syncs the whole folder again, so a message still there comes back.

use inboxcraft_model::{AccountId, Copied, Flags, Folder, FolderId, JobId, JobKind, Message, Moving, UidSet, UidValidity};
use inboxcraft_store::{Change, Storage};

use crate::EngineError;
use crate::act;
use crate::sync::{Plan, current, held};

/// A `Moved` answer.
pub(crate) struct Moved<'a> {
    pub(crate) folder: FolderId,
    pub(crate) uid_validity: UidValidity,
    pub(crate) uids: &'a UidSet,
    pub(crate) to: FolderId,
    pub(crate) copied: Option<&'a Copied>,
}

/// The messages of `account` whose move is on its way, from a place `from` takes, each with that place.
pub(crate) fn waiting(store: &dyn Storage, account: AccountId, from: impl Fn(&Moving) -> bool) -> Result<Vec<(Message, Moving)>, EngineError> {
    let mut found = Vec::new();
    for folder in store.folders(account)? {
        for entry in store.messages(folder.id())? {
            if let Some(place) = entry.message().moving().filter(|place| from(place)) {
                found.push((entry.message().clone(), place.clone()));
            }
        }
    }
    Ok(found)
}

/// Whether `place` is one of `uids` of `folder` at `uid_validity`.
fn among(place: &Moving, folder: FolderId, uid_validity: UidValidity, uids: &UidSet) -> bool {
    place.folder == folder && place.uid_validity == uid_validity && uids.contains(place.uid)
}

/// The answer to a move: each message of it takes its new place.
pub(crate) fn moved(store: &dyn Storage, plan: &mut Plan, account: AccountId, moved: &Moved<'_>) -> Result<bool, EngineError> {
    if !current(&mut plan.state, moved.folder, moved.uid_validity) {
        return Ok(false);
    }
    let to = act::folder(store, account, moved.to)?;
    let copied = moved.copied.filter(|copied| to.is_some() && current(&mut plan.state, moved.to, copied.uid_validity));
    let mut lost = false;
    for (record, place) in waiting(store, account, |place| among(place, moved.folder, moved.uid_validity, moved.uids))? {
        match copied.and_then(|copied| copied.new_uid(place.uid).map(|uid| (copied.uid_validity, uid))) {
            Some((uid_validity, uid)) => settle(store, plan, &record, Moving { folder: moved.to, uid_validity, uid, flags: place.flags })?,
            None => {
                plan.change(Change::RemoveMessage(record.id()));
                lost = true;
            }
        }
    }
    if lost && let Some(to) = to {
        resync(store, plan, account, &to, false)?;
    }
    Ok(true)
}

/// `record` now that the server keeps it at `place`: there it takes the server's UID, or a move takes it on to the
/// folder it is in; and the server gets the flags it has.
fn settle(store: &dyn Storage, plan: &mut Plan, record: &Message, place: Moving) -> Result<(), EngineError> {
    let account = record.account();
    let path = act::path(store, account, place.folder)?;
    let flags = record.summary().flags();
    if *flags != place.flags {
        let add: Flags = flags.iter().filter(|flag| !place.flags.contains(flag)).collect();
        let remove: Flags = place.flags.iter().filter(|flag| !flags.contains(flag)).collect();
        let uids: UidSet = std::iter::once(place.uid).collect();
        plan.kept_job(account, JobKind::StoreFlags { folder: place.folder, path: path.clone(), uid_validity: place.uid_validity, uids, add, remove });
    }
    if record.folder() == place.folder {
        // A copy the folder's sync brought before this answer is the same message.
        if let Some(other) = held(store, place.folder)?.get(&place.uid) {
            plan.change(Change::RemoveMessage(other.id()));
        }
        plan.change(Change::PutMessage(act::placed(record, place.folder, place.uid, None)));
    } else {
        let to_path = act::path(store, account, record.folder())?;
        let uids: UidSet = std::iter::once(place.uid).collect();
        plan.kept_job(account, JobKind::Move { folder: place.folder, path, uid_validity: place.uid_validity, uids, to: record.folder(), to_path });
        plan.change(Change::PutMessage(record.clone().with_moving(Some(place))));
    }
    Ok(())
}

/// A failed kept `job` of `account`: its write is taken back.
pub(crate) fn failed(store: &dyn Storage, plan: &mut Plan, account: AccountId, job: Option<JobId>) -> Result<(), EngineError> {
    let kind = match job {
        Some(job) if plan.state.kept.contains(&job) => store.queued_jobs()?.into_iter().find(|queued| queued.id == job).map(|queued| queued.kind),
        _ => None,
    };
    match kind {
        Some(JobKind::StoreFlags { folder, uid_validity, uids, add, remove, .. }) if plan.state.validity.get(&folder) == Some(&uid_validity) => {
            for message in held(store, folder)?.values().filter(|message| uids.contains(message.summary().uid())) {
                let mut flags = message.summary().flags().clone();
                for flag in add.iter() {
                    flags.remove(&flag);
                }
                flags.extend(remove.iter());
                if flags != *message.summary().flags() {
                    plan.change(Change::SetFlags(message.id(), flags));
                }
            }
        }
        Some(JobKind::Move { folder, uid_validity, uids, to, .. }) => {
            for (record, place) in waiting(store, account, |place| among(place, folder, uid_validity, &uids))? {
                if plan.state.validity.get(&place.folder) == Some(&place.uid_validity) {
                    let back = act::placed(&record, place.folder, record.summary().uid(), Some(place.clone()));
                    settle(store, plan, &back, place)?;
                } else {
                    plan.change(Change::RemoveMessage(record.id()));
                }
            }
            for id in [folder, to] {
                if let Some(folder) = act::folder(store, account, id)? {
                    resync(store, plan, account, &folder, false)?;
                }
            }
        }
        Some(JobKind::Expunge { folder, .. }) => {
            if let Some(folder) = act::folder(store, account, folder)? {
                resync(store, plan, account, &folder, true)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Every message whose move is on its way from `folder`, of an older UIDVALIDITY now: its place is void.
pub(crate) fn void(store: &dyn Storage, plan: &mut Plan, account: AccountId, folder: FolderId) -> Result<(), EngineError> {
    for (record, _) in waiting(store, account, |place| place.folder == folder)? {
        plan.change(Change::RemoveMessage(record.id()));
    }
    Ok(())
}

/// Syncs `folder` again: from where the last sync ended, or in `full`.
fn resync(store: &dyn Storage, plan: &mut Plan, account: AccountId, folder: &Folder, full: bool) -> Result<(), EngineError> {
    let known = if full { None } else { store.folder_state(folder.id())? };
    plan.job(account, JobKind::SyncFolder { folder: folder.id(), path: folder.listing().path().clone(), known });
    Ok(())
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
