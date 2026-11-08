// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Folder views: one folder, or the unified view of one role across every account, narrowed to one account if asked,
//! merged by date.

use std::cmp::Reverse;

use inboxcraft_model::{AccountId, FolderId, Message, MessageId, Role};
use inboxcraft_store::Entry;
use inboxcraft_time::Timestamp;

use crate::{Context, EngineError};

/// Every role, in the order views are listed.
pub(crate) const ROLES: [Role; 11] = [
    Role::Inbox,
    Role::Drafts,
    Role::Sent,
    Role::Archive,
    Role::Junk,
    Role::Trash,
    Role::All,
    Role::Flagged,
    Role::Screener,
    Role::Snoozed,
    Role::Blocked,
];

/// The role named `name` in snake_case (`inbox`, `sent`).
pub(crate) fn role(name: &str) -> Option<Role> {
    ROLES.into_iter().find(|role| role.label().eq_ignore_ascii_case(name))
}

/// The role's name as commands write it.
pub(crate) fn role_name(role: Role) -> String {
    role.label().to_ascii_lowercase()
}

/// What a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    /// One folder.
    Folder(FolderId),
    /// The folders with this role, across the accounts.
    Role(Role),
}

/// The accounts a view looks at: `account` alone, or every account.
pub(crate) fn accounts(context: &Context<'_>, account: Option<AccountId>) -> Result<Vec<AccountId>, EngineError> {
    let held: Vec<AccountId> = context.store().accounts()?.iter().map(inboxcraft_model::Account::id).collect();
    match account {
        Some(account) if held.contains(&account) => Ok(vec![account]),
        Some(account) => Err(EngineError::NotFound { what: "account", id: account.get() }),
        None => Ok(held),
    }
}

/// The folders `view` shows, narrowed to `account`.
///
/// # Errors
///
/// [`EngineError::NotFound`] for an account or folder the store does not hold, or a folder of another account than
/// `account`.
pub(crate) fn folders(context: &Context<'_>, view: View, account: Option<AccountId>) -> Result<Vec<FolderId>, EngineError> {
    let accounts = accounts(context, account)?;
    let mut folders = Vec::new();
    for account in accounts {
        match view {
            View::Folder(folder) => {
                if context.store().folders(account)?.iter().any(|held| held.id() == folder) {
                    folders.push(folder);
                }
            }
            View::Role(role) => {
                if let Some(folder) = context.state().accounts.get(&account).and_then(|state| state.roles.folder(role)) {
                    folders.push(folder);
                }
            }
        }
    }
    match view {
        View::Folder(folder) if folders.is_empty() => Err(EngineError::NotFound { what: "folder", id: folder.get() }),
        _ => Ok(folders),
    }
}

/// The date a list sorts by: when the server received the message, else its `Date:` header.
pub(crate) fn date(message: &Message) -> Option<Timestamp> {
    message.summary().received().or(message.summary().envelope().date())
}

/// The order of a list: newest first, undated last, then the higher id first.
pub(crate) fn order(message: &Message) -> (bool, Reverse<Option<Timestamp>>, Reverse<MessageId>) {
    order_at(message, None)
}

/// As [`order`], for a message that came back from a snooze at `returned`: listed as if it arrived then.
pub(crate) fn order_at(message: &Message, returned: Option<Timestamp>) -> (bool, Reverse<Option<Timestamp>>, Reverse<MessageId>) {
    let date = date(message).max(returned);
    (date.is_none(), Reverse(date), Reverse(message.id()))
}

/// The messages of `folders`, merged by date.
pub(crate) fn merged(context: &Context<'_>, folders: &[FolderId]) -> Result<Vec<Entry>, EngineError> {
    let mut entries = Vec::new();
    for folder in folders {
        entries.extend(context.store().messages(*folder)?);
    }
    entries.sort_by_cached_key(|entry| order(entry.message()));
    Ok(entries)
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
