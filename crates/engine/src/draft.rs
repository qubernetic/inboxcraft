// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Drafts: what the owner writes, kept in the store at every change, so a kill loses none of it, and copied to the
//! account's Drafts folder once it has been still for [`UPLOAD_AFTER`].
//!
//! - The recipients are kept as typed; each is checked as `Name <address>` or a bare address, and one that cannot be
//!   an address is marked with what is wrong and blocks Send, like a draft without any recipient.
//! - A new draft is from the account that last sent (the newest message in a Sent folder), else the first account,
//!   and starts with that account's signature below `-- `; another account swaps the signature when it is still at
//!   the end as it was.
//! - [`crate::Session::tick`] hands each draft that changed and has been still for [`UPLOAD_AFTER`] to its account's
//!   worker as a kept `APPEND` (`\Seen \Draft`), without Bcc and without the recipients that are no address. A draft
//!   with nothing typed, or of an account without a Drafts folder, stays here. The server's answer names the copy's
//!   UID (`APPENDUID`); the copy before it is then removed for good, and so is a copy that comes back for a draft
//!   discarded meanwhile. A draft on its way (held, waiting for its time or sending) is not copied; taken back, it
//!   is copied at the next tick. A server without UIDPLUS names no UID: that copy can be neither replaced nor taken
//!   back, so no further copy of the draft goes up and it leaves at most one. A failed upload waits for the draft's
//!   next change.

use std::time::Duration;

use inboxcraft_mime::{Attachment, Mailbox, MessageBuilder, MimeError};
use inboxcraft_model::{
    Account, AccountId, AddressError, Draft, DraftCopy, DraftId, EmailAddress, Flag, Flags, FolderId, JobId, JobKind, MessageId, MessageRef,
    OutboxState, Role, Uid, UidSet, UidValidity,
};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{MailDate, Offset, Timestamp, Zone};
use serde_json::{Value, json};

use crate::session::{Pending, State};
use crate::sync::Plan;
use crate::{Context, EngineError, act, json, reply, template, view};

/// The longest subject, in bytes.
pub(crate) const MAX_SUBJECT_BYTES: usize = 998;

/// The longest text, in bytes.
pub(crate) const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// How long a draft stays still before its copy goes to the server.
pub(crate) const UPLOAD_AFTER: Duration = Duration::from_secs(10);

/// The line above a signature (RFC 3676 section 4.3), with the blank line before it.
const SIGNATURE_LINE: &str = "\n\n-- \n";

/// The recipient `text` as typed: its name, if any, and its address, or what is wrong with it.
pub(crate) fn recipient(text: &str) -> Result<(Option<&str>, EmailAddress), AddressError> {
    let trimmed = text.trim();
    let named = trimmed
        .strip_suffix('>')
        .and_then(|rest| rest.rfind('<').map(|at| (rest.get(..at).unwrap_or_default(), rest.get(at.saturating_add(1)..).unwrap_or_default())));
    let (name, address) = match named {
        Some((name, address)) => (Some(name.trim().trim_matches('"').trim()).filter(|name| !name.is_empty()), address),
        None => (None, trimmed),
    };
    Ok((name, EmailAddress::new(address)?))
}

/// What keeps the draft from being sent: `no_recipient`, `bad_address`, `blank` (a template's blank not filled in),
/// `forward_body` (the body of the message it forwards is no longer kept, so neither are its attachments); none when
/// it can go.
pub(crate) fn blocks(store: &dyn Storage, draft: &Draft) -> Result<Vec<&'static str>, EngineError> {
    let mut blocks = Vec::new();
    if draft.to.is_empty() && draft.cc.is_empty() && draft.bcc.is_empty() {
        blocks.push("no_recipient");
    }
    if draft.to.iter().chain(&draft.cc).chain(&draft.bcc).any(|text| recipient(text).is_err()) {
        blocks.push("bad_address");
    }
    if !template::open_blanks(draft).is_empty() {
        blocks.push("blank");
    }
    if let Some(message) = draft.forwards
        && store.body(message)?.is_none()
    {
        blocks.push("forward_body");
    }
    Ok(blocks)
}

/// Whether `draft` is on its way: its outbox item waits for the worker's answer. Such a draft is neither sent
/// again, changed nor discarded, so the answer finds it as it went.
pub(crate) fn on_its_way(store: &dyn Storage, draft: DraftId) -> Result<bool, EngineError> {
    Ok(store.outbox()?.into_iter().any(|item| item.draft == draft && !matches!(item.state, OutboxState::Refused { .. })))
}

