// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The local-first cache: accounts, folders, messages, flags, bodies and sync state, written atomically.
//!
//! Layer 1 (see `docs/architecture.md`). The [`Storage`] trait is the store; two backends implement it:
//!
//! - [`Memory`]: always built, used on wasm and in tests. It keeps nothing past the process.
//! - `Sqlite` (native only, feature `sqlite`, on by default): one SQLite database in WAL mode with
//!   `synchronous=FULL`, a full-text index of the body text (FTS5) and the message sources as files next to it. A
//!   kill at any point loses no committed [`Batch`]; the schema has a version and migrations.
//!
//! Every write is a [`Batch`] of [`Change`]s, applied all together or not at all. Bodies are kept for messages the
//! owner opened and for the last [`StoreConfig::body_days`] days per folder ([`Storage::prune_bodies`],
//! [`Storage::bodies_to_fetch`]). [`Search`] is the simple offline search over from, to, subject, date and body
//! words. The store does no network I/O and parses no mail: the caller hands in a [`Body`] with its text.
//!
//! ```
//! use inboxcraft_model::{Account, AccountId, EmailAddress, Folder, FolderId, FolderListing, FolderPath, Host, Security, Server};
//! use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
//!
//! let imap = Server::new(Host::new("imap.example.org")?, Security::Tls.imap_port(), Security::Tls);
//! let account = Account::new(AccountId::new(1), "Home", EmailAddress::new("anna@example.org")?, "anna", imap)?;
//! let inbox = Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", Some('/'))?, vec![], true));
//!
//! let mut store = Memory::new(StoreConfig::default());
//! store.apply(&Batch::new().with(Change::PutAccount(account)).with(Change::PutFolder(inbox)))?;
//! assert_eq!(store.folders(AccountId::new(1))?.len(), 1);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod body;
mod change;
mod config;
mod entry;
mod error;
mod memory;
mod search;
#[cfg(all(feature = "sqlite", not(target_arch = "wasm32")))]
mod sqlite;

pub use body::{Body, MAX_SOURCE_BYTES, MAX_TEXT_BYTES};
pub use change::{Batch, Change};
pub use config::StoreConfig;
pub use entry::Entry;
pub use error::StoreError;
use inboxcraft_model::{Account, AccountId, Folder, FolderId, FolderState, MessageId};
use inboxcraft_time::Timestamp;
pub use memory::Memory;
pub use search::{MAX_QUERY_CHARS, MAX_QUERY_WORDS, MAX_RESULTS, Search};
#[cfg(all(feature = "sqlite", not(target_arch = "wasm32")))]
pub use sqlite::{DATABASE_FILE, SCHEMA_VERSION, SOURCES_DIR, Sqlite};

/// The local-first store. Reads give accounts, folders and messages in id order, messages of a folder in UID order.
pub trait Storage {
    /// The body setting the store was opened with.
    fn config(&self) -> StoreConfig;

    /// Applies every change of `batch` in order, or none of them.
    ///
    /// # Errors
    ///
    /// The first change that names something the store does not hold, or a file or database failure; the store
    /// is then as it was.
    fn apply(&mut self, batch: &Batch) -> Result<(), StoreError>;

    /// Every account.
    ///
    /// # Errors
    ///
    /// A record that does not read back, or a database failure.
    fn accounts(&self) -> Result<Vec<Account>, StoreError>;

    /// The folders of `account`.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn folders(&self, account: AccountId) -> Result<Vec<Folder>, StoreError>;

    /// Where the next sync of `folder` starts, if it was synced.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn folder_state(&self, folder: FolderId) -> Result<Option<FolderState>, StoreError>;

    /// The messages of `folder`.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn messages(&self, folder: FolderId) -> Result<Vec<Entry>, StoreError>;

    /// One message.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn message(&self, id: MessageId) -> Result<Option<Entry>, StoreError>;

    /// The kept body of a message.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`], or a source file that is gone or does not read back.
    fn body(&self, id: MessageId) -> Result<Option<Body>, StoreError>;

    /// The messages that match `search`, newest first.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn search(&self, search: &Search) -> Result<Vec<MessageId>, StoreError>;

    /// Messages with a kept body, not opened, dated before `cutoff`.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn stale_bodies(&self, cutoff: Timestamp) -> Result<Vec<MessageId>, StoreError>;

    /// Messages of `folder` without a kept body that are opened, undated or dated at or after `cutoff`, in UID order.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn missing_bodies(&self, folder: FolderId, cutoff: Timestamp) -> Result<Vec<MessageId>, StoreError>;

    /// Removes the bodies that are no longer kept at `now`, as one batch; returns how many.
    ///
    /// # Errors
    ///
    /// As [`Storage::apply`].
    fn prune_bodies(&mut self, now: Timestamp) -> Result<usize, StoreError> {
        let stale = self.stale_bodies(self.config().cutoff(now))?;
        let mut batch = Batch::new();
        for id in &stale {
            batch.push(Change::RemoveBody(*id));
        }
        self.apply(&batch)?;
        Ok(stale.len())
    }

    /// The messages of `folder` whose body the store should keep at `now` but does not, in UID order.
    ///
    /// # Errors
    ///
    /// As [`Storage::accounts`].
    fn bodies_to_fetch(&self, folder: FolderId, now: Timestamp) -> Result<Vec<MessageId>, StoreError> {
        self.missing_bodies(folder, self.config().cutoff(now))
    }
}

#[cfg(test)]
mod tests;
