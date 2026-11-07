// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mailbox model: accounts, folders, messages and threads.
//!
//! Layer 1 (see `docs/architecture.md`). No I/O: every type is plain data, serialisable with `serde`, and the same
//! on native and wasm. Our own model is the centre: protocols and formats are translated into it at the edge, so
//! the engine, the store and the front ends never see a protocol type.
//!
//! - Our own ids ([`AccountId`], [`FolderId`], [`MessageId`], [`ThreadId`], [`JobId`], [`DraftId`], [`ScheduleId`],
//!   [`CalendarId`], [`CalendarEventId`], [`AlertId`]) and the server's numbers ([`Uid`], [`UidValidity`], [`ModSeq`],
//!   [`UidSet`]).
//! - [`Account`] with its [`Server`]s: TLS or STARTTLS, no unencrypted choice, no secret.
//! - [`Folder`], its [`FolderPath`] and [`SpecialUse`] attributes; [`Role`]s from SPECIAL-USE first, then English
//!   and Hungarian names ([`detect_role`]), then the owner, asked once ([`RoleMap`]).
//! - [`Message`]: our ids around the server's [`Summary`] ([`Envelope`], [`Flags`], size, preview); [`Address`]es
//!   that are told apart by the address ([`labels`]); [`group_threads`] links replies into threads.
//! - The read path: the [`Job`]s the engine gives a `net` worker and the [`Event`]s it reports back, with one
//!   [`ErrorKind`] per cause. They live here because `net` (layer 3) may not depend on `engine` (layer 4).
//! - The local records the store keeps beside the mail: [`Draft`]s, the outbox ([`Outgoing`]), the schedule
//!   ([`Scheduled`]), [`Sender`]s with the screener's decision and their [`Group`], each message's [`MessageState`]
//!   (done, parked, pinned, snoozed, follow-up), and [`Calendar`]s with their [`CalendarEvent`]s and [`Alert`]s.
//! - The account check: its [`CheckStep`]s in order and the [`StepReport`] of each (host, port, the certificate's
//!   issuer and a [`StepStatus`]).
//!
//! Every text from outside is checked or cleaned on the way in, also when read back from storage: CR, LF, NUL and
//! the other control characters never reach a name, subject, preview, address, flag, host or folder name, and every
//! list from a server has a cap.
//!
//! ```
//! use inboxcraft_model::{
//!     AccountId, Address, EmailAddress, Folder, FolderId, FolderListing, FolderPath, Role, RoleMap, SpecialUse,
//!     labels,
//! };
//!
//! let folder = |id, name: &str, special_use: Vec<SpecialUse>| {
//!     let path = FolderPath::new(name, Some('/'))?;
//!     Ok::<_, inboxcraft_model::FolderError>(Folder::new(FolderId::new(id), AccountId::new(1), FolderListing::new(path, special_use, true)))
//! };
//! let folders = [folder(1, "INBOX", vec![])?, folder(2, "Elküldött", vec![])?, folder(3, "Bin", vec![SpecialUse::Trash])?];
//! let mut roles = RoleMap::default();
//! assert_eq!(roles.detect(&folders), [Role::Drafts, Role::Junk, Role::Archive]); // asked once
//! assert_eq!(roles.folder(Role::Sent), Some(FolderId::new(2)));
//! assert_eq!(roles.folder(Role::Trash), Some(FolderId::new(3)));
//!
//! let anna = Address::new(Some("Anna"), EmailAddress::new("anna@example.org")?);
//! let other_anna = Address::new(Some("Anna"), EmailAddress::new("anna@example.net")?);
//! assert_eq!(labels(&[anna, other_anna]), ["Anna <anna@example.org>", "Anna <anna@example.net>"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod account;
mod address;
mod check;
mod event;
mod flags;
mod folder;
mod id;
mod job;
mod local;
mod message;
mod role;
mod signs;
mod text;
mod thread;
mod uid_set;

pub use account::{Account, AccountError, Host, Security, Server};
pub use address::{Address, AddressError, EmailAddress, MAX_NAME_CHARS, labels};
pub use check::{CheckStep, MAX_ISSUER_CHARS, StepReport, StepStatus};
pub use event::{ConnectionState, Copied, ErrorKind, Event, EventKind, FlagChange};
pub use flags::{Flag, FlagError, Flags, Keyword};
pub use folder::{Folder, FolderError, FolderListing, FolderPath, SpecialUse};
pub use id::{AccountId, AlertId, CalendarEventId, CalendarId, DraftId, FolderId, JobId, MessageId, ModSeq, ScheduleId, ThreadId, Uid, UidValidity};
pub use job::{FolderState, Job, JobKind};
pub use local::{Alert, Calendar, CalendarEvent, Draft, DraftCopy, Due, Group, MessageState, OutboxState, Outgoing, Scheduled, Screening, Sender};
pub use message::{Envelope, MAX_ADDRESSES, MAX_PREVIEW_CHARS, MAX_REFERENCES, MAX_SUBJECT_CHARS, Message, MessageRef, Moving, Summary};
pub use role::{Role, RoleError, RoleMap, RoleSource, detect_role};
pub use signs::{Precedence, Signs};
pub use text::is_invisible_mark;
pub use thread::{Thread, group_threads};
pub use uid_set::UidSet;

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
