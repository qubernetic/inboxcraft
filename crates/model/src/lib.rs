// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mailbox model: accounts, folders, messages and threads.
//!
//! Layer 1 (see `docs/architecture.md`). No I/O.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod id;

pub use id::{AccountId, FolderId, JobId, MessageId, ModSeq, ThreadId, Uid, UidValidity};

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
