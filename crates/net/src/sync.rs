// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The read path's jobs on a signed-in connection: the folder list, the first sync of a folder (headers with
//! PREVIEW, then the bodies of the last days) and the body of one message.
//!
//! Folders are opened with EXAMINE: the read path changes nothing on the server. Every answer streams out as
//! events while it comes in, so a large folder never sits in memory whole.

use std::time::Duration;

use inboxcraft_model::{
    ErrorKind, Event, EventKind, Flag, Flags, FolderId, FolderListing, FolderPath, FolderState, Job, JobKind, Summary, Uid, UidSet, UidValidity,
};
use inboxcraft_proto::imap::{
    Answer, Client, Event as ImapEvent, FetchAttributes, FetchItem, MAX_LITERAL, MailboxStatus, PreviewFetch, RefusalKind, Section, SelectOptions,
    encode_utf7,
};
use inboxcraft_time::Timestamp;

use crate::envelope::envelope;
use crate::{ImapConnection, NetError};

/// How a first sync runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncConfig {
    /// Bodies of messages that arrived in the last this many days are fetched with the first sync, per folder (the
    /// store keeps them, with those of opened messages).
    pub body_days: u16,
    /// Messages per [`EventKind::MessagesAdded`].
    pub batch: usize,
    /// Bodies asked for in one command.
    pub body_batch: usize,
    /// Most bodies fetched in one folder sync; the others are fetched when they are opened.
    pub max_bodies: usize,
}

impl Default for SyncConfig {
    /// 30 days, 200 messages per event, 10 bodies per command, at most 10 000 bodies per folder sync.
    fn default() -> Self {
        Self { body_days: 30, batch: 200, body_batch: 10, max_bodies: 10_000 }
    }
}

impl SyncConfig {
    /// The oldest arrival time whose body the first sync fetches, at `now`.
    #[must_use]
    pub fn cutoff(&self, now: Timestamp) -> Timestamp {
        let days = Duration::from_secs(u64::from(self.body_days).saturating_mul(86_400));
        now.checked_sub(days).unwrap_or(Timestamp::MIN)
    }
}

/// Runs `job` on `connection` at `now`, handing every event it causes to `emit`.
///
/// # Errors
///
/// A [`NetError`]: the folder or message is gone, the server refused a command, or the connection failed.
pub(crate) fn run(
    connection: &mut ImapConnection,
    job: &Job,
    config: &SyncConfig,
    now: Timestamp,
    emit: &mut dyn FnMut(Event),
) -> Result<(), NetError> {
    let mut say = |kind: EventKind| emit(Event::new(job.account, Some(job.id), kind));
    match &job.kind {
        JobKind::SyncFolderList => {
            let listings = folder_list(connection)?;
            say(EventKind::Folders { listings });
            Ok(())
        }
        JobKind::SyncFolder { folder, path, known } => sync_folder(connection, *folder, path, *known, config, now, &mut say),
        JobKind::FetchBody { folder, path, uid_validity, uid } => {
            let raw = fetch_body(connection, path, *uid_validity, *uid)?;
            say(EventKind::BodyFetched { folder: *folder, uid_validity: *uid_validity, uid: *uid, raw });
            Ok(())
        }
        // Watching a folder comes with E1.7b; until then the worker waits for the next job.
        JobKind::Idle { .. } => Ok(()),
    }
}

/// LIST: every folder whose name the model takes.
fn folder_list(connection: &mut ImapConnection) -> Result<Vec<FolderListing>, NetError> {
    match connection.run(Client::list, &mut |_| {})? {
        Ok(Answer::Listed(entries)) => Ok(entries.iter().filter_map(|entry| entry.listing().ok()).collect()),
        Ok(_) => Err(other_answer()),
        Err(refusal) => Err(NetError::new(ErrorKind::CommandRefused, refusal.code.unwrap_or_default())),
    }
}

/// EXAMINE: the folder's status, with its UIDVALIDITY.
fn examine(connection: &mut ImapConnection, path: &FolderPath) -> Result<(MailboxStatus, UidValidity), NetError> {
    let mailbox = if path.is_inbox() { "INBOX".to_owned() } else { encode_utf7(path.name()) };
    let condstore = connection.capabilities_known().is_some_and(|caps| caps.has("CONDSTORE") || caps.has("QRESYNC"));
    let options = SelectOptions { condstore, ..SelectOptions::default() };
    let status = match connection.run(|client| client.examine_with(&mailbox, &options), &mut |_| {})? {
        Ok(Answer::Selected(status)) => status,
        Ok(_) => return Err(other_answer()),
        Err(refusal) if refusal.kind == RefusalKind::No => return Err(NetError::new(ErrorKind::FolderGone, refusal.code.unwrap_or_default())),
        Err(refusal) => return Err(NetError::new(ErrorKind::CommandRefused, refusal.code.unwrap_or_default())),
    };
    let uid_validity = status.uid_validity.ok_or_else(|| NetError::new(ErrorKind::Protocol, "the folder has no UIDVALIDITY"))?;
    Ok((status, uid_validity))
}