/// A draft as the commands answer it, every recipient checked.
pub(crate) fn json(store: &dyn Storage, draft: &Draft) -> Result<Value, EngineError> {
    let blocks = blocks(store, draft)?;
    Ok(json!({
        "id": draft.id.get(),
        "account": draft.account.get(),
        "saved": json::date(draft.saved),
        "to": recipients(&draft.to),
        "cc": recipients(&draft.cc),
        "bcc": recipients(&draft.bcc),
        "subject": draft.subject,
        "text": draft.text,
        "answers": draft.answers.map(MessageId::get),
        "in_reply_to": draft.in_reply_to.as_ref().map(MessageRef::as_str),
        "references": draft.references.iter().map(MessageRef::as_str).collect::<Vec<_>>(),
        "forwards": draft.forwards.map(MessageId::get),
        "blanks": template::open_blanks(draft),
        "uploaded": draft.uploaded.is_some(),
        "sendable": blocks.is_empty(),
        "blocks": blocks,
    }))
}

fn recipients(list: &[String]) -> Value {
    list.iter()
        .map(|text| match recipient(text) {
            Ok(_) => json!({ "text": text, "ok": true }),
            Err(error) => json!({ "text": text, "ok": false, "problem": error.to_string() }),
        })
        .collect()
}

/// The text a new draft of `account` starts with: its signature below the signature line, or nothing.
pub(crate) fn signed(account: &Account) -> String {
    account.signature().map(|signature| format!("{SIGNATURE_LINE}{signature}")).unwrap_or_default()
}

/// `text` written from `to` instead of `from`: the signature of `from` at its end, as it was, becomes that of `to`.
pub(crate) fn resigned(text: &str, from: &Account, to: &Account) -> String {
    let old = signed(from);
    match text.strip_suffix(old.as_str()) {
        Some(rest) => format!("{rest}{}", signed(to)),
        None => text.to_owned(),
    }
}

/// The account a new draft is from: the one that last sent, else the first one.
pub(crate) fn last_sent(context: &Context<'_>) -> Result<Option<Account>, EngineError> {
    let store = context.store();
    let accounts = store.accounts()?;
    let mut newest: Option<(Timestamp, AccountId)> = None;
    for account in &accounts {
        let Some(sent) = context.state().accounts.get(&account.id()).and_then(|state| state.roles.folder(Role::Sent)) else { continue };
        for entry in store.messages(sent)? {
            if let Some(date) = view::date(entry.message())
                && newest.is_none_or(|(held, _)| date > held)
            {
                newest = Some((date, account.id()));
            }
        }
    }
    let chosen = newest.and_then(|(_, id)| accounts.iter().find(|account| account.id() == id)).or(accounts.first());
    Ok(chosen.cloned())
}

/// The job that removes `copy` of a draft of `account` for good, unless its folder is gone.
pub(crate) fn take_back(store: &dyn Storage, account: AccountId, copy: DraftCopy) -> Result<Option<JobKind>, EngineError> {
    Ok(act::folder(store, account, copy.folder)?.map(|folder| JobKind::Expunge {
        folder: copy.folder,
        path: folder.listing().path().clone(),
        uid_validity: copy.uid_validity,
        uids: std::iter::once(copy.uid).collect::<UidSet>(),
    }))
}

/// The Drafts folder of `account`, if it has one.
fn drafts_folder(store: &dyn Storage, state: &State, account: AccountId) -> Result<Option<inboxcraft_model::Folder>, EngineError> {
    match state.accounts.get(&account).and_then(|held| held.roles.folder(Role::Drafts)) {
        Some(folder) => act::folder(store, account, folder),
        None => Ok(None),
    }
}

/// Whether `draft` of `account` holds something the owner typed: a recipient, a subject, or text besides the signature.
fn typed(draft: &Draft, account: &Account) -> bool {
    let recipients = !(draft.to.is_empty() && draft.cc.is_empty() && draft.bcc.is_empty());
    let text = !draft.text.trim().is_empty() && draft.text != signed(account);
    recipients || !draft.subject.trim().is_empty() || text
}

/// The drafts whose copy is due some time, each with its account and Drafts folder; not one with an unnamed copy.
fn waiting(store: &dyn Storage, state: &State) -> Result<Vec<(Account, inboxcraft_model::Folder, Draft)>, EngineError> {
    let going: Vec<DraftId> =
        store.outbox()?.into_iter().filter(|item| !matches!(item.state, OutboxState::Refused { .. })).map(|item| item.draft).collect();
    let mut found = Vec::new();
    for account in store.accounts()? {
        let Some(folder) = drafts_folder(store, state, account.id())? else { continue };
        for draft in store.drafts(account.id())? {
            if draft.synced != Some(draft.saved) && !draft.unnamed && typed(&draft, &account) && !going.contains(&draft.id) {
                found.push((account.clone(), folder.clone(), draft));
            }
        }
    }
    Ok(found)
}

/// When the next draft copy is due.
pub(crate) fn next_upload(store: &dyn Storage, state: &State) -> Result<Option<Timestamp>, EngineError> {
    Ok(waiting(store, state)?.iter().map(|(_, _, draft)| draft.saved.saturating_add(UPLOAD_AFTER)).min())
}

