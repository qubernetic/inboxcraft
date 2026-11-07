// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `mail.*`: lists of messages, one message, the search.

use std::collections::BTreeMap;

use inboxcraft_model::{AccountId, FolderId, JobKind, Message, MessageId, MessageRef, Role, group_threads};
use inboxcraft_store::{Change, Search};
use inboxcraft_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::arrive::screener;
use crate::params::{decode, parse_date};
use crate::view::{self, View};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, json};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[ATTACHMENT, LIST, READ, SEARCH, THREAD];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["account", "folder", "role", "limit", "offset", "id", "index", "content_id", "from", "to", "subject", "text", "since", "before"];

/// The most messages one answer lists.
pub const MAX_LIST: u64 = 1000;

/// How many a list gives when the caller does not say.
const DEFAULT_LIST: u64 = 100;

/// `mail.list`: the messages of one folder, or of one role across every account (the unified view), newest first.
pub(crate) const LIST: CommandSpec = CommandSpec {
    id: "mail.list",
    class: Some(AgentClass::Safe),
    summary: "List the messages of a folder or of a unified view, newest first",
    run: list,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    /// One folder.
    folder: Option<u64>,
    /// A role across the accounts (`inbox`, `sent` ...); the inbox when neither is given.
    role: Option<String>,
    /// Only this account.
    account: Option<u64>,
    /// At most this many, 1 to [`MAX_LIST`].
    limit: Option<u64>,
    /// Skip this many first.
    offset: Option<u64>,
}

/// `limit` between 1 and [`MAX_LIST`], `DEFAULT_LIST` when not given.
pub(crate) fn limit(limit: Option<u64>) -> usize {
    usize::try_from(limit.unwrap_or(DEFAULT_LIST).clamp(1, MAX_LIST)).unwrap_or(1)
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ListParams = decode(LIST.id, params)?;
    let view = match (params.folder, params.role.as_deref()) {
        (Some(folder), None) => View::Folder(FolderId::new(folder)),
        (None, Some(name)) => View::Role(view::role(name).ok_or_else(|| EngineError::bad_params(LIST.id, &format!("no role {name:?}")))?),
        (None, None) => View::Role(Role::Inbox),
        (Some(_), Some(_)) => return Err(EngineError::bad_params(LIST.id, "give a folder or a role, not both")),
    };
    let account = params.account.map(AccountId::new);
    let entries = view::merged(context, &view::folders(context, view, account)?)?;
    let limit = limit(params.limit);
    let offset = usize::try_from(params.offset.unwrap_or(0)).unwrap_or(usize::MAX);
    let accounts = context.store().accounts()?;
    let mut messages = Vec::new();
    for entry in entries.iter().skip(offset).take(limit) {
        let message = entry.message();
        let role = context.state().accounts.get(&message.account()).and_then(|held| held.roles.role_of(message.folder()));
        let new = screener::is_new(context.store(), context.state().screener, role, &accounts, message)?;
        let mut shown = json::message(entry);
        if let Value::Object(fields) = &mut shown {
            fields.insert("new_sender".into(), json!(new));
        }
        messages.push(shown);
    }
    let shown = match view {
        View::Folder(folder) => json!({ "folder": folder.get() }),
        View::Role(role) => json!({ "role": view::role_name(role) }),
    };
    Ok(Outcome::new(json!({
        "view": shown,
        "account": account.map(AccountId::get),
        "total": entries.len(),
        "offset": offset,
        "limit": limit,
        "messages": messages,
    })))
}

