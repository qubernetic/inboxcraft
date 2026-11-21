// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `mail.*`: lists of messages, one message, the search.

use std::collections::{BTreeMap, BTreeSet};

use inboxcraft_model::{
    Account, AccountId, EmailAddress, FolderId, JobKind, Message, MessageId, MessageRef, MessageState, Role, Screening, is_invisible_mark, links,
    thread_order,
};
use inboxcraft_store::{Change, Entry, Search, Storage};
use inboxcraft_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::arrive::group::{self, Grouping};
use crate::arrive::screener::{self, ScreenerMode};
use crate::params::{decode, parse_date};
use crate::view::{self, View};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, json};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[ATTACHMENT, LIST, READ, ROW, SEARCH, THREAD, WHY];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["account", "folder", "role", "group", "done", "parked", "limit", "offset", "id", "index", "content_id", "from", "to", "subject", "text", "since", "before"];

/// The most messages one answer lists.
pub const MAX_LIST: u64 = 1000;

/// How many a list gives when the caller does not say.
const DEFAULT_LIST: u64 = 100;

/// `mail.list`: the messages of one folder, or of one role across every account (the unified view), newest first.
/// The park pile is left out (counted in `parked`) unless asked for; "show done" (`done`) lists the done mail in the
/// place it was done from, besides the mail there. A message back from a snooze is listed as if it arrived then.
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
    /// Only this group of the grouped inbox (`people`, `updates`, `newsletters`, `invitations`), or `none` for the
    /// mail of the accounts not grouped.
    group: Option<String>,
    /// "Show done": the done mail is listed where it was done from too.
    done: Option<bool>,
    /// The park pile is listed too.
    parked: Option<bool>,
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
    let only = match params.group.as_deref() {
        None => None,
        Some("none") => Some(None),
        Some(name) => Some(Some(group::named(name).ok_or_else(|| EngineError::bad_params(LIST.id, "the group is people, updates, newsletters, invitations or none"))?)),
    };
    let mut grouping = Grouping::new(context.store())?;
    let states: BTreeMap<MessageId, MessageState> = context.store().message_states()?.into_iter().collect();
    let folders = view::folders(context, view, account)?;
    let mut entries = view::merged(context, &folders)?;
    if params.done == Some(true) {
        let listed: BTreeSet<MessageId> = entries.iter().map(|entry| entry.message().id()).collect();
        for (id, state) in &states {
            if state.done.is_some_and(|from| folders.contains(&from))
                && !listed.contains(id)
                && let Some(entry) = context.store().message(*id)?
            {
                entries.push(entry);
            }
        }
    }
    entries.sort_by_cached_key(|entry| view::order_at(entry.message(), states.get(&entry.message().id()).and_then(|state| state.returned)));
    if let Some(only) = only {
        let mut kept = Vec::with_capacity(entries.len());
        for entry in entries {
            if grouping.of(entry.message())? == only {
                kept.push(entry);
            }
        }
        entries = kept;
    }
    let is_parked = |entry: &Entry| states.get(&entry.message().id()).is_some_and(|state| state.parked);
    let parked = entries.iter().filter(|entry| is_parked(entry)).count();
    if params.parked != Some(true) {
        entries.retain(|entry| !is_parked(entry));
    }
    let limit = limit(params.limit);
    let offset = usize::try_from(params.offset.unwrap_or(0)).unwrap_or(usize::MAX);
    let accounts = context.store().accounts()?;
    let mut messages = Vec::new();
    for entry in entries.iter().skip(offset).take(limit) {
        messages.push(row(context, &mut grouping, &accounts, entry, states.get(&entry.message().id()))?);
    }
    let shown = match view {
        View::Folder(folder) => json!({ "folder": folder.get() }),
        View::Role(role) => json!({ "role": view::role_name(role) }),
    };
    Ok(Outcome::new(json!({
        "view": shown,
        "account": account.map(AccountId::get),
        "group": params.group,
        "total": entries.len(),
        "parked": parked,
        "offset": offset,
        "limit": limit,
        "messages": messages,
    })))
}

