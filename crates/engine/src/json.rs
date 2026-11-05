// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The commands' answers as JSON: snake_case keys, ISO 8601 dates in UTC, full values, fields only ever added.

use inboxcraft_model::{Account, Address, ConnectionState, ErrorKind, Folder, FolderId, Role, Server, ThreadId};
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
        "username": account.username(),
        "imap": server(account.imap()),
        "submission": account.submission().map(server),
        "connection": state.and_then(|state| state.connection).map(connection),
        "failure": state.and_then(|state| state.failure).map(|(kind, folder)| failure(kind, folder)),
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
        "uid": summary.uid().get(),
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

/// A kept body: its plain text, whether it has an HTML part (shown through `html`, never run) and its attachments.
/// Nothing in it runs and nothing is fetched for it.
pub(crate) fn body(source: &[u8]) -> Value {
    let Ok(message) = inboxcraft_mime::parse(source) else {
        return json!({ "readable": false, "text": null, "has_html": false, "attachments": [] });
    };
    let attachments: Vec<Value> = message
        .attachments()
        .iter()
        .map(|part| {
            json!({
                "name": part.file_name(),
                "type": part.content_type().essence(),
                "size": part.decoded().ok().map(|bytes| bytes.len()),
            })
        })
        .collect();
    json!({
        "readable": true,
        "text": message.text_body().and_then(|part| part.text().ok()),
        "has_html": message.html_body().is_some(),
        "attachments": attachments,
    })
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