/// The copies due at the context's time: each draft marked as handed over, and its kept `APPEND`.
pub(crate) fn uploads(context: &Context<'_>) -> Result<(Batch, Vec<Pending>), EngineError> {
    let mut batch = Batch::new();
    let mut jobs = Vec::new();
    for (account, folder, draft) in waiting(context.store(), context.state())? {
        if draft.saved.saturating_add(UPLOAD_AFTER) > context.now() {
            continue;
        }
        // A copy the builder refuses is not sent; the draft waits for its next change. A forward whose message's body
        // is gone goes up without its attachments: it is a copy, and Send refuses it.
        let attachments = reply::forwarded(context.store(), &draft)?.unwrap_or_default();
        if let Ok(raw) = message(&account, &draft, &attachments) {
            let flags: Flags = [Flag::Seen, Flag::Draft].into_iter().collect();
            let kind =
                JobKind::Append { folder: folder.id(), path: folder.listing().path().clone(), flags, raw, draft: Some(draft.id), message: None };
            jobs.push(Pending { account: account.id(), kind, message: None, keep: true, until: None });
        }
        batch.push(Change::PutDraft(Draft { synced: Some(draft.saved), ..draft }));
    }
    Ok((batch, jobs))
}

/// The copy of `draft` of `account`: From, Date (its saved time), To and Cc with the addresses that are ones, Subject,
/// the thread's ids, the text and `attachments`.
fn message(account: &Account, draft: &Draft, attachments: &[Attachment]) -> Result<Vec<u8>, MimeError> {
    built(account, draft, MailDate::new(draft.saved, Zone::Fixed(Offset::UTC)), None, attachments)
}

/// `draft` of `account` as a message: From, `date`, To and Cc with the addresses that are ones, Subject,
/// `message_id`, `In-Reply-To` and `References`, the text, its calendar object (with the object's `METHOD`; one that
/// names none or cannot be read makes no message) and `attachments`. Bcc is never in it.
pub(crate) fn built(
    account: &Account,
    draft: &Draft,
    date: Option<MailDate>,
    message_id: Option<&str>,
    attachments: &[Attachment],
) -> Result<Vec<u8>, MimeError> {
    let address = account.address().as_str();
    let from = account.display_name().map_or_else(|| Mailbox::new(address), |name| Mailbox::named(name, address));
    let mut builder = MessageBuilder::new().from(from).subject(&draft.subject).text(&draft.text);
    if let Some(date) = date {
        builder = builder.date(date);
    }
    if let Some(id) = message_id {
        builder = builder.message_id(id);
    }
    if let Some(id) = &draft.in_reply_to {
        builder = builder.in_reply_to(id.as_str());
    }
    for id in &draft.references {
        builder = builder.reference(id.as_str());
    }
    if let Some(calendar) = &draft.calendar {
        let method = inboxcraft_ical::Calendar::parse(calendar.as_bytes()).ok().and_then(|object| object.method).unwrap_or_default();
        builder = builder.calendar(&method, calendar);
    }
    for attachment in attachments {
        builder = builder.attach(attachment.clone());
    }
    for (name, address) in draft.to.iter().filter_map(|text| recipient(text).ok()) {
        builder = builder.to(mailbox(name, &address));
    }
    for (name, address) in draft.cc.iter().filter_map(|text| recipient(text).ok()) {
        builder = builder.cc(mailbox(name, &address));
    }
    builder.build()
}

fn mailbox(name: Option<&str>, address: &EmailAddress) -> Mailbox {
    name.map_or_else(|| Mailbox::new(address.as_str()), |name| Mailbox::named(name, address.as_str()))
}

/// The answer to a kept `APPEND` of `account`: the draft it copied now has its copy at `uid` in `folder`, and the copy
/// before goes; a copy of a draft that is gone goes too. Without a UID the draft is marked as having an unnamed copy.
pub(crate) fn appended(
    store: &dyn Storage,
    plan: &mut Plan,
    account: AccountId,
    job: Option<JobId>,
    folder: FolderId,
    uid: Option<(UidValidity, Uid)>,
) -> Result<(), EngineError> {
    let Some(job) = job.filter(|job| plan.state.kept.contains(job)) else { return Ok(()) };
    let Some(draft) = store.queued_jobs()?.into_iter().find(|queued| queued.id == job).and_then(|queued| match queued.kind {
        JobKind::Append { draft, .. } => draft,
        _ => None,
    }) else {
        return Ok(());
    };
    let copy = uid.map(|(uid_validity, uid)| DraftCopy { folder, uid_validity, uid });
    let gone = match store.draft(draft)? {
        Some(held) if held.account == account => {
            plan.change(Change::PutDraft(Draft { uploaded: copy, unnamed: copy.is_none(), ..held.clone() }));
            held.uploaded.filter(|old| Some(*old) != copy)
        }
        _ => copy,
    };
    if let Some(gone) = gone
        && let Some(kind) = take_back(store, account, gone)?
    {
        plan.kept_job(account, kind);
    }
    Ok(())
}

/// The id a new draft gets: past every draft the session has held.
pub(crate) fn next_id(state: &State) -> DraftId {
    DraftId::new(state.next_draft)
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
