// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The read path's jobs on a signed-in connection: the folder list, the sync of a folder (headers with PREVIEW,
//! then the bodies of the last days) and the body of one message.
//!
//! Folders are opened with EXAMINE: the read path changes nothing on the server. Every answer streams out as
//! events while it comes in, so a large folder never sits in memory whole.
//!
//! A folder known from an earlier sync is resynced: what changed and what went among the messages it had, then what
//! is new. With QRESYNC on and a modification sequence known, EXAMINE itself brings both (RFC 7162 section 3.2.5):
//! the flags changed since, and the UIDs removed since as `VANISHED (EARLIER)`. Without, the flags of every known
//! message are fetched (only those changed since are reported where the server keeps modification sequences) and the
//! UIDs below the known UIDNEXT that did not come back are the ones removed.
//!
//! What a server tells is capped: QRESYNC's changes are kept once per message and at most [`SyncConfig::max_told`] of
//! them (past it the flags are fetched instead), and the known messages still there are kept as ranges, at most
//! [`SyncConfig::max_ranges`] of them (past it the sync fails with [`ErrorKind::TooLarge`]).

use std::collections::BTreeMap;
use std::time::Duration;

use inboxcraft_model::{
    ErrorKind, Event, EventKind, Flag, FlagChange, Flags, FolderId, FolderListing, FolderPath, FolderState, Job, JobKind, ModSeq, Summary, Uid,
    UidSet, UidValidity,
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
    /// Most flag changes and most removed-UID ranges kept from what QRESYNC tells while a known folder opens; past
    /// it they are dropped and the flags of the known messages are fetched instead.
    pub max_told: usize,
    /// Most ranges of known messages still on the server a resync without QRESYNC keeps; past it the sync fails with
    /// [`ErrorKind::TooLarge`].
    pub max_ranges: usize,
}

