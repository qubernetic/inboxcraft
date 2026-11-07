// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a write is made of: [`Change`]s, applied together as one [`Batch`].

use inboxcraft_model::{Account, AccountId, Flags, Folder, FolderId, FolderState, Message, MessageId};

use crate::Body;

/// One change to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Adds an account or replaces the one with its id.
    PutAccount(Account),
    /// Removes an account with its folders, messages and bodies; nothing when it is not there.
    RemoveAccount(AccountId),
    /// Adds a folder of a held account or replaces the one with its id (of the same account).
    PutFolder(Folder),
    /// Removes a folder with its messages and bodies; nothing when it is not there.
    RemoveFolder(FolderId),
    /// Records where the next sync of a held folder starts (UIDVALIDITY, UIDNEXT, HIGHESTMODSEQ).
    SetFolderState(FolderId, FolderState),
    /// Adds a message to a held folder or replaces the one with its id; a replaced message keeps its body and
    /// whether it was opened.
    PutMessage(Message),
    /// Sets the flags of a held message.
    SetFlags(MessageId, Flags),
    /// Removes a message and its body; nothing when it is not there.
    RemoveMessage(MessageId),
    /// Keeps the body of a held message, replacing an older one.
    PutBody(MessageId, Body),
    /// Removes the body of a message; nothing when there is none.
    RemoveBody(MessageId),
    /// Marks a held message as opened by the owner, so its body is kept however old it is.
    MarkOpened(MessageId),
}

/// Changes applied all together or not at all.
///
/// ```
/// use inboxcraft_model::{AccountId, MessageId};
/// use inboxcraft_store::{Batch, Change};
///
/// let batch = Batch::new().with(Change::RemoveMessage(MessageId::new(7))).with(Change::RemoveAccount(AccountId::new(1)));
/// assert_eq!(batch.len(), 2);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    changes: Vec<Change>,
}

impl Batch {
    /// An empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The batch with `change` added at the end.
    #[must_use]
    pub fn with(mut self, change: Change) -> Self {
        self.changes.push(change);
        self
    }

    /// Adds `change` at the end.
    pub fn push(&mut self, change: Change) {
        self.changes.push(change);
    }

    /// The changes, in the order they are applied.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// How many changes the batch holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Whether the batch holds no change.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}
