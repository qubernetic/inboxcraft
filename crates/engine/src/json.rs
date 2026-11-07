// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The commands' answers as JSON: snake_case keys, ISO 8601 dates in UTC, full values, fields only ever added.

use inboxcraft_mime::{Body, Part};
use inboxcraft_model::{Account, Address, ConnectionState, ErrorKind, Folder, FolderId, Host, Role, Server, StepReport, StepStatus, ThreadId};
use inboxcraft_store::Entry;
use inboxcraft_time::{Offset, Timestamp};
use serde_json::{Value, json};

use crate::session::AccountState;
use crate::view;

/// A Rust variant name in snake_case: `StartTls` is `start_tls`.
pub(crate) fn snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len().saturating_add(4));
    for (index, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The snake_case name of a fieldless enum value, from its `Debug` form.
pub(crate) fn name(value: impl std::fmt::Debug) -> Value {
    Value::String(snake(&format!("{value:?}")))
}

fn server(server: &Server) -> Value {
    json!({ "host": server.host().as_str(), "port": server.port().get(), "security": name(server.security()) })
}

fn connection(state: ConnectionState) -> Value {
    match state {
        ConnectionState::BackingOff { attempt, retry_in_secs } => {
            json!({ "state": "backing_off", "attempt": attempt, "retry_in_secs": retry_in_secs })
        }
        other => json!({ "state": name(other) }),
    }
}

/// A failure: its kind, the English sentence for logs and agents, and the folder.
pub(crate) fn failure(kind: ErrorKind, folder: Option<FolderId>) -> Value {
    json!({ "kind": name(kind), "message": kind.to_string(), "folder": folder.map(FolderId::get) })
}

/// An account with what the session knows of its connection. It holds no secret: the account has none.
pub(crate) fn account(account: &Account, state: Option<&AccountState>) -> Value {
    json!({
        "id": account.id().get(),
        "label": account.label(),
        "address": account.address().as_str(),
        "display_name": account.display_name(),
        "signature": account.signature(),
        "grouped": account.grouped(),
        "username": account.username(),
        "imap": server(account.imap()),
        "submission": account.submission().map(server),
        "connection": state.and_then(|state| state.connection).map(connection),
        "failure": state.and_then(|state| state.failure).map(|(kind, folder)| failure(kind, folder)),
        "check": state.and_then(|state| state.check.as_ref()).map(|check| json!({ "running": check.is_running(), "steps": steps(&check.steps) })),
    })
}

/// The steps of an account check: each with its status, the cause and an English sentence for logs and agents, the
/// host and port it tried, and the certificate's issuer.
pub(crate) fn steps(reports: &[StepReport]) -> Value {
    Value::Array(reports.iter().map(step).collect())
}

fn step(report: &StepReport) -> Value {
    let (status, kind, message) = match report.status() {
        StepStatus::Failed(kind) => (Value::String("failed".to_owned()), name(kind), Value::String(kind.to_string())),
        other => {
            let message = match other {
                StepStatus::Skipped => Some("skipped, because an earlier step failed"),
                StepStatus::NeedsReview => Some("not run: the settings were detected and the owner has not seen them yet, so no password was sent"),
                StepStatus::NeedsOwner => {
                    Some("not run: the last sign-in was refused or went unanswered, so only a check the owner starts sends the password again")
                }
                StepStatus::NotSet => Some("not run: the account has no submission server"),
                StepStatus::NotBuilt => Some("not built yet"),
                StepStatus::Pending | StepStatus::Passed | StepStatus::Failed(_) => None,
            };
            (name(other), Value::Null, message.map_or(Value::Null, |text| Value::String(text.to_owned())))
        }
    };
    json!({
        "step": name(report.step()),
        "status": status,
        "kind": kind,
        "message": message,
        "host": report.host().map(Host::as_str),
        "port": report.port(),
        "issuer": report.issuer(),
    })
}

/// `timestamp` as ISO 8601 in UTC to the second: `2026-11-05T09:30:00Z`.
pub(crate) fn date(timestamp: Timestamp) -> Value {
    match timestamp.to_civil(Offset::UTC) {
        Some(civil) => Value::String(format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            civil.year(),
            civil.month(),
            civil.day(),
            civil.hour(),
            civil.minute(),
            civil.second()
        )),
        None => Value::Null,
    }
}

