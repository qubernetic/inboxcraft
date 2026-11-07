// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a worker answers to a send (`mail.send`, see `crate::cmd`).
//!
//! - Sent: the draft and its outbox item go, its copy in the Drafts folder is removed for good, and the message as it
//!   went is appended to the account's Sent folder (`\Seen`), both kept in the job queue until they end. An account
//!   without a Sent folder keeps no copy.
//! - Failed: the draft stays, and its outbox item holds the server's reply, or the cause where the server gave none,
//!   so the owner can change it and send it again.
//!
//! An answer for a draft that is not on its way, or from another account's worker, is dropped. A send still on its way
//! when the program stopped gets no answer: a new session marks it with [`CUT_OFF`], so the owner can send it again.

use inboxcraft_model::{AccountId, DraftId, ErrorKind, Flag, JobKind, OutboxState, Outgoing, Role};
use inboxcraft_store::{Batch, Change, Storage};

use crate::sync::Plan;
use crate::{EngineError, act, draft};

/// What the outbox item of a send cut off by a stop says.
pub(crate) const CUT_OFF: &str = "the program stopped before the server answered, so the message may have gone";

/// The outbox items still on their way in `store`, marked as cut off: a new session will not hear of them.
pub(crate) fn cut_off(store: &dyn Storage) -> Result<Batch, EngineError> {
    Ok(store.outbox()?.into_iter().filter(|item| item.state == OutboxState::Sending).fold(Batch::new(), |batch, item| {
        batch.with(Change::PutOutgoing(Outgoing { state: OutboxState::Refused { reply: CUT_OFF.to_owned() }, ..item }))
    }))
}

/// The outbox item of `draft` of `account`, while it is on its way.
fn sending(store: &dyn Storage, account: AccountId, draft: DraftId) -> Result<Option<Outgoing>, EngineError> {
    let Some(held) = store.draft(draft)? else { return Ok(None) };
    if held.account != account {
        return Ok(None);
    }
    Ok(store.outbox()?.into_iter().find(|item| item.draft == draft && item.state == OutboxState::Sending))
}

/// The submission server took the message of `draft`; whether the answer was kept.
pub(crate) fn sent(store: &dyn Storage, plan: &mut Plan, account: AccountId, draft: DraftId) -> Result<bool, EngineError> {
    let Some(outgoing) = sending(store, account, draft)? else { return Ok(false) };
    if let Some(copy) = store.draft(draft)?.and_then(|held| held.uploaded)
        && let Some(kind) = draft::take_back(store, account, copy)?
    {
        plan.kept_job(account, kind);
    }
    let sent_folder = plan.state.accounts.get(&account).and_then(|held| held.roles.folder(Role::Sent));
    if let Some(folder) = sent_folder
        && let Some(folder) = act::folder(store, account, folder)?
    {
        let flags = std::iter::once(Flag::Seen).collect();
        plan.kept_job(
            account,
            JobKind::Append { folder: folder.id(), path: folder.listing().path().clone(), flags, raw: outgoing.message, draft: None, message: None },
        );
    }
    plan.change(Change::RemoveDraft(draft));
    Ok(true)
}

/// The send of `draft` failed for `kind`, with the server's `reply` if it refused; whether the answer was kept.
pub(crate) fn failed(
    store: &dyn Storage,
    plan: &mut Plan,
    account: AccountId,
    draft: DraftId,
    kind: ErrorKind,
    reply: Option<&str>,
) -> Result<bool, EngineError> {
    let Some(outgoing) = sending(store, account, draft)? else { return Ok(false) };
    let reply = reply.map_or_else(|| kind.to_string(), str::to_owned);
    plan.change(Change::PutOutgoing(Outgoing { state: OutboxState::Refused { reply }, ..outgoing }));
    Ok(true)
}
