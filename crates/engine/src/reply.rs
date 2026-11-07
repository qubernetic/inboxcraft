// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Replies and forwards (`compose.reply`, `compose.reply_all`, `compose.forward`): a new draft made from a message.
//!
//! - A reply goes to the message's `Reply-To`, else its `From`; a reply to the owner's own message goes to its `To`.
//!   Reply all adds the other `To` and `Cc` recipients, each address once, and none of the owner's own: the address
//!   and the login (where it is an address) of each account. Bcc is never read. The draft is from the account that holds the message.
//! - The subject gets `Re: ` (a reply) or `Fwd: ` (a forward) unless it starts with one already (`Re:`, `Fw:` or
//!   `Fwd:`, any case).
//! - A reply carries `In-Reply-To` (the message's `Message-ID`) and `References` (the message's `References`, else
//!   its `In-Reply-To`, then its `Message-ID`), so it joins the message's thread here and at the recipients. Only ids
//!   the message builder can write are kept, and of a long thread the first id and the newest ones.
//! - A reply quotes the message's plain text (its preview where the body is not kept or has no plain text) below a
//!   line naming who wrote and when; a forward puts the text inline below the original's sender, date, subject and
//!   recipients, and its draft names the message, whose attachments go with it when it is sent. A forward needs the
//!   body kept, so its attachments are known.
//!
//! The message is the sender's data: its text is only quoted, never followed.

use std::collections::BTreeSet;

use inboxcraft_mime::{Attachment, is_media_type, is_message_id};
use inboxcraft_model::{Account, Address, Draft, EmailAddress, Envelope, Message, MessageRef};
use inboxcraft_store::Storage;
use inboxcraft_time::Timestamp;
use serde_json::{Value, json};

use crate::draft::{self, MAX_SUBJECT_BYTES, MAX_TEXT_BYTES};
use crate::{Context, EngineError};

/// What kind of draft is made from a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A reply to the sender.
    Reply,
    /// A reply to the sender and every other recipient.
    ReplyAll,
    /// A forward, to no one yet.
    Forward,
}

/// The most ids a reply keeps in `References`: the first of the thread and the newest.
const MAX_REFERENCES: usize = 20;

/// The most bytes of the message's text a draft quotes; with the quote marks it stays within the longest text.
const MAX_QUOTED_BYTES: usize = MAX_TEXT_BYTES / 4;

/// The months, as the quote line writes them.
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The draft `kind` makes from `message`, from `account` (the one holding it), and for a forward the attachments that
/// go with it, as the answer lists them.
pub(crate) fn make(context: &Context<'_>, account: &Account, message: &Message, kind: Kind) -> Result<(Draft, Value), EngineError> {
    let store = context.store();
    let envelope = message.summary().envelope();
    let source = store.body(message.id())?.map(|body| body.source().to_vec());
    let parsed = source.as_deref().and_then(|source| inboxcraft_mime::parse(source).ok());
    let text = parsed.as_ref().and_then(|part| part.text_body()).and_then(|part| part.text().ok()).map(|text| text.replace("\r\n", "\n"));
    let text = text.or_else(|| message.summary().preview().map(str::to_owned)).unwrap_or_default();
    let text = cut(text.trim_end_matches(['\n', ' ', '\t']), MAX_QUOTED_BYTES);
    let own = own_addresses(store)?;
    let mut draft = Draft { answers: Some(message.id()), ..Draft::new(draft::next_id(context.state()), account.id(), context.now()) };
    let mut listed = Vec::new();
    let date = envelope.date().and_then(|date| written(context, date));
    match kind {
        Kind::Reply | Kind::ReplyAll => {
            let (to, cc) = recipients(envelope, &own, kind == Kind::ReplyAll);
            draft.to = to;
            draft.cc = cc;
            draft.subject = prefixed(envelope.subject(), "Re: ", &["re:"]);
            draft.in_reply_to = envelope.message_id().filter(|id| is_message_id(id.as_str())).cloned();
            draft.references = references(envelope);
            let who = envelope.from().first().map_or_else(|| "Someone".to_owned(), typed);
            let when = date.map(|date| format!(" on {date}")).unwrap_or_default();
            draft.text = format!("\n\n{who} wrote{when}:\n{}{}", quoted(text), draft::signed(account));
        }
        Kind::Forward => {
            let source = source.ok_or(EngineError::NotFound { what: "body", id: message.id().get() })?;
            let attachments = attachments(&source);
            listed = attachments.iter().map(|(name, kind, data)| json!({ "name": name, "type": kind, "size": data.len() })).collect();
            draft.forwards = Some(message.id());
            draft.subject = prefixed(envelope.subject(), "Fwd: ", &["fwd:", "fw:"]);
            let mut header = String::from("\n\nForwarded message\n");
            header.push_str(&field("From", envelope.from()));
            if let Some(date) = date {
                header.push_str(&format!("Date: {date}\n"));
            }
            header.push_str(&format!("Subject: {}\n", envelope.subject()));
            header.push_str(&field("To", envelope.to()));
            header.push_str(&field("Cc", envelope.cc()));
            draft.text = format!("{header}\n{text}{}", draft::signed(account));
        }
    }
    Ok((draft, Value::Array(listed)))
}

