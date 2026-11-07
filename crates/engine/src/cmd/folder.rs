// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `folder.*`: the folders and the unified views.

use inboxcraft_model::{AccountId, FolderId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::params::decode;
use crate::view::{self, ROLES, View};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, json};

/// `folder.list`: the folders of every account or of one, the unified views by role, and the roles to ask the owner
/// about.
pub(crate) const LIST: CommandSpec = CommandSpec {
    id: "folder.list",
    class: Some(AgentClass::Safe),
    summary: "List the folders with their roles, and the unified views by role",
    run: list,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    /// Only this account.
    account: Option<u64>,
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ListParams = decode(LIST.id, params)?;
    let account = params.account.map(AccountId::new);
    let store = context.store();
    let mut folders = Vec::new();
    let mut ask = Vec::new();
    for account in view::accounts(context, account)? {
        let state = context.state().accounts.get(&account);
        for folder in store.folders(account)? {
            let entries = store.messages(folder.id())?;
            let unread = entries.iter().filter(|entry| !entry.message().summary().flags().is_seen()).count();
            let role = state.and_then(|state| state.roles.role_of(folder.id()));
            let synced = store.folder_state(folder.id())?.is_some();
            folders.push(json::folder(&folder, role, synced, entries.len(), unread));
        }
        for role in state.map(|state| state.ask.as_slice()).unwrap_or_default() {
            ask.push(json!({ "account": account.get(), "role": view::role_name(*role) }));
        }
    }
    let mut views = Vec::new();
    for role in ROLES {
        let ids: Vec<u64> = view::folders(context, View::Role(role), account)?.into_iter().map(FolderId::get).collect();
        if !ids.is_empty() {
            views.push(json!({ "role": view::role_name(role), "folders": ids }));
        }
    }
    Ok(Outcome::new(json!({ "folders": folders, "views": views, "roles_to_ask": ask })))
}
