// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Events in: what a `net` worker reports, turned into store changes, session state and the next jobs.
//!
//! Like a command, planning an event only reads; the session makes the plan afterwards. An event about an account
//! or a folder the session does not hold, or with UIDs of an older UIDVALIDITY, is dropped.

use std::collections::{BTreeMap, BTreeSet};

use inboxcraft_model::{
    AccountId, CheckStep, Event, EventKind, FlagChange, Folder, FolderId, FolderListing, FolderPath, JobId, JobKind, Message, Role, StepReport,
    Summary, Uid, UidSet, UidValidity,
};
use inboxcraft_store::{Batch, Body, Change, Storage};

use crate::act::{self, Effects};
use crate::arrive::{self, Arrival};
use crate::session::{Pending, State};
use crate::write::{self, Moved};
use crate::{Context, EngineError};

/// What an event changes: the session's new state, the store changes and the jobs that follow.
pub(crate) struct Plan {
    pub(crate) state: State,
    pub(crate) batch: Batch,
    pub(crate) jobs: Vec<Pending>,
}

impl Plan {
    pub(crate) fn change(&mut self, change: Change) {
        self.batch.push(change);
    }

    pub(crate) fn job(&mut self, account: AccountId, kind: JobKind) {
        self.jobs.push(Pending { account, kind, message: None, keep: false });
    }

    /// A job kept in the store's queue until its last event.
    pub(crate) fn kept_job(&mut self, account: AccountId, kind: JobKind) {
        self.jobs.push(Pending { account, kind, message: None, keep: true });
    }

