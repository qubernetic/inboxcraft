// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The write path's jobs on a signed-in connection: flags (UID STORE), a move (UID MOVE, or its fallback), a delete for
//! good (`\Deleted` and UID EXPUNGE), APPEND and CREATE.
//!
//! A folder is opened with SELECT and its UIDVALIDITY checked against the job's. When it changed, the job's UIDs name
//! other messages or none: nothing is written, the change is told ([`EventKind::UidValidityReset`]) and the job ends
//! with its usual last event, which the engine then drops as stale.
//!
//! A message gone from the server meanwhile is no error. After a store the flags are read back, and a UID that does
//! not come back is told as expunged; a move names no new UID for it; a delete for good of it is done. So a write the
//! worker runs again after a reconnect ends in the state one run leaves: flags set twice are set, a message moved
//! with UID MOVE already is not found again, and CREATE of a folder that exists counts as made. APPEND is the one
//! exception: one cut off before the server's answer may have added the message, and runs again (see
//! [`crate::Worker`]). A move on a server without MOVE is not run again ([`may_run_again`]): its fallback (UID COPY,
//! `\Deleted`, UID EXPUNGE) cut off after the copy leaves the messages in both folders, and a second run would copy
//! them once more.
//!
//! Nothing here sends CLOSE or EXPUNGE: both remove every message marked `\Deleted` in the folder, not only the job's.
//! A delete for good therefore needs UID EXPUNGE (UIDPLUS or IMAP4rev2); without it the job fails before anything is
//! sent.

use inboxcraft_model::{Copied, ErrorKind, EventKind, Flag, FolderId, FolderPath, Job, JobKind, Uid, UidSet, UidValidity};
use inboxcraft_proto::imap::{Answer, Event as ImapEvent, FetchAttributes, FlagStore, Moved, Refusal, Removal, StoreMode};

use crate::sync::{Present, SyncConfig, flag_change, mailbox, open, other_answer, refused_fetch};
use crate::{ImapConnection, NetError};

/// The most new UIDs of a move passed on; past it the next sync of the folder they went to brings them.
const MAX_COPIED: u64 = 1_000_000;

/// Runs the write `job` on `connection`, handing the events it causes to `say`.
///
/// # Errors
///
/// A [`NetError`]: the folder is gone, the server refused a command, or the connection failed.
pub(crate) fn run(connection: &mut ImapConnection, job: &Job, config: &SyncConfig, say: &mut dyn FnMut(EventKind)) -> Result<(), NetError> {
    match &job.kind {
        JobKind::StoreFlags { folder, path, uid_validity, uids, add, remove } => {
            if select(connection, *folder, path, *uid_validity, say)? {
                for (mode, flags) in [(StoreMode::Add, add), (StoreMode::Remove, remove)] {
                    if !flags.is_empty() {
                        store(connection, uids, &FlagStore { silent: true, ..FlagStore::new(mode, flags.clone()) })?;
                    }
                }
                read_back(connection, *folder, *uid_validity, uids, config, say)?;
            }
            say(EventKind::FlagsStored { folder: *folder, uid_validity: *uid_validity, uids: uids.clone() });
        }
        JobKind::Move { folder, path, uid_validity, uids, to, to_path } => {
            let copied = if select(connection, *folder, path, *uid_validity, say)? { move_to(connection, uids, to_path)? } else { None };
            say(EventKind::Moved { folder: *folder, uid_validity: *uid_validity, uids: uids.clone(), to: *to, copied });
        }
        JobKind::Expunge { folder, path, uid_validity, uids } => {
            if !connection.capabilities_known().is_some_and(|caps| caps.has("UIDPLUS") || caps.has("IMAP4rev2")) {
                return Err(NetError::new(ErrorKind::CommandRefused, "the server cannot remove only these messages (no UIDPLUS)"));
            }
            if select(connection, *folder, path, *uid_validity, say)? {
                let deleted = FlagStore { silent: true, ..FlagStore::new(StoreMode::Add, std::iter::once(Flag::Deleted).collect()) };
                store(connection, uids, &deleted)?;
                match connection.ask(|client| client.uid_expunge(uids), &mut |_| {})? {
                    Ok(Answer::Expunged) => {}
                    Ok(_) => return Err(other_answer()),
                    Err(refusal) => return Err(refused(&refusal)),
                }
            }
            say(EventKind::Expunged { folder: *folder, uid_validity: *uid_validity, uids: uids.clone() });
        }
        JobKind::Append { folder, path, flags, raw } => match connection.ask(|client| client.append(&mailbox(path), flags, raw), &mut |_| {})? {
            Ok(Answer::Appended(uid)) => say(EventKind::Appended { folder: *folder, uid: uid.map(|uid| (uid.uid_validity, uid.uid)) }),
            Ok(_) => return Err(other_answer()),
            Err(refusal) => return Err(refused(&refusal)),
        },
        JobKind::CreateFolder { path } => {
            match connection.ask(|client| client.create(&mailbox(path)), &mut |_| {})? {
                Ok(Answer::Created) => {}
                // Made already, by an earlier run of this job or by someone else.
                Err(refusal) if refusal.code.as_deref() == Some("ALREADYEXISTS") => {}
                Ok(_) => return Err(other_answer()),
                Err(refusal) => return Err(refused(&refusal)),
            }
            say(EventKind::FolderCreated { path: path.clone() });
        }
        _ => return Err(NetError::new(ErrorKind::Protocol, "not a write job")),
    }
    Ok(())
}