/// One message as a list shows it: [`json::message`], whether its sender is new, its group and its own `state`.
pub(super) fn row(context: &Context<'_>, grouping: &mut Grouping<'_>, accounts: &[Account], entry: &Entry, state: Option<&MessageState>) -> Result<Value, EngineError> {
    let message = entry.message();
    let role = context.state().accounts.get(&message.account()).and_then(|held| held.roles.role_of(message.folder()));
    let new = screener::is_new(context.store(), context.state().screener, role, accounts, message)?;
    let in_group = grouping.of(message)?.map(group::name);
    let mut shown = json::message(entry);
    if let Value::Object(fields) = &mut shown {
        fields.insert("new_sender".into(), json!(new));
        fields.insert("group".into(), json!(in_group));
        fields.insert("done".into(), json!(state.is_some_and(|state| state.done.is_some())));
        fields.insert("parked".into(), json!(state.is_some_and(|state| state.parked)));
        fields.insert("pinned".into(), json!(state.is_some_and(|state| state.pinned)));
        fields.insert("snoozed_until".into(), state.and_then(|state| state.snoozed_until).map_or(Value::Null, json::date));
        fields.insert("returned".into(), state.and_then(|state| state.returned).map_or(Value::Null, json::date));
        fields.insert("follow_up".into(), state.and_then(|state| state.follow_up).map_or(Value::Null, json::date));
        fields.insert("bounced".into(), bounced(context.store(), message, state)?);
    }
    Ok(shown)
}

/// What a delivery report said of `message`, which the owner sent: the reason, and the report while it is still the
/// report of the same account (message ids are given again once a message is gone), or `null`.
fn bounced(store: &dyn Storage, message: &Message, state: Option<&MessageState>) -> Result<Value, EngineError> {
    let Some(bounce) = state.and_then(|state| state.bounced.as_ref()) else { return Ok(Value::Null) };
    let report = store.message(bounce.report)?.filter(|entry| {
        let held = entry.message();
        held.account() == message.account() && held.summary().envelope().signs().report()
    });
    Ok(json!({ "report": report.map(|_| bounce.report.get()), "reason": bounce.reason }))
}

/// `mail.read`: one message with its body when it is kept, and whether it carries an invitation (`invitation`, whose
/// card `invite.show` reads). Opening a message changes nothing on the server: the
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
        fields.insert("bounced".into(), bounced(store, message, Some(&store.message_state(id)?))?);
        // A calendar part: `invite.show` reads its card.
        fields.insert("invitation".into(), json!(envelope.signs().calendar() || envelope.signs().calendar_type()));
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
    let entry = store.message(id)?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let (thread, entries) = thread_of(store, entry)?;
    let skip = thread.len().saturating_sub(limit(Some(MAX_LIST)));
    let shown: Vec<Value> = thread.iter().skip(skip).filter_map(|id| entries.get(id)).map(json::message).collect();
    let mut drafts = Vec::new();
    for account in store.accounts()? {
        for draft in store.drafts(account.id())? {
            if draft.answers.is_some_and(|answers| thread.contains(&answers)) {
                drafts.push(crate::draft::json(store, &draft)?);
            }
        }
    }
    Ok(Outcome::new(json!({ "total": thread.len(), "messages": shown, "drafts": drafts })))
}