    /// An act's changes and kept jobs, as a stage of the arriving-mail pipeline adds them.
    pub(crate) fn effects(&mut self, effects: Effects) {
        for change in effects.changes {
            self.change(change);
        }
        for (account, kind) in effects.jobs {
            self.kept_job(account, kind);
        }
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
        return Ok(ended(context, event));
    }
    let mut plan = Plan { state: context.state().clone(), batch: Batch::new(), jobs: Vec::new() };
    let kept = match &event.kind {
        EventKind::Folders { listings } => folders(store, &mut plan, account, listings)?,
        EventKind::MessagesAdded { folder, uid_validity, messages } => added(context, &mut plan, account, *folder, *uid_validity, messages)?,
        EventKind::MessagesChanged { folder, uid_validity, changes } => changed(store, &mut plan, *folder, *uid_validity, changes)?,
        EventKind::MessagesExpunged { folder, uid_validity, uids } => expunged(store, &mut plan, *folder, *uid_validity, uids)?,
        EventKind::UidValidityReset { folder, new, .. } => reset(store, &mut plan, account, *folder, *new)?,
        EventKind::BodyFetched { folder, uid_validity, uid, raw } => fetched(store, &mut plan, *folder, *uid_validity, *uid, raw)?,
        EventKind::FolderSynced { folder, state } => {
            let kept = current(&mut plan.state, *folder, state.uid_validity());
            if kept {
                plan.change(Change::SetFolderState(*folder, *state));
            }
            kept
        }
        EventKind::FlagsStored { folder, uid_validity, .. } => current(&mut plan.state, *folder, *uid_validity),
        EventKind::Moved { folder, uid_validity, uids, to, copied } => {
            let moved = Moved { folder: *folder, uid_validity: *uid_validity, uids, to: *to, copied: copied.as_ref() };
            write::moved(store, &mut plan, account, &moved)?
        }
        EventKind::Expunged { folder, uid_validity, uids } => expunged(store, &mut plan, *folder, *uid_validity, uids)?,
        EventKind::Appended { folder, uid } => {
            match write::copy_of(store, &plan, event.job)? {
                Some(message) => write::appended(store, &mut plan, message, *folder, *uid)?,
                None => crate::draft::appended(store, &mut plan, account, event.job, *folder, *uid)?,
            }
            true
        }
        EventKind::FolderCreated { .. } => {
            plan.job(account, JobKind::SyncFolderList);
            true
        }
        EventKind::CheckStep(report) => {
            let kept = checked(&mut plan.state, account, event.job, report);
            // A new account: its first sign-in that passes starts the sync, so the password goes out no sooner.
            if kept && report.step() == CheckStep::ImapLogin && report.status().passed() && store.folders(account)?.is_empty() {
                plan.job(account, JobKind::SyncFolderList);
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
        EventKind::Sent { draft } => crate::send::sent(store, &mut plan, account, *draft)?,
        EventKind::SendFailed { draft, kind, reply } => crate::send::failed(store, &mut plan, account, *draft, *kind, reply.as_deref())?,
        // The calendar engine takes these; until it does, they change nothing.
        EventKind::Calendar(_) => false,
        EventKind::Abilities { uid_expunge } => {
            plan.state.accounts.entry(account).or_default().uid_expunge = *uid_expunge;
            true
        }
        EventKind::Failed { kind, folder } => {
            plan.state.accounts.entry(account).or_default().failure = Some((*kind, *folder));
            if let Some(job) = event.job {
                plan.state.fetching.remove(&job);
            }
            write::failed(store, &mut plan, account, event.job)?;
            true
        }
    };
    Ok(kept.then_some(plan))
}

/// For an event about a folder we no longer hold: a failure still ends the `FetchBody` job it answers, and nothing
/// else of the event is kept.
fn ended(context: &Context<'_>, event: &Event) -> Option<Plan> {
    let job = event.job.filter(|job| matches!(event.kind, EventKind::Failed { .. }) && context.state().fetching.contains_key(job))?;
    let mut state = context.state().clone();
    state.fetching.remove(&job);
    Some(Plan { state, batch: Batch::new(), jobs: Vec::new() })
}

/// A step of the running check of `account`, from its job: it takes the step's place, and the last step ends the
/// check. A step of another job, or with no check running, is stale.
fn checked(state: &mut State, account: AccountId, job: Option<JobId>, report: &StepReport) -> bool {
    let Some(check) = state.accounts.get_mut(&account).and_then(|held| held.check.as_mut()) else { return false };
    if job.is_none() || check.job != job {
        return false;
    }
    if let Some(entry) = check.steps.iter_mut().find(|entry| entry.step() == report.step()) {
        *entry = report.clone();
    }
    if CheckStep::ALL.last() == Some(&report.step()) {
        check.job = None;
    }
    true
}

/// Whether UIDs of `uid_validity` belong to what we hold of `folder`; the first UIDVALIDITY seen is taken.
pub(crate) fn current(state: &mut State, folder: FolderId, uid_validity: UidValidity) -> bool {
    *state.validity.entry(folder).or_insert(uid_validity) == uid_validity
}

fn same_path(a: &FolderPath, b: &FolderPath) -> bool {
    a.name() == b.name() || (a.is_inbox() && b.is_inbox())
}

/// The account's whole folder list: held folders keep their ids, new ones get one, missing ones go. Then every
/// folder that holds mail is synced, the inbox first, and the inbox is watched.
fn folders(store: &dyn Storage, plan: &mut Plan, account: AccountId, listings: &[FolderListing]) -> Result<bool, EngineError> {
    let held = store.folders(account)?;
    // Our own folders on their way to the server: kept, and synced once the server lists them.
    let making: Vec<FolderPath> = store
        .queued_jobs()?
        .into_iter()
        .filter(|job| job.account == account)
        .filter_map(|job| match job.kind {
            JobKind::CreateFolder { path } => Some(path),
            _ => None,
        })
        .collect();
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
    let listed = next.len();
    let missing: Vec<&Folder> = held.iter().filter(|folder| next.iter().all(|kept| kept.id() != folder.id())).collect();
    for gone in missing {
        if making.iter().any(|path| same_path(path, gone.listing().path())) {
            next.push(gone.clone());
            continue;
        }
        plan.change(Change::RemoveFolder(gone.id()));
        plan.state.validity.remove(&gone.id());
    }
    let held_state = plan.state.accounts.entry(account).or_default();
    let questions = held_state.roles.detect(&next);
    held_state.ask.extend(questions);
    held_state.ask.retain(|role| held_state.roles.folder(*role).is_none());
    let inbox = held_state.roles.folder(Role::Inbox);
    let mut order: Vec<&Folder> = next.iter().take(listed).filter(|folder| folder.listing().is_selectable()).collect();
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

/// The messages of `folder` we hold under the server's UIDs, by UID: not those whose move is on its way, whose UID
/// is ours.
pub(crate) fn held(store: &dyn Storage, folder: FolderId) -> Result<BTreeMap<Uid, Message>, EngineError> {
    Ok(store
        .messages(folder)?
        .into_iter()
        .filter(|entry| entry.message().moving().is_none())
        .map(|entry| (entry.message().summary().uid(), entry.message().clone()))
        .collect())
}

/// Messages new to us or changed: kept under their ids, and the new ones sent through the arriving-mail pipeline.
fn added(
    context: &Context<'_>,
    plan: &mut Plan,
    account: AccountId,
    folder: FolderId,
    uid_validity: UidValidity,
    messages: &[Summary],
) -> Result<bool, EngineError> {
    if !current(&mut plan.state, folder, uid_validity) {
        return Ok(false);
    }
    let store = context.store();
    let entries = store.messages(folder)?;
    let mut taken: BTreeSet<Uid> = entries.iter().map(|entry| entry.message().summary().uid()).collect();
    taken.extend(messages.iter().map(Summary::uid));
    let mut ours: BTreeMap<Uid, Message> = entries
        .iter()
        .filter(|entry| entry.message().moving().is_some())
        .map(|entry| (entry.message().summary().uid(), entry.message().clone()))
        .collect();
    let held = held(store, folder)?;
    let mut given: BTreeMap<Uid, inboxcraft_model::MessageId> = BTreeMap::new();
    let mut new = Vec::new();
    for summary in messages {
        let uid = summary.uid();
        if let Some(waiting) = ours.remove(&uid) {
            // The server named a UID we gave a message whose move is on its way: ours gives way.
            let free = act::free_uid(&taken);
            taken.extend(free);
            plan.change(free.map_or(Change::RemoveMessage(waiting.id()), |free| {
                Change::PutMessage(act::placed(&waiting, folder, free, waiting.moving().cloned()))
            }));
        }
        let old = held.get(&uid);
        let id = match (old, given.get(&uid)) {
            (_, Some(id)) => *id,
            (Some(old), None) => old.id(),
            (None, None) => plan.state.message_id(),
        };
        let message = Message::new(id, account, folder, summary.clone()).with_thread(old.and_then(Message::thread));
        let first = given.insert(uid, id).is_none();
        if old.is_none() && first {
            new.push(message.clone());
        }
        plan.change(Change::PutMessage(message));
    }
    let role = plan.state.accounts.get(&account).and_then(|held| held.roles.role_of(folder));
    let arrival = Arrival { store, now: context.now(), account, folder, role, first_sync: store.folder_state(folder)?.is_none() };
    arrive::arrived(arrive::STAGES, &arrival, &new, plan)?;
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

/// Every UID we hold of `folder` is void: its messages go, and so do those whose move from it is on its way; the next
/// ones belong to `new`.
fn reset(store: &dyn Storage, plan: &mut Plan, account: AccountId, folder: FolderId, new: UidValidity) -> Result<bool, EngineError> {
    for message in held(store, folder)?.values() {
        plan.change(Change::RemoveMessage(message.id()));
    }
    write::void(store, plan, account, folder)?;
    plan.state.validity.insert(folder, new);
    Ok(true)
}

/// A whole message: kept with its plain text for the search. A message we do not hold is dropped; for one we hold,
/// the answer ends the fetch, and a body too large to keep is marked so it is not asked for again.
fn fetched(store: &dyn Storage, plan: &mut Plan, folder: FolderId, uid_validity: UidValidity, uid: Uid, raw: &[u8]) -> Result<bool, EngineError> {
    if !current(&mut plan.state, folder, uid_validity) {
        return Ok(false);
    }
    let Some(message) = held(store, folder)?.remove(&uid) else { return Ok(false) };
    plan.state.fetching.retain(|_, out| *out != message.id());
    match Body::new(raw.to_vec(), &plain_text(raw)) {
        Ok(body) => {
            plan.change(Change::PutBody(message.id(), body));
            // A calendar part, known only now, makes the message an invitation in the grouped inbox.
            let envelope = message.summary().envelope();
            let calendar = arrive::group::calendar(raw);
            if calendar != envelope.signs().calendar() {
                let envelope = envelope.clone().with_signs(envelope.signs().with_calendar(calendar));
                let summary = message.summary().clone().with_envelope(envelope);
                let thread = message.thread();
                let moving = message.moving().cloned();
                plan.change(Change::PutMessage(
                    Message::new(message.id(), message.account(), message.folder(), summary).with_thread(thread).with_moving(moving),
                ));
            }
        }
        Err(_) => {
            plan.state.too_large.insert(message.id());
        }
    }
    Ok(true)
}

/// The text a search looks at: the message's plain text part, or nothing when it has none or cannot be read.
pub(crate) fn plain_text(raw: &[u8]) -> String {
    inboxcraft_mime::parse(raw).ok().and_then(|message| message.text_body().and_then(|part| part.text().ok())).unwrap_or_default()
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