/// Whether `job`, cut off by its connection, may run again on `connection`'s server: not a move on a server without
/// MOVE, whose fallback may have copied the messages before the cut, so that a second run copies them again.
pub(crate) fn may_run_again(connection: &ImapConnection, job: &JobKind) -> bool {
    !matches!(job, JobKind::Move { .. }) || connection.capabilities_known().is_some_and(|caps| caps.has("MOVE") || caps.has("IMAP4rev2"))
}

/// SELECT of the folder, read-write; whether its UIDVALIDITY is still the job's (when not, the change is told).
fn select(
    connection: &mut ImapConnection,
    folder: FolderId,
    path: &FolderPath,
    uid_validity: UidValidity,
    say: &mut dyn FnMut(EventKind),
) -> Result<bool, NetError> {
    let (status, current) = open(connection, path, None, true, &mut |_| {})?;
    if status.read_only {
        return Err(NetError::new(ErrorKind::CommandRefused, "the server opened the folder read-only"));
    }
    if current != uid_validity {
        say(EventKind::UidValidityReset { folder, old: Some(uid_validity), new: current });
        return Ok(false);
    }
    Ok(true)
}

fn store(connection: &mut ImapConnection, uids: &UidSet, store: &FlagStore) -> Result<(), NetError> {
    match connection.ask(|client| client.uid_store(uids, store), &mut |_| {})? {
        Ok(Answer::Stored { .. }) => Ok(()),
        Ok(_) => Err(other_answer()),
        Err(refusal) => Err(refused(&refusal)),
    }
}

/// The flags of `uids` as the server has them now, told as changed; the UIDs that do not come back are told as
/// expunged.
fn read_back(
    connection: &mut ImapConnection,
    folder: FolderId,
    uid_validity: UidValidity,
    uids: &UidSet,
    config: &SyncConfig,
    say: &mut dyn FnMut(EventKind),
) -> Result<(), NetError> {
    let modseq = connection.capabilities_known().is_some_and(|caps| caps.has("CONDSTORE") || caps.has("QRESYNC"));
    let batch = config.batch.max(1);
    let mut present = Present::new(config.max_ranges);
    let mut changes = Vec::new();
    let attributes = FetchAttributes { flags: true, modseq, ..FetchAttributes::default() };
    let outcome = connection.ask(|client| client.uid_fetch(uids, attributes), &mut |event| {
        let ImapEvent::Fetched(item) = event else { return };
        let Some(uid) = item.uid.filter(|uid| uids.contains(*uid)) else { return };
        if !present.note(uid) {
            return;
        }
        if let Some(flags) = item.flags {
            changes.push(flag_change(uid, &flags, item.modseq));
            if changes.len() >= batch {
                say(EventKind::MessagesChanged { folder, uid_validity, changes: std::mem::take(&mut changes) });
            }
        }
    })?;
    refused_fetch(outcome)?;
    if present.overflowed {
        return Err(NetError::new(ErrorKind::TooLarge, "the server named more messages than this program keeps"));
    }
    if !changes.is_empty() {
        say(EventKind::MessagesChanged { folder, uid_validity, changes });
    }
    let gone = absent(uids, &UidSet::from(present.ranges));
    if !gone.is_empty() {
        say(EventKind::MessagesExpunged { folder, uid_validity, uids: gone });
    }
    Ok(())
}

/// UID MOVE of `uids` to `to_path`: the new UIDs where the server named them.
fn move_to(connection: &mut ImapConnection, uids: &UidSet, to_path: &FolderPath) -> Result<Option<Copied>, NetError> {
    match connection.ask(|client| client.uid_move(uids, &mailbox(to_path)), &mut |_| {})? {
        Ok(Answer::Moved(Moved { copy_uid, removal })) => match removal {
            Removal::Moved | Removal::Expunged | Removal::MarkedDeleted => Ok(copy_uid
                .filter(|copy_uid| copy_uid.len() <= MAX_COPIED)
                .map(|copy_uid| Copied { uid_validity: copy_uid.uid_validity(), uids: copy_uid.pairs().collect() })),
            // Copied, and still where they were: the engine takes the move back and syncs both folders.
            Removal::NotMarked(refusal) | Removal::NotExpunged(refusal) => {
                Err(NetError::new(ErrorKind::CommandRefused, refusal.code.unwrap_or_default()))
            }
        },
        Ok(_) => Err(other_answer()),
        Err(refusal) => Err(refused(&refusal)),
    }
}

/// A refused write: the folder named is gone (`TRYCREATE`, `NONEXISTENT`), or the server would not.
fn refused(refusal: &Refusal) -> NetError {
    match refusal.code.as_deref() {
        Some(code @ ("TRYCREATE" | "NONEXISTENT")) => NetError::new(ErrorKind::FolderGone, code),
        code => NetError::new(ErrorKind::CommandRefused, code.unwrap_or_default()),
    }
}

/// The UIDs of `asked` that are in none of the `present` ranges.
pub(crate) fn absent(asked: &UidSet, present: &UidSet) -> UidSet {
    let mut gaps = Vec::new();
    let mut found = present.ranges().iter().copied().peekable();
    for &(first, last) in asked.ranges() {
        let mut next = Some(first);
        while let Some(from) = next {
            while found.peek().is_some_and(|&(_, high)| high < from) {
                found.next();
            }
            match found.peek() {
                Some(&(low, high)) if low <= last => {
                    if let Some(end) = Uid::new(low.get().saturating_sub(1)).filter(|end| *end >= from) {
                        gaps.push((from, end));
                    }
                    next = high.get().checked_add(1).and_then(Uid::new).filter(|after| *after <= last);
                }
                _ => {
                    gaps.push((from, last));
                    next = None;
                }
            }
        }
    }
    UidSet::from(gaps)
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