/// A folder sync: what is new since `known` (everything without it, or when the UIDVALIDITY changed), as summaries,
/// then the bodies of the messages that arrived since the cutoff, then the state for the next sync.
fn sync_folder(
    connection: &mut ImapConnection,
    folder: FolderId,
    path: &FolderPath,
    known: Option<FolderState>,
    config: &SyncConfig,
    now: Timestamp,
    say: &mut dyn FnMut(EventKind),
) -> Result<(), NetError> {
    let (status, uid_validity) = examine(connection, path)?;
    let start = match known {
        Some(state) if state.uid_validity() == uid_validity => state.uid_next(),
        Some(state) => {
            say(EventKind::UidValidityReset { folder, old: Some(state.uid_validity()), new: uid_validity });
            None
        }
        None => None,
    };
    let (first, last) = Uid::new(1).zip(Uid::new(u32::MAX)).ok_or_else(|| NetError::new(ErrorKind::Protocol, "no UID range"))?;
    let start = start.unwrap_or(first);
    let mut highest = None::<Uid>;
    let mut bodies = Vec::new();
    if status.exists > 0 {
        let cutoff = config.cutoff(now);
        let mut batch = Vec::new();
        let preview = connection.capabilities_known().is_some_and(|caps| caps.has("PREVIEW")).then_some(PreviewFetch::Generate);
        let modseq = connection.capabilities_known().is_some_and(|caps| caps.has("CONDSTORE") || caps.has("QRESYNC"));
        // `start:4294967295`: the server answers for the messages it has.
        let attributes = FetchAttributes {
            flags: true,
            internal_date: true,
            size: true,
            modseq,
            preview,
            section: Some(Section::Header),
            ..FetchAttributes::default()
        };
        let set: UidSet = std::iter::once((start, last)).collect();
        let outcome = connection.run(|client| client.uid_fetch(&set, attributes), &mut |event| {
            let Some(summary) = header_summary(event, start) else { return };
            highest = highest.max(Some(summary.uid()));
            let wanted = summary.received().is_some_and(|received| received >= cutoff);
            if wanted && usize::try_from(summary.size()).is_ok_and(|size| size <= MAX_LITERAL) && bodies.len() < config.max_bodies {
                bodies.push(summary.uid());
            }
            batch.push(summary);
            if batch.len() >= config.batch.max(1) {
                say(EventKind::MessagesAdded { folder, uid_validity, messages: std::mem::take(&mut batch) });
            }
        })?;
        refused_fetch(outcome)?;
        if !batch.is_empty() {
            say(EventKind::MessagesAdded { folder, uid_validity, messages: batch });
        }
    }
    for chunk in bodies.chunks(config.body_batch.max(1)) {
        let set: UidSet = chunk.iter().map(|uid| (*uid, *uid)).collect();
        let outcome = connection.run(
            |client| client.uid_fetch(&set, FetchAttributes { section: Some(Section::Full), ..FetchAttributes::default() }),
            &mut |event| {
                if let Some((uid, raw)) = full_body(event).filter(|(uid, _)| set.contains(*uid)) {
                    say(EventKind::BodyFetched { folder, uid_validity, uid, raw });
                }
            },
        )?;
        refused_fetch(outcome)?;
    }
    let uid_next = status.uid_next.or_else(|| highest.and_then(|uid| Uid::new(uid.get().saturating_add(1)))).or(known.and_then(|k| k.uid_next()));
    let state = FolderState::new(uid_validity).with_uid_next(uid_next).with_highest_modseq(status.highest_modseq);
    say(EventKind::FolderSynced { folder, state });
    Ok(())
}

/// The body of one message, when its UIDVALIDITY still holds.
fn fetch_body(connection: &mut ImapConnection, path: &FolderPath, uid_validity: UidValidity, uid: Uid) -> Result<Vec<u8>, NetError> {
    let (_status, current) = examine(connection, path)?;
    if current != uid_validity {
        return Err(NetError::new(ErrorKind::MessageGone, "the folder's UIDVALIDITY changed"));
    }
    let set: UidSet = std::iter::once((uid, uid)).collect();
    let mut found = None;
    let outcome = connection.run(
        |client| client.uid_fetch(&set, FetchAttributes { section: Some(Section::Full), ..FetchAttributes::default() }),
        &mut |event| {
            if let Some((_, raw)) = full_body(event).filter(|(got, _)| *got == uid) {
                found = Some(raw);
            }
        },
    )?;
    refused_fetch(outcome)?;
    found.ok_or_else(|| NetError::new(ErrorKind::MessageGone, "the server has no message with this UID"))
}

/// The summary in a FETCH response of the header fetch; `None` for anything else (a flag change the server tells
/// on its own, a UID below the one asked from).
fn header_summary(event: ImapEvent, start: Uid) -> Option<Summary> {
    let ImapEvent::Fetched(item) = event else { return None };
    let FetchItem { uid, flags, internal_date, size, modseq, preview, section, .. } = item;
    let uid = uid.filter(|uid| *uid >= start)?;
    let (Section::Header, header) = section? else { return None };
    let flags: Flags = flags.unwrap_or_default().iter().filter_map(|flag| Flag::parse(flag).ok()).collect();
    Some(
        Summary::new(uid, envelope(&header))
            .with_flags(flags)
            .with_size(size.unwrap_or(0))
            .with_received(internal_date)
            .with_preview(preview.as_deref())
            .with_modseq(modseq),
    )
}

/// The whole message in a FETCH response, with its UID.
fn full_body(event: ImapEvent) -> Option<(Uid, Vec<u8>)> {
    let ImapEvent::Fetched(item) = event else { return None };
    let uid = item.uid?;
    let (Section::Full, raw) = item.section? else { return None };
    Some((uid, raw))
}

fn refused_fetch(outcome: crate::imap::Outcome) -> Result<(), NetError> {
    match outcome {
        Ok(_) => Ok(()),
        Err(refusal) => Err(NetError::new(ErrorKind::CommandRefused, refusal.code.unwrap_or_default())),
    }
}

fn other_answer() -> NetError {
    NetError::new(ErrorKind::Protocol, "an answer of another command")
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