/// `mail.read`: one message with its body when it is kept. Opening a message changes nothing on the server: the
/// store keeps its body from now on, and a body not kept yet is fetched (`fetching` in the answer). A body the
/// worker sent but the store could not keep is not asked for again in this session (`body_error`: `too_large`).
pub(crate) const READ: CommandSpec = CommandSpec {
    id: "mail.read",
    class: Some(AgentClass::Safe),
    summary: "Read one message: its headers, its plain text and its attachments",
    run: read,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadParams {
    /// The message.
    id: u64,
}

fn read(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ReadParams = decode(READ.id, params)?;
    read_one(context, params.id)
}

/// `mail.read` of the message `id`.
pub(super) fn read_one(context: &Context<'_>, number: u64) -> Result<Outcome, EngineError> {
    let id = MessageId::new(number);
    let store = context.store();
    let entry = store.message(id)?.ok_or(EngineError::NotFound { what: "message", id: number })?;
    let message = entry.message();
    let body = store.body(id)?;
    let too_large = body.is_none() && context.state().too_large.contains(&id);
    let blocked = blocked(context, message)?;
    let fetch = match (&body, context.state().validity.get(&message.folder())) {
        // A blocked sender's message is never fetched: it shows what is kept, or that nothing is.
        _ if blocked => None,
        // A message whose move is on its way has no UID of the server's yet; its body comes once it has.
        (None, Some(uid_validity)) if !too_large && message.moving().is_none() => {
            store.folders(message.account())?.into_iter().find(|folder| folder.id() == message.folder()).map(|folder| JobKind::FetchBody {
                folder: folder.id(),
                path: folder.listing().path().clone(),
                uid_validity: *uid_validity,
                uid: message.summary().uid(),
            })
        }
        _ => None,
    };
    let mut shown = json::message(&entry);
    let envelope = message.summary().envelope();
    if let Value::Object(fields) = &mut shown {
        fields.insert("cc".into(), json::addresses(envelope.cc()));
        fields.insert("bcc".into(), json::addresses(envelope.bcc()));
        fields.insert("reply_to".into(), json::addresses(envelope.reply_to()));
        fields.insert("message_id".into(), json!(envelope.message_id().map(MessageRef::as_str)));
        fields.insert("in_reply_to".into(), json!(envelope.in_reply_to().iter().map(MessageRef::as_str).collect::<Vec<_>>()));
        fields.insert("references".into(), json!(envelope.references().iter().map(MessageRef::as_str).collect::<Vec<_>>()));
        fields.insert("body".into(), body.as_ref().map_or(Value::Null, |body| json::body(body.source())));
    }
    let body_error = if too_large {
        Some("too_large")
    } else {
        (blocked && body.is_none()).then_some("not_downloaded")
    };
    let remote_allowed = !blocked && super::remote::allowed(context, envelope.from());
    let mut outcome = Outcome::new(json!({
        "message": shown,
        "fetching": fetch.is_some(),
        "body_error": body_error,
        "remote_allowed": remote_allowed,
        "blocked": blocked,
    }));
    if !entry.opened() {
        outcome = outcome.with_change(Change::MarkOpened(id));
    }
    if let Some(kind) = fetch {
        outcome = outcome.with_fetch(message.account(), kind, id);
    }
    Ok(outcome)
}

/// Whether `message` is from a blocked sender, or in a blocked folder.
fn blocked(context: &Context<'_>, message: &Message) -> Result<bool, EngineError> {
    let role = context.state().accounts.get(&message.account()).and_then(|held| held.roles.role_of(message.folder()));
    if role == Some(Role::Blocked) {
        return Ok(true);
    }
    match screener::sender_of(message) {
        Some(address) => Ok(context.store().sender(address)?.is_some_and(|sender| sender.screening == Some(inboxcraft_model::Screening::Blocked))),
        None => Ok(false),
    }
}

/// `mail.thread`: the thread a message is in, oldest first, across folders and accounts (linked by `Message-ID`,
/// `In-Reply-To` and `References`, never by subject). A message that names no other is a thread of one. A thread of
/// more than [`MAX_LIST`] messages gives its newest. The drafts that answer a message of the thread come with it.
pub(crate) const THREAD: CommandSpec =
    CommandSpec { id: "mail.thread", class: Some(AgentClass::Safe), summary: "List the thread a message is in, oldest first", run: thread };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThreadParams {
    /// The message.
    id: u64,
}

fn thread(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ThreadParams = decode(THREAD.id, params)?;
    let id = MessageId::new(params.id);
    let store = context.store();
    store.message(id)?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let mut entries = BTreeMap::new();
    for account in store.accounts()? {
        for folder in store.folders(account.id())? {
            for entry in store.messages(folder.id())? {
                entries.insert(entry.message().id(), entry);
            }
        }
    }
    let messages: Vec<Message> = entries.values().map(|entry| entry.message().clone()).collect();
    let thread = group_threads(&messages).into_iter().find(|thread| thread.contains(&id)).unwrap_or_else(|| vec![id]);
    let skip = thread.len().saturating_sub(limit(Some(MAX_LIST)));
    let shown: Vec<Value> = thread.iter().skip(skip).filter_map(|id| entries.get(id)).map(json::message).collect();
    let mut drafts = Vec::new();
    for account in store.accounts()? {
        for draft in store.drafts(account.id())? {
            if draft.answers.is_some_and(|answers| thread.contains(&answers)) {
                drafts.push(crate::draft::json(&draft));
            }
        }
    }
    Ok(Outcome::new(json!({ "total": thread.len(), "messages": shown, "drafts": drafts })))
}

/// `mail.attachment`: one attachment of a kept body (by its `index` in `mail.read`'s list) or one inline part (by
/// its `content_id`), its content in base64: what the reader saves or shows as a picture. Nothing is fetched for it.
pub(crate) const ATTACHMENT: CommandSpec = CommandSpec {
    id: "mail.attachment",
    class: Some(AgentClass::Safe),
    summary: "Give one attachment or inline part of a kept message, in base64",
    run: attachment,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentParams {
    /// The message.
    id: u64,
    /// The attachment's place in `mail.read`'s list.
    index: Option<u64>,
    /// The inline part's Content-ID, without the angle brackets.
    content_id: Option<String>,
}

fn attachment(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: AttachmentParams = decode(ATTACHMENT.id, params)?;
    let id = MessageId::new(params.id);
    context.store().message(id)?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let body = context.store().body(id)?.ok_or(EngineError::NotFound { what: "body", id: params.id })?;
    let message = inboxcraft_mime::parse(body.source()).ok();
    let part = match (params.index, params.content_id.as_deref()) {
        (Some(index), None) => message
            .as_ref()
            .and_then(|message| message.attachments().into_iter().nth(usize::try_from(index).unwrap_or(usize::MAX)))
            .ok_or(EngineError::NotFound { what: "attachment", id: index })?,
        (None, Some(content_id)) => message
            .as_ref()
            .and_then(|message| json::inline_parts(message).into_iter().find(|part| part.content_id().as_deref() == Some(content_id)))
            .ok_or(EngineError::NotFound { what: "inline part", id: params.id })?,
        _ => return Err(EngineError::bad_params(ATTACHMENT.id, "give an index or a content_id, one of them")),
    };
    let data = part.decoded().map_err(|error| EngineError::bad_params(ATTACHMENT.id, &format!("the part cannot be decoded: {error}")))?;
    Ok(Outcome::new(json!({
        "name": part.file_name(),
        "type": part.content_type().essence(),
        "size": data.len(),
        "data": json::base64(&data),
    })))
}

/// `mail.search`: the simple offline search over from, to, subject, date and body words, newest first.
pub(crate) const SEARCH: CommandSpec = CommandSpec {
    id: "mail.search",
    class: Some(AgentClass::Safe),
    summary: "Search the kept mail by sender, recipient, subject, date and body words, newest first",
    run: search,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchParams {
    /// Sender names or addresses that contain this.
    from: Option<String>,
    /// To or Cc names or addresses that contain this.
    to: Option<String>,
    /// Subjects that contain this.
    subject: Option<String>,
    /// Bodies with these words in this order.
    text: Option<String>,
    /// Dated at or after this ISO 8601 date or time.
    since: Option<String>,
    /// Dated before this ISO 8601 date or time.
    before: Option<String>,
    /// Only this account.
    account: Option<u64>,
    /// Only this folder.
    folder: Option<u64>,
    /// At most this many, 1 to [`MAX_LIST`].
    limit: Option<u64>,
}

fn date_param(name: &str, text: &str) -> Result<Timestamp, EngineError> {
    parse_date(text).ok_or_else(|| EngineError::bad_params(SEARCH.id, &format!("{name} is not an ISO 8601 date: {text:?}")))
}

fn search(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SearchParams = decode(SEARCH.id, params)?;
    let account = params.account.map(AccountId::new);
    let mut search = Search::new().with_limit(limit(params.limit));
    view::accounts(context, account)?;
    if let Some(account) = account {
        search = search.with_account(account);
    }
    if let Some(folder) = params.folder.map(FolderId::new) {
        view::folders(context, View::Folder(folder), account)?;
        search = search.with_folder(folder);
    }
    if let Some(from) = &params.from {
        search = search.with_from(from);
    }
    if let Some(to) = &params.to {
        search = search.with_to(to);
    }
    if let Some(subject) = &params.subject {
        search = search.with_subject(subject);
    }
    if let Some(text) = &params.text {
        search = search.with_text(text);
    }
    if let Some(since) = &params.since {
        search = search.with_since(date_param("since", since)?);
    }
    if let Some(before) = &params.before {
        search = search.with_before(date_param("before", before)?);
    }
    let mut messages = Vec::new();
    for id in context.store().search(&search)? {
        if let Some(entry) = context.store().message(id)? {
            messages.push(json::message(&entry));
        }
    }
    Ok(Outcome::new(json!({ "total": messages.len(), "messages": messages })))
}