/// The thread `entry`'s message is in, oldest first (a thread of one when it names no other), and its messages by id:
/// the thread [`inboxcraft_model::group_threads`] finds over every message, found from the message by its links
/// through [`Storage::linked`]. Each link is followed once and each message taken once, without recursion, so the
/// work is that of the thread's messages and their links, whatever the store holds besides; a loop of references
/// ends where it began.
fn thread_of(store: &dyn Storage, entry: Entry) -> Result<(Vec<MessageId>, BTreeMap<MessageId, Entry>), EngineError> {
    let mut followed: BTreeSet<MessageRef> = BTreeSet::new();
    let mut to_follow: Vec<MessageRef> = links(entry.message()).cloned().collect();
    let mut entries = BTreeMap::from([(entry.message().id(), entry)]);
    while let Some(link) = to_follow.pop() {
        if followed.contains(&link) {
            continue;
        }
        for found in store.linked(&link)? {
            if let std::collections::btree_map::Entry::Vacant(slot) = entries.entry(found.message().id()) {
                to_follow.extend(links(found.message()).filter(|named| !followed.contains(*named)).cloned());
                slot.insert(found);
            }
        }
        followed.insert(link);
    }
    let mut thread: Vec<&Message> = entries.values().map(Entry::message).collect();
    thread.sort_by_key(|message| thread_order(message));
    let thread = thread.into_iter().map(Message::id).collect();
    Ok((thread, entries))
}

/// `mail.row`: one message as a list shows it (its `new_sender`, `group` and triage state with it), wherever it is,
/// read by its id alone: what a reader in a window of its own reads to keep its head fresh and to learn the message is
/// gone, at the cost of one message whatever the store's size. Nothing changes.
pub(crate) const ROW: CommandSpec =
    CommandSpec { id: "mail.row", class: Some(AgentClass::Safe), summary: "Read one message as a list shows it, by its id", run: row_by_id };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RowParams {
    /// The message.
    id: u64,
}

