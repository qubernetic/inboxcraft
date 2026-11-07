// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The jobs: what the engine asks an account's `net` worker to do, to read and to write.
//!
//! They live here, not in `engine`, because `net` (layer 3) may not depend on `engine` (layer 4). A job carries
//! everything the worker needs, so the worker never reads the engine's state.

use serde::{Deserialize, Serialize};

use crate::{AccountId, DraftId, Flags, FolderId, FolderPath, JobId, MessageId, ModSeq, Uid, UidSet, UidValidity};

/// What we know of a folder from the last sync: where a resync starts (CONDSTORE and QRESYNC, RFC 7162).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderState {
    uid_validity: UidValidity,
    uid_next: Option<Uid>,
    highest_modseq: Option<ModSeq>,
}

impl FolderState {
    /// The state of a folder with `uid_validity`, nothing else known.
    #[must_use]
    pub fn new(uid_validity: UidValidity) -> Self {
        Self { uid_validity, uid_next: None, highest_modseq: None }
    }

    /// With the next UID the server will give (`UIDNEXT`).
    #[must_use]
    pub fn with_uid_next(mut self, uid_next: Option<Uid>) -> Self {
        self.uid_next = uid_next;
        self
    }

    /// With the highest modification sequence seen (`HIGHESTMODSEQ`).
    #[must_use]
    pub fn with_highest_modseq(mut self, highest_modseq: Option<ModSeq>) -> Self {
        self.highest_modseq = highest_modseq;
        self
    }

    /// `UIDVALIDITY`.
    #[must_use]
    pub fn uid_validity(&self) -> UidValidity {
        self.uid_validity
    }

    /// `UIDNEXT`.
    #[must_use]
    pub fn uid_next(&self) -> Option<Uid> {
        self.uid_next
    }

    /// `HIGHESTMODSEQ`.
    #[must_use]
    pub fn highest_modseq(&self) -> Option<ModSeq> {
        self.highest_modseq
    }
}

/// What to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    /// List the account's folders; answered with [`crate::EventKind::Folders`].
    SyncFolderList,
    /// Bring one folder up to date from `known` (a full sync when `None`); answered with messages added, changed and
    /// expunged, a UIDVALIDITY reset if there was one, and [`crate::EventKind::FolderSynced`] last.
    SyncFolder {
        /// Our id of the folder.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
        /// The state of the last sync.
        known: Option<FolderState>,
    },
    /// Fetch the whole message `uid`; answered with [`crate::EventKind::BodyFetched`]. A changed UIDVALIDITY means
    /// the message is gone.
    FetchBody {
        /// Our id of the folder.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
        /// The UIDVALIDITY the UID belongs to.
        uid_validity: UidValidity,
        /// The message.
        uid: Uid,
    },
    /// Check the account step by step on a connection of its own, with the settings as saved; answered with one
    /// [`crate::EventKind::CheckStep`] per step.
    Check {
        /// The owner started the check: it signs in also after the last sign-in was refused or went unanswered. A
        /// check an agent started stops before the sign-in then ([`crate::StepStatus::NeedsOwner`]), so an agent
        /// cannot send a refused password over and over.
        by_owner: bool,
    },
    /// Watch the folder for changes between jobs (IDLE where the server has it), reporting them as the events of a
    /// resync, until another `Idle` moves the watch.
    Idle {
        /// Our id of the folder.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
    },
    /// Set and clear flags of messages (`UID STORE`); answered with their flags as the server now has them
    /// ([`crate::EventKind::MessagesChanged`]) and [`crate::EventKind::FlagsStored`] last.
    StoreFlags {
        /// Our id of the folder.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The messages.
        uids: UidSet,
        /// The flags to set.
        add: Flags,
        /// The flags to clear.
        remove: Flags,
    },
    /// Move messages to another folder of the account (`UID MOVE`, or `UID COPY`, `\Deleted` and `UID EXPUNGE` where
    /// the server has no MOVE); answered with [`crate::EventKind::Moved`], which carries their new UIDs where the
    /// server gives them (`COPYUID`).
    Move {
        /// Our id of the folder they are in.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The messages.
        uids: UidSet,
        /// Our id of the folder they go to.
        to: FolderId,
        /// Its path on the server.
        to_path: FolderPath,
    },
    /// Remove messages for good: `\Deleted` on them and `UID EXPUNGE` of these UIDs only, so nothing else marked
    /// deleted goes with them; answered with [`crate::EventKind::Expunged`].
    Expunge {
        /// Our id of the folder.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The messages.
        uids: UidSet,
    },
    /// Add a message to a folder (`APPEND`); answered with [`crate::EventKind::Appended`].
    Append {
        /// Our id of the folder.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
        /// The flags it gets.
        flags: Flags,
        /// The message (RFC 5322 bytes).
        raw: Vec<u8>,
        /// The draft it is a copy of, whose earlier copy the answer replaces.
        #[serde(default)]
        draft: Option<DraftId>,
        /// The message it is a new copy of (a snooze writes its time into one): the answer gives the message the
        /// copy's place, and the old copy goes.
        #[serde(default)]
        message: Option<MessageId>,
    },
    /// Create a folder (`CREATE`); answered with [`crate::EventKind::FolderCreated`].
    CreateFolder {
        /// Its path on the server.
        path: FolderPath,
    },
}

impl JobKind {
    /// The folder the job is about, if any.
    #[must_use]
    pub fn folder(&self) -> Option<FolderId> {
        match self {
            Self::SyncFolderList | Self::Check { .. } | Self::CreateFolder { .. } => None,
            Self::SyncFolder { folder, .. }
            | Self::FetchBody { folder, .. }
            | Self::Idle { folder, .. }
            | Self::StoreFlags { folder, .. }
            | Self::Move { folder, .. }
            | Self::Expunge { folder, .. }
            | Self::Append { folder, .. } => Some(*folder),
        }
    }

    /// Whether the job changes something on the server: an act of the owner, which must not be lost.
    #[must_use]
    pub fn writes(&self) -> bool {
        matches!(self, Self::StoreFlags { .. } | Self::Move { .. } | Self::Expunge { .. } | Self::Append { .. } | Self::CreateFolder { .. })
    }
}

/// A job for the `net` worker of one account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// Our id; the events the job causes carry it back.
    pub id: JobId,
    /// The account whose worker runs it.
    pub account: AccountId,
    /// What to do.
    pub kind: JobKind,
}

impl Job {
    /// The job `id` for `account`.
    #[must_use]
    pub fn new(id: JobId, account: AccountId, kind: JobKind) -> Self {
        Self { id, account, kind }
    }
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
