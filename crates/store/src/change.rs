// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a write is made of: [`Change`]s, applied together as one [`Batch`].

use inboxcraft_model::{
    Account, AccountId, Alert, AlertId, Calendar, CalendarEvent, CalendarEventId, CalendarId, Draft, DraftId, EmailAddress, Flags, Folder, FolderId,
    FolderState, Job, JobId, Message, MessageId, MessageState, Outgoing, ScheduleId, Scheduled, Sender,
};

use crate::Body;

/// One change to the store.
///
/// A removal takes what hangs below: an account its folders, messages, queued jobs, drafts and calendars; a folder its
/// messages; a message its body and state; a draft its outbox item; a calendar its events; an event its alerts.
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
    /// Queues a job of a held account for its `net` worker, at the end; a job queued again keeps its place.
    QueueJob(Job),
    /// Takes a job off the queue; nothing when it is not there.
    RemoveJob(JobId),
    /// Adds a draft of a held account or replaces the one with its id, also with another account.
    PutDraft(Draft),
    /// Removes a draft and its outbox item; nothing when it is not there.
    RemoveDraft(DraftId),
    /// Puts a held draft in the outbox or changes where it stands.
    PutOutgoing(Outgoing),
    /// Takes a draft out of the outbox; nothing when it is not there.
    RemoveOutgoing(DraftId),
    /// Adds a schedule entry or replaces the one with its id.
    PutScheduled(Scheduled),
    /// Removes a schedule entry; nothing when it is not there.
    RemoveScheduled(ScheduleId),
    /// Adds what we know of a sender or replaces it; one record per address, told apart without case.
    PutSender(Sender),
    /// Forgets a sender; nothing when it is not there.
    RemoveSender(EmailAddress),
    /// Sets the owner's own state of a held message; the empty state removes it.
    SetMessageState(MessageId, MessageState),
    /// Adds a calendar of a held account or replaces the one with its id; a replaced calendar keeps its sync token.
    PutCalendar(Calendar),
    /// Removes a calendar with its events and alerts; nothing when it is not there.
    RemoveCalendar(CalendarId),
    /// Records where the next sync of a held calendar starts (its WebDAV sync token), or that it starts afresh.
    SetSyncToken(CalendarId, Option<String>),
    /// Adds an event to a held calendar or replaces the one with its id.
    PutCalendarEvent(CalendarEvent),
    /// Removes an event with its alerts; nothing when it is not there.
    RemoveCalendarEvent(CalendarEventId),
    /// Adds an alert to a held event or replaces the one with its id.
    PutAlert(Alert),
    /// Removes an alert; nothing when it is not there.
    RemoveAlert(AlertId),
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