fn row_by_id(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: RowParams = decode(ROW.id, params)?;
    let id = MessageId::new(params.id);
    let store = context.store();
    let entry = store.message(id)?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let state = store.message_state(id)?;
    let mut grouping = Grouping::new(store)?;
    let accounts = store.accounts()?;
    Ok(Outcome::new(json!({ "message": row(context, &mut grouping, &accounts, &entry, Some(&state))? })))
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

/// `mail.why`: why a message stands where it does, from what the triage already decided and keeps, never from a
/// second look at the message: at most [`MAX_REASONS`], most specific first. Each reason has a `kind` (`snooze`,
/// `rule`, `sender`, `screener`, `thread`, `default`), a `text_id` from a fixed set the front end words in its own
/// language, and `args` with a fixed set of keys per `text_id`: the values the sentence needs, never text of the
/// message. `default` (the folder) comes only when nothing else does.
pub(crate) const WHY: CommandSpec =
    CommandSpec { id: "mail.why", class: Some(AgentClass::Safe), summary: "Say why a message stands where it does, most specific first", run: why };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WhyParams {
    /// The message.
    id: u64,
}

/// Every `text_id` `mail.why` gives: the fixed set the front end words. The reasons take them from here, by name
/// ([`text_id`]).
pub(crate) const TEXT_IDS: [&str; 13] = [
    "why.snooze.returned",
    "why.snooze.until",
    "why.rule.group",
    "why.sender.group",
    "why.sender.accepted",
    "why.sender.blocked",
    "why.screener.held",
    "why.screener.blocked",
    "why.screener.new",
    "why.screener.known",
    "why.thread.yours",
    "why.thread.other",
    "why.default",
];

/// The names of [`TEXT_IDS`], one each: the reasons are built with these.
pub(crate) mod text_id {
    use super::TEXT_IDS;

    /// Back from a snooze (`at`).
    pub(crate) const SNOOZE_RETURNED: &str = TEXT_IDS[0];
    /// Snoozed until (`until`).
    pub(crate) const SNOOZE_UNTIL: &str = TEXT_IDS[1];
    /// The grouped inbox's rule gave the group (`group`, `rule`).
    pub(crate) const RULE_GROUP: &str = TEXT_IDS[2];
    /// The owner chose the sender's group (`address`, `group`).
    pub(crate) const SENDER_GROUP: &str = TEXT_IDS[3];
    /// The owner let the sender in (`address`).
    pub(crate) const SENDER_ACCEPTED: &str = TEXT_IDS[4];
    /// The owner blocked the sender (`address`).
    pub(crate) const SENDER_BLOCKED: &str = TEXT_IDS[5];
    /// Held back by the screener (`address`).
    pub(crate) const SCREENER_HELD: &str = TEXT_IDS[6];
    /// In the blocked folder (`address`).
    pub(crate) const SCREENER_BLOCKED: &str = TEXT_IDS[7];
    /// First mail from a new sender (`address`).
    pub(crate) const SCREENER_NEW: &str = TEXT_IDS[8];
    /// Let in by the screener as a sender the owner knows (`address`).
    pub(crate) const SCREENER_KNOWN: &str = TEXT_IDS[9];
    /// In a thread the owner wrote in (`messages`).
    pub(crate) const THREAD_YOURS: &str = TEXT_IDS[10];
    /// In a thread (`messages`).
    pub(crate) const THREAD_OTHER: &str = TEXT_IDS[11];
    /// The folder alone (`folder`, `role`).
    pub(crate) const DEFAULT: &str = TEXT_IDS[12];
}

/// The most reasons one answer gives.
pub(crate) const MAX_REASONS: usize = 4;

/// The most characters of an address in an answer: an address is at most this many bytes.
pub(crate) const ADDRESS_CAP: usize = EmailAddress::MAX_LEN;

/// The most characters of a name (a group, a rule, a role) in an answer.
pub(crate) const NAME_CAP: usize = 64;

/// `text` with no control characters and no invisible direction or joiner marks, cut at `cap` characters.
pub(crate) fn clean(text: &str, cap: usize) -> String {
    text.chars().filter(|c| !c.is_control() && !is_invisible_mark(*c)).take(cap).collect()
}

/// One reason.
fn reason(kind: &str, text_id: &str, args: Value) -> Value {
    json!({ "kind": kind, "text_id": text_id, "args": args })
}

/// An address for an answer, cleaned and capped; `null` when there is none.
fn address_arg(address: Option<&EmailAddress>) -> Value {
    address.map_or(Value::Null, |address| Value::String(clean(address.as_str(), ADDRESS_CAP)))
}

/// A name for an answer, cleaned and capped.
fn name_arg(name: &str) -> Value {
    Value::String(clean(name, NAME_CAP))
}

fn why(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: WhyParams = decode(WHY.id, params)?;
    let id = MessageId::new(params.id);
    let store = context.store();
    let entry = store.message(id)?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let message = entry.message();
    let state = store.message_state(id)?;
    let role_of = |message: &Message| context.state().accounts.get(&message.account()).and_then(|held| held.roles.role_of(message.folder()));
    let role = role_of(message);
    let accounts = store.accounts()?;
    let from = screener::sender_of(message);
    let screened = from.filter(|address| screener::teaches(&accounts, address));
    let record = match screened {
        Some(address) => store.sender(address)?,
        None => None,
    };
    let mut reasons = Vec::new();

    // Snooze: it waits in the snoozed folder, or it came back and still stands where it came back to. The store keeps
    // the time it came back but not the folder, and a snooze brings mail back to the inbox, so it counts there only:
    // once the owner moves it on, the return no longer says why it is where it is.
    match (role, state.snoozed_until, state.returned) {
        (Some(Role::Snoozed), Some(until), _) => reasons.push(reason("snooze", text_id::SNOOZE_UNTIL, json!({ "until": json::date(until) }))),
        (Some(Role::Inbox), _, Some(at)) => reasons.push(reason("snooze", text_id::SNOOZE_RETURNED, json!({ "at": json::date(at) }))),
        _ => {}
    }

    // The grouped inbox, where grouping applies (the inbox of a grouped account, `of_by` says which account): the
    // classifier's rule, or else the owner's choice for the sender.
    let grouped = if role == Some(Role::Inbox) { group::Grouping::new(store)?.of_by(message)? } else { None };
    match grouped {
        Some((in_group, Some(rule))) => {
            reasons.push(reason("rule", text_id::RULE_GROUP, json!({ "group": name_arg(group::name(in_group)), "rule": name_arg(group::rule_name(rule)) })));
        }
        Some((in_group, None)) => {
            reasons.push(reason("sender", text_id::SENDER_GROUP, json!({ "address": address_arg(from), "group": name_arg(group::name(in_group)) })));
        }
        None => {}
    }

    // The owner's decisions on the sender.
    match record.as_ref().and_then(|record| record.screening) {
        Some(Screening::Accepted) => reasons.push(reason("sender", text_id::SENDER_ACCEPTED, json!({ "address": address_arg(screened) }))),
        Some(Screening::Blocked) => reasons.push(reason("sender", text_id::SENDER_BLOCKED, json!({ "address": address_arg(screened) }))),
        None => {}
    }

    // The screener: held, blocked, marked as new, or let in as known.
    let mode = context.state().screener;
    let screener_said = match role {
        Some(Role::Screener) => Some(text_id::SCREENER_HELD),
        Some(Role::Blocked) => Some(text_id::SCREENER_BLOCKED),
        Some(Role::Inbox) if screener::is_new(store, mode, role, &accounts, message)? => Some(text_id::SCREENER_NEW),
        Some(Role::Inbox)
            if mode != ScreenerMode::Off && record.as_ref().is_some_and(|record| record.known && record.screening.is_none()) =>
        {
            Some(text_id::SCREENER_KNOWN)
        }
        _ => None,
    };
    if let Some(text_id) = screener_said {
        reasons.push(reason("screener", text_id, json!({ "address": address_arg(from) })));
    }

    // The thread: one the owner wrote in, or another.
    let (thread, entries) = thread_of(store, entry.clone())?;
    if thread.len() > 1 {
        let yours = thread.iter().filter(|other| **other != id).filter_map(|other| entries.get(other)).any(|other| role_of(other.message()) == Some(Role::Sent));
        let text_id = if yours { text_id::THREAD_YOURS } else { text_id::THREAD_OTHER };
        reasons.push(reason("thread", text_id, json!({ "messages": thread.len() })));
    }

    if reasons.is_empty() {
        reasons.push(reason("default", text_id::DEFAULT, json!({ "folder": message.folder().get(), "role": role.map(|role| name_arg(&view::role_name(role))) })));
    }
    Ok(Outcome::new(json!({ "reasons": settle(reasons) })))
}

/// Pairs of reasons that say one decision of the owner's twice: the first, more specific, stays and the second goes.
/// A sender blocked is why its mail is in the blocked folder; a group chosen for a sender is chosen by accepting it
/// (`screener.accept` with a group) or for a sender whose mail comes in, so it says the sender is let in.
const SAME_DECISION: [(&str, &str); 2] = [(text_id::SENDER_BLOCKED, text_id::SCREENER_BLOCKED), (text_id::SENDER_GROUP, text_id::SENDER_ACCEPTED)];

/// The reasons an answer gives of `reasons`, most specific first: one reason a decision, then at most
/// [`MAX_REASONS`].
fn settle(mut reasons: Vec<Value>) -> Vec<Value> {
    let said = |text_id: &str, reasons: &[Value]| reasons.iter().any(|reason| reason["text_id"] == text_id);
    let dropped: Vec<&str> = SAME_DECISION.iter().filter(|(stays, _)| said(stays, &reasons)).map(|(_, goes)| *goes).collect();
    reasons.retain(|reason| !dropped.iter().any(|goes| reason["text_id"] == *goes));
    reasons.truncate(MAX_REASONS);
    reasons
}

#[cfg(test)]
#[path = "mail_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "mail_row_tests.rs"]
mod row_tests;
#[cfg(test)]
#[path = "thread_pin_tests.rs"]
mod thread_pin_tests;