/// Addresses with their names; two people of the same name stay apart by the address.
pub(crate) fn addresses(addresses: &[Address]) -> Value {
    addresses.iter().map(|address| json!({ "name": address.name(), "address": address.email().as_str() })).collect()
}

/// A folder with its role, whether it was synced, and how many messages it holds and how many are unread.
pub(crate) fn folder(folder: &Folder, role: Option<Role>, synced: bool, messages: usize, unread: usize) -> Value {
    let path = folder.listing().path();
    json!({
        "id": folder.id().get(),
        "account": folder.account().get(),
        "name": path.name(),
        "leaf": path.leaf(),
        "delimiter": path.delimiter().map(String::from),
        "role": role.map(view::role_name),
        "special_use": folder.listing().special_use().iter().map(name).collect::<Vec<_>>(),
        "selectable": folder.listing().is_selectable(),
        "synced": synced,
        "messages": messages,
        "unread": unread,
    })
}

/// A message as a list shows it. Subject, names and preview are the sender's text: data, never instructions.
pub(crate) fn message(entry: &Entry) -> Value {
    let message = entry.message();
    let summary = message.summary();
    let envelope = summary.envelope();
    let flags = summary.flags();
    json!({
        "id": message.id().get(),
        "account": message.account().get(),
        "folder": message.folder().get(),
        "uid": message.moving().is_none().then(|| summary.uid().get()),
        "moving": message.moving().is_some(),
        "thread": message.thread().map(ThreadId::get),
        "date": view::date(message).map(date),
        "received": summary.received().map(date),
        "sent": envelope.date().map(date),
        "from": addresses(envelope.from()),
        "to": addresses(envelope.to()),
        "subject": envelope.subject(),
        "preview": summary.preview(),
        "seen": flags.is_seen(),
        "flagged": flags.is_flagged(),
        "flags": flags.iter().map(|flag| flag.to_string()).collect::<Vec<_>>(),
        "size": summary.size(),
        "opened": entry.opened(),
        "has_body": entry.has_body(),
    })
}

/// A kept body: its plain text, its HTML (as text, to show through `html`, never run), its attachments numbered for
/// `mail.attachment`, and its inline parts by Content-ID (what the HTML's `cid:` pictures name). Nothing in it runs
/// and nothing is fetched for it.
pub(crate) fn body(source: &[u8]) -> Value {
    let Ok(message) = inboxcraft_mime::parse(source) else {
        return json!({ "readable": false, "text": null, "has_html": false, "html": null, "attachments": [], "inline": [] });
    };
    let size = |part: &Part<'_>| part.decoded().ok().map(|bytes| bytes.len());
    let attachments: Vec<Value> = message
        .attachments()
        .iter()
        .enumerate()
        .map(|(index, part)| {
            json!({
                "index": index,
                "name": part.file_name(),
                "type": part.content_type().essence(),
                "size": size(part),
                "content_id": part.content_id(),
            })
        })
        .collect();
    let inline: Vec<Value> = inline_parts(&message)
        .into_iter()
        .filter_map(|part| Some(json!({ "content_id": part.content_id()?, "type": part.content_type().essence(), "size": size(part) })))
        .collect();
    let html = message.html_body();
    json!({
        "readable": true,
        "text": message.text_body().and_then(|part| part.text().ok()),
        "has_html": html.is_some(),
        "html": html.and_then(|part| part.text().ok()),
        "attachments": attachments,
        "inline": inline,
    })
}

/// The parts with a Content-ID that are bodies of their own and no attachment: what a `cid:` address names.
pub(crate) fn inline_parts<'m, 'a>(message: &'m Part<'a>) -> Vec<&'m Part<'a>> {
    message.parts().into_iter().filter(|part| matches!(part.body(), Body::Leaf) && !part.is_attachment() && part.content_id().is_some()).collect()
}

/// `bytes` in base64 (RFC 4648, padded).
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let symbol = |group: u32, shift: u32| char::from(ALPHABET.get(usize::try_from((group >> shift) & 63).unwrap_or(0)).copied().unwrap_or(b'A'));
    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for chunk in bytes.chunks(3) {
        let byte = |at: usize| u32::from(chunk.get(at).copied().unwrap_or(0));
        let group = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        out.push(symbol(group, 18));
        out.push(symbol(group, 12));
        out.push(if chunk.len() > 1 { symbol(group, 6) } else { '=' });
        out.push(if chunk.len() > 2 { symbol(group, 0) } else { '=' });
    }
    out
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