impl Default for SyncConfig {
    /// 30 days, 200 messages per event, 10 bodies per command, at most 10 000 bodies per folder sync, 10 000 changes
    /// told by QRESYNC and a million ranges of known messages.
    fn default() -> Self {
        Self { body_days: 30, batch: 200, body_batch: 10, max_bodies: 10_000, max_told: 10_000, max_ranges: 1_000_000 }
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
        // The worker watches the folder between jobs (see `watch`); there is nothing to run here.
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

/// EXAMINE: the folder's status, with its UIDVALIDITY. With `known`, QRESYNC asks for what changed since then; the
/// events it brings go to `on_event`.
fn examine(
    connection: &mut ImapConnection,
    path: &FolderPath,
    known: Option<FolderState>,
    on_event: &mut dyn FnMut(ImapEvent),
) -> Result<(MailboxStatus, UidValidity), NetError> {
    let mailbox = if path.is_inbox() { "INBOX".to_owned() } else { encode_utf7(path.name()) };
    let condstore = connection.capabilities_known().is_some_and(|caps| caps.has("CONDSTORE") || caps.has("QRESYNC"));
    let options = SelectOptions { condstore, known, ..SelectOptions::default() };
    let status = match connection.run(|client| client.examine_with(&mailbox, &options), on_event)? {
        Ok(Answer::Selected(status)) => status,
        Ok(_) => return Err(other_answer()),
        Err(refusal) if refusal.kind == RefusalKind::No => return Err(NetError::new(ErrorKind::FolderGone, refusal.code.unwrap_or_default())),
        Err(refusal) => return Err(NetError::new(ErrorKind::CommandRefused, refusal.code.unwrap_or_default())),
    };
    let uid_validity = status.uid_validity.ok_or_else(|| NetError::new(ErrorKind::Protocol, "the folder has no UIDVALIDITY"))?;
    Ok((status, uid_validity))
}

/// A folder sync: with `known` and the same UIDVALIDITY, what changed and went among the known messages; then what is
/// new since `known` (everything without it, or when the UIDVALIDITY changed), as summaries; then the bodies of the
/// messages that arrived since the cutoff; then the state for the next sync.
fn sync_folder(
    connection: &mut ImapConnection,
    folder: FolderId,
    path: &FolderPath,
    known: Option<FolderState>,
    config: &SyncConfig,
    now: Timestamp,
    say: &mut dyn FnMut(EventKind),
) -> Result<(), NetError> {
    // QRESYNC answers for a state with a modification sequence only (RFC 7162 section 3.2.5).
    let qresync = known.filter(|state| connection.qresync() && state.highest_modseq().is_some());
    let below = known.and_then(|state| state.uid_next());
    let mut told = Told::new(below, config.batch, config.max_told);
    let (status, uid_validity) = examine(connection, path, qresync, &mut |event| told.take(event))?;
    let start = match known {
        Some(state) if state.uid_validity() == uid_validity => state.uid_next(),
        Some(state) => {
            say(EventKind::UidValidityReset { folder, old: Some(state.uid_validity()), new: uid_validity });
            None
        }
        None => None,
    };
    if let Some(below) = start {
        if qresync.is_some() && !told.overflowed() {
            told.report(folder, uid_validity, say);
        } else {
            let since = known.and_then(|state| state.highest_modseq());
            resync_flags(connection, folder, uid_validity, below, since, status.exists, config, say)?;
        }
    }
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

/// Without QRESYNC: the flags of the messages below `below`, all of them or (with modification sequences) those
/// changed `since`, and the UIDs below `below` that did not come back as removed.
#[allow(clippy::too_many_arguments)]
fn resync_flags(
    connection: &mut ImapConnection,
    folder: FolderId,
    uid_validity: UidValidity,
    below: Uid,
    since: Option<ModSeq>,
    exists: u32,
    config: &SyncConfig,
    say: &mut dyn FnMut(EventKind),
) -> Result<(), NetError> {
    let (Some(first), Some(last)) = (Uid::new(1), below.get().checked_sub(1).and_then(Uid::new)) else { return Ok(()) };
    let batch = config.batch;
    let mut present = Present::new(config.max_ranges);
    if exists > 0 {
        let modseq = connection.capabilities_known().is_some_and(|caps| caps.has("CONDSTORE") || caps.has("QRESYNC"));
        let mut changes = Vec::new();
        let set: UidSet = std::iter::once((first, last)).collect();
        let outcome =
            connection.run(|client| client.uid_fetch(&set, FetchAttributes { flags: true, modseq, ..FetchAttributes::default() }), &mut |event| {
                let ImapEvent::Fetched(item) = event else { return };
                let Some(uid) = item.uid.filter(|uid| *uid <= last) else { return };
                if !present.note(uid) {
                    return;
                }
                let unchanged = since.zip(item.modseq).is_some_and(|(since, modseq)| modseq <= since);
                if let Some(change) = item.flags.filter(|_| !unchanged).map(|flags| flag_change(uid, &flags, item.modseq)) {
                    changes.push(change);
                    if changes.len() >= batch.max(1) {
                        say(EventKind::MessagesChanged { folder, uid_validity, changes: std::mem::take(&mut changes) });
                    }
                }
            })?;
        refused_fetch(outcome)?;
        if present.overflowed {
            return Err(NetError::new(ErrorKind::TooLarge, "the server named more known messages than this program keeps"));
        }
        if !changes.is_empty() {
            say(EventKind::MessagesChanged { folder, uid_validity, changes });
        }
    }
    let gone = missing(first, last, present.ranges);
    if !gone.is_empty() {
        say(EventKind::MessagesExpunged { folder, uid_validity, uids: gone });
    }
    Ok(())
}

/// The UIDs of known messages a flags fetch brought back, as ascending ranges: servers answer in UID order, so a
/// folder takes one range per gap however often a UID comes back. At most `cap` ranges.
struct Present {
    cap: usize,
    ranges: Vec<(Uid, Uid)>,
    overflowed: bool,
}

impl Present {
    fn new(cap: usize) -> Self {
        Self { cap, ranges: Vec::new(), overflowed: false }
    }

    /// Notes `uid`; `false` once there are more ranges than the cap, and from then on.
    fn note(&mut self, uid: Uid) -> bool {
        if self.overflowed {
            return false;
        }
        let full = self.ranges.len() >= self.cap;
        match self.ranges.last_mut() {
            Some((from, to)) if *from <= uid && uid <= *to => {}
            Some((_, to)) if to.get().checked_add(1) == Some(uid.get()) => *to = uid,
            _ if full => {
                self.overflowed = true;
                self.ranges = Vec::new();
                return false;
            }
            _ => self.ranges.push((uid, uid)),
        }
        true
    }
}

/// What QRESYNC told while the folder opened: the removed UIDs and the flags changed among the known messages, the
/// latest change per message. Past `cap` changes or ranges everything is dropped: the flags are fetched instead.
struct Told {
    below: Option<Uid>,
    batch: usize,
    cap: usize,
    vanished: Vec<(Uid, Uid)>,
    changes: BTreeMap<Uid, FlagChange>,
    overflowed: bool,
}

impl Told {
    fn new(below: Option<Uid>, batch: usize, cap: usize) -> Self {
        Self { below, batch: batch.max(1), cap, vanished: Vec::new(), changes: BTreeMap::new(), overflowed: false }
    }

    fn take(&mut self, event: ImapEvent) {
        if self.overflowed {
            return;
        }
        match event {
            ImapEvent::Vanished { uids, .. } => {
                if self.vanished.len().saturating_add(uids.ranges().len()) > self.cap {
                    self.overflow();
                } else {
                    self.vanished.extend_from_slice(uids.ranges());
                }
            }
            ImapEvent::Fetched(item) => {
                // A message at or above the known UIDNEXT is new: the header fetch brings it.
                if let (Some(uid), Some(flags), Some(below)) = (item.uid, item.flags, self.below)
                    && uid < below
                {
                    self.changes.insert(uid, flag_change(uid, &flags, item.modseq));
                    if self.changes.len() > self.cap {
                        self.overflow();
                    }
                }
            }
            _ => {}
        }
    }

    fn overflow(&mut self) {
        self.overflowed = true;
        self.vanished = Vec::new();
        self.changes = BTreeMap::new();
    }

    fn overflowed(&self) -> bool {
        self.overflowed
    }

    fn report(self, folder: FolderId, uid_validity: UidValidity, say: &mut dyn FnMut(EventKind)) {
        if !self.vanished.is_empty() {
            say(EventKind::MessagesExpunged { folder, uid_validity, uids: UidSet::from(self.vanished) });
        }
        let mut changes = self.changes.into_values().peekable();
        while changes.peek().is_some() {
            let chunk: Vec<FlagChange> = changes.by_ref().take(self.batch).collect();
            say(EventKind::MessagesChanged { folder, uid_validity, changes: chunk });
        }
    }
}

fn flag_change(uid: Uid, flags: &[String], modseq: Option<ModSeq>) -> FlagChange {
    FlagChange { uid, flags: flags.iter().filter_map(|flag| Flag::parse(flag).ok()).collect(), modseq }
}

/// The UIDs from `first` to `last` that are in none of the `present` ranges.
fn missing(first: Uid, last: Uid, present: Vec<(Uid, Uid)>) -> UidSet {
    let mut gaps = Vec::new();
    let mut next = Some(first);
    for (from, to) in UidSet::from(present).ranges().iter().filter(|(from, to)| *to >= first && *from <= last) {
        if let Some(gap) = next.filter(|gap| gap < from)
            && let Some(end) = Uid::new(from.get().saturating_sub(1))
        {
            gaps.push((gap, end));
        }
        next = Uid::new(to.get().saturating_add(1)).filter(|_| *to < last);
    }
    if let Some(from) = next {
        gaps.push((from, last));
    }
    UidSet::from(gaps)
}

/// The body of one message, when its UIDVALIDITY still holds.
fn fetch_body(connection: &mut ImapConnection, path: &FolderPath, uid_validity: UidValidity, uid: Uid) -> Result<Vec<u8>, NetError> {
    let (_status, current) = examine(connection, path, None, &mut |_| {})?;
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
