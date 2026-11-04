// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mailbox model: accounts, folders, messages and threads.
//!
//! Layer 1 (see `docs/architecture.md`). No I/O.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod account;
mod address;
mod flags;
mod folder;
mod id;
mod role;

pub use account::{Account, AccountError, Host, Security, Server};
pub use address::{Address, AddressError, EmailAddress, MAX_NAME_CHARS, labels};
pub use flags::{Flag, FlagError, Flags, Keyword};
pub use folder::{Folder, FolderError, FolderListing, FolderPath, SpecialUse};
pub use id::{AccountId, FolderId, JobId, MessageId, ModSeq, ThreadId, Uid, UidValidity};
pub use role::{Role, RoleError, RoleMap, RoleSource, detect_role};

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_model::name(), "inboxcraft-model");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
