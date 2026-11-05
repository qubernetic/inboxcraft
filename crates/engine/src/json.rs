// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The commands' answers as JSON: snake_case keys, full values, fields only ever added.

use inboxcraft_model::{Account, ConnectionState, ErrorKind, FolderId, Server};
use serde_json::{Value, json};

use crate::session::AccountState;

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

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
