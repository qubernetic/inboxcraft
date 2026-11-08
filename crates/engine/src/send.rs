// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sending: the hold before a send goes, the message made when it goes, and what a worker answers to it
//! (`mail.send`, see `crate::cmd`).
//!
//! - Every send waits in the outbox for [`Hold`] (5 to 60 seconds, 5 unless the owner chose otherwise) or until the
//!   time the owner picked, as a schedule entry ([`inboxcraft_model::Due::Send`]), and can be taken back until then,
//!   its draft as it was. When [`crate::Session::tick`] fires the entry, the message is made from the draft with its
//!   own `Message-ID` and the `Date` of that moment ([`go`]) and handed to the account's worker; a draft that can no
//!   longer go then (its account lost its submission server, say) stays, its outbox item refused with why.
//! - Nothing goes while the computer is off or asleep, or InboxCraft is closed ([`OFF`]): a time missed then fires at
//!   the next start, late, and the owner is told ([`crate::notify::Notice::SentLate`]).
//! - Sent: the draft and its outbox item go, its copy in the Drafts folder is removed for good, and the message as it
//!   went is appended to the account's Sent folder (`\Seen`), both kept in the job queue until they end. An account
//!   without a Sent folder keeps no copy.
//! - Failed: the draft stays, and its outbox item holds the server's reply, or the cause where the server gave none,
//!   so the owner can change it and send it again.
//!
//! An answer for a draft that is not on its way, or from another account's worker, is dropped. A send still on its way
//! when the program stopped gets no answer: a new session marks it with [`CUT_OFF`], so the owner can send it again.

use std::collections::BTreeSet;
use std::time::Duration;

use inboxcraft_model::{AccountId, Draft, DraftId, EmailAddress, ErrorKind, Flag, JobKind, OutboxState, Outgoing, Role};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{MailDate, Timestamp, Zone};

use crate::sync::Plan;
use crate::{Context, EngineError, act, draft, reply};

/// What the owner is told of a send later: nothing goes while InboxCraft cannot run.
pub const OFF: &str =
    "Nothing is sent while the computer is off or asleep, or InboxCraft is closed: a time missed then is sent at the next start, and you are told.";

/// How long every send waits before it goes, so it can be taken back: [`Hold::MIN`] to [`Hold::MAX`] seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hold(u8);

impl Hold {
    /// The shortest hold, in seconds, and the one until the owner chooses.
    pub const MIN: u8 = 5;

    /// The longest hold, in seconds.
    pub const MAX: u8 = 60;

    /// A hold of `seconds`, if it is in the range.
    #[must_use]
    pub fn new(seconds: u64) -> Option<Self> {
        u8::try_from(seconds).ok().filter(|seconds| (Self::MIN..=Self::MAX).contains(seconds)).map(Self)
    }

    /// The hold in seconds.
    #[must_use]
    pub const fn seconds(self) -> u8 {
        self.0
    }

    /// The hold as a duration.
    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl Default for Hold {
    fn default() -> Self {
        Self(Self::MIN)
    }
}

/// A draft made into the message that goes: its outbox item on its way, and the job that sends it.
pub(crate) struct Ready {
    pub(crate) account: AccountId,
    pub(crate) outgoing: Outgoing,
    pub(crate) job: JobKind,
}

/// `draft` made into its message at the context's time, or why it cannot go now.
pub(crate) fn ready(context: &Context<'_>, draft: &Draft) -> Result<Result<Ready, &'static str>, EngineError> {
    let store = context.store();
    if !draft::blocks(store, draft)?.is_empty() {
        return Ok(Err("the draft cannot be sent as it is"));
    }
    let Some(account) = store.accounts()?.into_iter().find(|account| account.id() == draft.account) else {
        return Ok(Err("the draft's account is gone"));
    };
    if account.submission().is_none() {
        return Ok(Err("the account has no submission server"));
    }
    let Some(attachments) = reply::forwarded(store, draft)? else {
        return Ok(Err("the message it forwards is no longer kept, so its attachments are not"));
    };
    let now = context.now();
    let message_id = message_id(draft, account.address(), now);
    let date = MailDate::new(now, Zone::Fixed(context.offsets().offset_at(now)));
    let Ok(raw) = draft::built(&account, draft, date, Some(&message_id), &attachments) else {
        return Ok(Err("the draft cannot be made into a message"));
    };
    let outgoing = Outgoing { message: raw.clone(), ..Outgoing::new(draft.id, OutboxState::Sending) };
    let job = JobKind::Send { draft: draft.id, from: account.address().clone(), recipients: envelope(draft), raw };
    Ok(Ok(Ready { account: account.id(), outgoing, job }))
}

/// A send's job for the worker of its account.
pub(crate) type SendJob = (AccountId, JobKind);

/// The send of `draft` whose time came: its outbox item, on its way or refused with why, and the job that sends it;
/// nothing when it no longer waits (it was taken back).
pub(crate) fn go(context: &Context<'_>, draft: DraftId) -> Result<Option<(Change, Option<SendJob>)>, EngineError> {
    let store = context.store();
    let Some(item) = store.outbox()?.into_iter().find(|item| item.draft == draft && item.state == OutboxState::Waiting) else { return Ok(None) };
    let Some(held) = store.draft(draft)? else { return Ok(None) };
    Ok(Some(match ready(context, &held)? {
        Ok(Ready { account, outgoing, job }) => (Change::PutOutgoing(outgoing), Some((account, job))),
        Err(why) => (Change::PutOutgoing(Outgoing { state: OutboxState::Refused { reply: why.to_owned() }, ..item }), None),
    }))
}

/// The envelope's recipients: To, Cc and Bcc, each address once, in that order.
fn envelope(draft: &Draft) -> Vec<EmailAddress> {
    let mut seen = BTreeSet::new();
    draft
        .to
        .iter()
        .chain(&draft.cc)
        .chain(&draft.bcc)
        .filter_map(|text| draft::recipient(text).ok().map(|(_, address)| address))
        .filter(|address| seen.insert(address.as_str().to_lowercase()))
        .collect()
}

/// A `Message-ID` of our own: the time to the nanosecond and the draft, at the domain of the sender's address (or
/// `invalid` where it is not ASCII), so no two sends share one and it tells nothing of the computer.
fn message_id(draft: &Draft, from: &EmailAddress, now: Timestamp) -> String {
    let domain =
        Some(from.domain().to_ascii_lowercase()).filter(|domain| domain.is_ascii() && !domain.is_empty()).unwrap_or_else(|| "invalid".to_owned());
    format!("<{}.{:09}.{}@{domain}>", now.unix_seconds(), now.subsec_nanos(), draft.id.get())
}

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