/// The attachments `draft` forwards, ready for the builder: none when it forwards nothing, `None` when the body of
/// the message it forwards is no longer kept.
pub(crate) fn forwarded(store: &dyn Storage, draft: &Draft) -> Result<Option<Vec<Attachment>>, EngineError> {
    let Some(message) = draft.forwards else { return Ok(Some(Vec::new())) };
    Ok(store
        .body(message)?
        .map(|body| attachments(body.source()).into_iter().map(|(name, kind, data)| Attachment::new(&name, &kind, data)).collect()))
}

/// The attachments of the message `source`: each one's file name, the media type it goes out with (its own where the
/// builder takes it, else `application/octet-stream`) and its decoded bytes (the bytes as they are where they cannot
/// be decoded). None for a source that is no message.
fn attachments(source: &[u8]) -> Vec<(String, String, Vec<u8>)> {
    let Ok(message) = inboxcraft_mime::parse(source) else { return Vec::new() };
    message
        .attachments()
        .into_iter()
        .map(|part| {
            let essence = part.content_type().essence();
            let kind = if is_media_type(&essence) { essence.to_ascii_lowercase() } else { "application/octet-stream".to_owned() };
            let data = part.decoded().map_or_else(|_| part.raw_body().to_vec(), std::borrow::Cow::into_owned);
            (part.file_name().unwrap_or_default(), kind, data)
        })
        .collect()
}

/// The address of each of the owner's accounts and its login where that is an address, lower case.
fn own_addresses(store: &dyn Storage) -> Result<BTreeSet<String>, EngineError> {
    let mut own = BTreeSet::new();
    for account in store.accounts()? {
        own.insert(account.address().as_str().to_lowercase());
        if let Ok(login) = EmailAddress::new(account.username()) {
            own.insert(login.as_str().to_lowercase());
        }
    }
    Ok(own)
}

/// `To` and `Cc` of a reply to `envelope`, as typed.
fn recipients(envelope: &Envelope, own: &BTreeSet<String>, all: bool) -> (Vec<String>, Vec<String>) {
    let is_own = |address: &Address| own.contains(&address.email().as_str().to_lowercase());
    let from_owner = !envelope.from().is_empty() && envelope.from().iter().all(is_own);
    let target: &[Address] = if from_owner {
        envelope.to()
    } else if envelope.reply_to().is_empty() {
        envelope.from()
    } else {
        envelope.reply_to()
    };
    let mut seen = BTreeSet::new();
    let mut take = |list: &[Address], skip_own: bool| -> Vec<String> {
        list.iter().filter(|address| !(skip_own && is_own(address)) && seen.insert(address.email().as_str().to_lowercase())).map(typed).collect()
    };
    if !all {
        return (take(target, false), Vec::new());
    }
    let mut to = take(target, true);
    if !from_owner {
        to.extend(take(envelope.to(), true));
    }
    let cc = take(envelope.cc(), true);
    (to, cc)
}

/// An address as a recipient is typed: `Name <address>`, the name without the characters that would end it.
fn typed(address: &Address) -> String {
    let name: String = address.name().unwrap_or_default().chars().filter(|c| !matches!(c, '<' | '>' | '"' | ',' | ';')).collect();
    let name = name.trim();
    if name.is_empty() { address.email().as_str().to_owned() } else { format!("{name} <{}>", address.email().as_str()) }
}

/// A line `Name: a, b` of a forward's header, or nothing for an empty list.
fn field(name: &str, list: &[Address]) -> String {
    if list.is_empty() {
        return String::new();
    }
    format!("{name}: {}\n", list.iter().map(typed).collect::<Vec<_>>().join(", "))
}

/// `subject` with `prefix` in front, unless it starts with one of `known` (lower case) already; cut to the longest
/// subject.
fn prefixed(subject: &str, prefix: &str, known: &[&str]) -> String {
    let subject = subject.trim();
    let lower = subject.to_lowercase();
    let whole = if known.iter().any(|known| lower.starts_with(known)) { subject.to_owned() } else { format!("{prefix}{subject}") };
    cut(&whole, MAX_SUBJECT_BYTES)
}

/// `References` of a reply to `envelope`: its references (else its `In-Reply-To`) and its `Message-ID`, only ids the
/// builder can write, the first and the newest of a long thread.
fn references(envelope: &Envelope) -> Vec<MessageRef> {
    let before = if envelope.references().is_empty() { envelope.in_reply_to() } else { envelope.references() };
    let mut ids: Vec<MessageRef> = before.iter().chain(envelope.message_id()).filter(|id| is_message_id(id.as_str())).cloned().collect();
    if ids.len() > MAX_REFERENCES {
        ids.drain(1..ids.len().saturating_sub(MAX_REFERENCES.saturating_sub(1)));
    }
    ids
}

/// `text` with each line quoted: `> `, or `>` before an empty line or one quoted already.
fn quoted(text: String) -> String {
    text.split('\n')
        .map(|line| if line.is_empty() || line.starts_with('>') { format!(">{line}") } else { format!("> {line}") })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `at` on the owner's wall clock: `5 Nov 2026 at 08:00`.
fn written(context: &Context<'_>, at: Timestamp) -> Option<String> {
    let local = at.to_civil(context.offsets().offset_at(at))?;
    let month = MONTHS.get(usize::from(local.month()).saturating_sub(1))?;
    Some(format!("{} {month} {} at {:02}:{:02}", local.day(), local.year(), local.hour(), local.minute()))
}

/// `text` cut to at most `max` bytes, at a character boundary.
fn cut(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text.get(..end).unwrap_or_default().to_owned()
}
