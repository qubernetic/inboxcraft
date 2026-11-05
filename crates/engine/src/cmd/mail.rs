// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `mail.*`: lists of messages.

use inboxcraft_model::{AccountId, FolderId, Role};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::params::decode;
use crate::view::{self, View};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, json};

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
    let messages: Vec<Value> = entries.iter().skip(offset).take(limit).map(json::message).collect();
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
