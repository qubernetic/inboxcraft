// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The jobs of the read path: what the engine asks an account's `net` worker to do.
//!
//! They live here, not in `engine`, because `net` (layer 3) may not depend on `engine` (layer 4). A job carries
//! everything the worker needs, so the worker never reads the engine's state.

use serde::{Deserialize, Serialize};

use crate::{AccountId, FolderId, FolderPath, JobId, ModSeq, Uid, UidValidity};

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
    /// Watch the folder for changes (IDLE, or NOTIFY where the server has it) until the next job, reporting them as
    /// events.
    Idle {
        /// Our id of the folder.
        folder: FolderId,
        /// Its path on the server.
        path: FolderPath,
    },
}

impl JobKind {
    /// The folder the job is about, if any.
    #[must_use]
    pub fn folder(&self) -> Option<FolderId> {
        match self {
            Self::SyncFolderList => None,
            Self::SyncFolder { folder, .. } | Self::FetchBody { folder, .. } | Self::Idle { folder, .. } => Some(*folder),
        }
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
