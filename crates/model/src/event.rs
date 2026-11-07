// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The events: what an account's `net` worker reports back to the engine.
//!
//! The worker turns everything the server says into these, in our own model; the engine applies them and never sees
//! a protocol type. Every UID in an event belongs to the UIDVALIDITY it carries: an event for an older UIDVALIDITY is
//! stale and dropped.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    AccountId, DraftId, Flags, FolderId, FolderListing, FolderPath, FolderState, JobId, ModSeq, StepReport, Summary, Uid, UidSet, UidValidity,
};

/// New flags for one message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagChange {
    /// The message.
    pub uid: Uid,
    /// All its flags now.
    pub flags: Flags,
    /// The modification sequence of the change.
    pub modseq: Option<ModSeq>,
}

/// The UIDs moved messages got in the folder they went to (`COPYUID`, RFC 4315).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copied {
    /// The UIDVALIDITY of the folder they went to.
    pub uid_validity: UidValidity,
    /// Each old UID with its new one.
    pub uids: Vec<(Uid, Uid)>,
}

impl Copied {
    /// The new UID of the message that had `old`, if the server named one.
    #[must_use]
    pub fn new_uid(&self, old: Uid) -> Option<Uid> {
        self.uids.iter().find_map(|(from, to)| (*from == old).then_some(*to))
    }
}

/// How an account's connection stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// No network.
    Offline,
    /// Connecting, encrypting and logging in.
    Connecting,
    /// Logged in and running jobs.
    Online,
    /// Logged in and waiting for changes (IDLE or NOTIFY).
    Watching,
    /// The last attempt failed; the next comes after `retry_in_secs` (back-off with a cap).
    BackingOff {
        /// Failed attempts in a row.
        attempt: u32,
        /// Seconds to the next attempt.
        retry_in_secs: u64,
    },
}

/// Why a job or a connection failed: one kind per cause, so the owner is told the cause and not "check your
/// firewall".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    /// The host name does not resolve.
    HostNotFound,
    /// The server refused the connection.
    ConnectionRefused,
    /// The server did not answer in time.
    TimedOut,
    /// The server answers on other ports, but not on this one: the port is likely wrong.
    PortTimedOut,
    /// Port 443 of the server connects, but the mail ports (993, 465, 587) time out: the network blocks mail.
    MailPortsBlocked,
    /// The connection closed in the middle.
    ConnectionDropped,
    /// The certificate chain ends in an issuer the trust store does not hold.
    CertificateUnknownIssuer,
    /// The certificate has expired.
    CertificateExpired,
    /// The certificate is not valid yet.
    CertificateNotYetValid,
    /// The certificate is for another host name.
    CertificateWrongHost,
    /// The certificate is outside its validity at this computer's clock, and that clock is far off.
    ClockFarOff,
    /// Any other TLS failure.
    Tls,
    /// The server does not offer STARTTLS on a connection set to use it; nothing was sent.
    StartTlsMissing,
    /// The server refused the user name or password.
    LoginRefused,
    /// The password went out and the server never answered it; it is not sent again.
    LoginUnanswered,
    /// The server offers no login method we use over TLS.
    NoLoginMethod,
    /// The OS credential store holds no password for the account.
    CredentialMissing,
    /// The server answered `NO` or `BAD` to a command.
    CommandRefused,
    /// The server sent something that is not IMAP.
    Protocol,
    /// A response is larger than we accept.
    TooLarge,
    /// The folder no longer exists on the server.
    FolderGone,
    /// The message no longer exists on the server.
    MessageGone,
    /// The submission server refused the message, its sender or one of its recipients; nothing was sent.
    MessageRefused,
    /// The account has no submission (SMTP) server, so it cannot send.
    NoSubmissionServer,
}

impl ErrorKind {
    /// Whether trying again later can help on its own: passing network trouble. Everything else needs the owner, the
    /// server's administrator or another job.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::HostNotFound | Self::ConnectionRefused | Self::TimedOut | Self::ConnectionDropped)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::HostNotFound => "the server name was not found",
            Self::ConnectionRefused => "the server refused the connection",
            Self::TimedOut => "the server did not answer in time",
            Self::PortTimedOut => "the server answers on other ports but not on this one, so the port is likely wrong",
            Self::MailPortsBlocked => "the network this computer is on blocks the mail ports of the server",
            Self::ConnectionDropped => "the connection closed in the middle",
            Self::CertificateUnknownIssuer => "the server's certificate comes from an issuer this computer does not trust",
            Self::CertificateExpired => "the server's certificate has expired",
            Self::CertificateNotYetValid => "the server's certificate is not valid yet",
            Self::CertificateWrongHost => "the server's certificate is for another server name",
            Self::ClockFarOff => "the clock of this computer is far off, so the certificate cannot be checked",
            Self::Tls => "the encrypted connection failed",
            Self::StartTlsMissing => "the server does not offer STARTTLS, so nothing was sent",
            Self::LoginRefused => "the server refused the user name or password",
            Self::LoginUnanswered => "the server did not answer the sign-in, so the password is not sent again",
            Self::NoLoginMethod => "the server offers no login method this program uses",
            Self::CredentialMissing => "the password for this account is not in the credential store",
            Self::CommandRefused => "the server refused a command",
            Self::Protocol => "the server sent something this program cannot read",
            Self::TooLarge => "the server sent more than this program accepts at once",
            Self::FolderGone => "the folder no longer exists on the server",
            Self::MessageGone => "the message no longer exists on the server",
            Self::MessageRefused => "the mail server refused the message, so it was not sent",
            Self::NoSubmissionServer => "the account has no server for sending mail set",
        })
    }
}

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    /// The account's whole folder list.
    Folders {
        /// Every folder, in the server's order.
        listings: Vec<FolderListing>,
    },
    /// Messages new to us.
    MessagesAdded {
        /// The folder.
        folder: FolderId,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The messages.
        messages: Vec<Summary>,
    },
    /// Flags changed on the server.
    MessagesChanged {
        /// The folder.
        folder: FolderId,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The changes.
        changes: Vec<FlagChange>,
    },
    /// Messages removed on the server (EXPUNGE, or VANISHED with QRESYNC).
    MessagesExpunged {
        /// The folder.
        folder: FolderId,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The UIDs gone.
        uids: UidSet,
    },
    /// The folder's UIDVALIDITY changed: every UID we hold for it is void, and the folder is reloaded.
    UidValidityReset {
        /// The folder.
        folder: FolderId,
        /// The UIDVALIDITY we knew.
        old: Option<UidValidity>,
        /// The new one.
        new: UidValidity,
    },
    /// A whole message, as the server keeps it (RFC 5322 bytes, for `mime`).
    BodyFetched {
        /// The folder.
        folder: FolderId,
        /// The UIDVALIDITY the UID belongs to.
        uid_validity: UidValidity,
        /// The message.
        uid: Uid,
        /// The raw message.
        raw: Vec<u8>,
    },
    /// A folder sync finished; `state` is where the next one starts.
    FolderSynced {
        /// The folder.
        folder: FolderId,
        /// The state now.
        state: FolderState,
    },
    /// One step of an account check ([`crate::JobKind::Check`]); every step of [`crate::CheckStep::ALL`] comes, in
    /// order.
    CheckStep(StepReport),
    /// The connection changed.
    Connection(ConnectionState),
    /// A [`crate::JobKind::StoreFlags`] job is done; the flags it left came before it as
    /// [`EventKind::MessagesChanged`].
    FlagsStored {
        /// The folder.
        folder: FolderId,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The messages of the job.
        uids: UidSet,
    },
    /// A [`crate::JobKind::Move`] job is done.
    Moved {
        /// The folder they were in.
        folder: FolderId,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The messages of the job.
        uids: UidSet,
        /// The folder they went to.
        to: FolderId,
        /// Their UIDs there, when the server gave them; a message of the job missing from them was gone already.
        copied: Option<Copied>,
    },
    /// A [`crate::JobKind::Expunge`] job is done: the messages are gone for good.
    Expunged {
        /// The folder.
        folder: FolderId,
        /// The UIDVALIDITY the UIDs belong to.
        uid_validity: UidValidity,
        /// The messages of the job.
        uids: UidSet,
    },
    /// A [`crate::JobKind::Append`] job is done.
    Appended {
        /// The folder.
        folder: FolderId,
        /// The message's UIDVALIDITY and UID there, when the server gave them (`APPENDUID`, RFC 4315).
        uid: Option<(UidValidity, Uid)>,
    },
    /// A [`crate::JobKind::CreateFolder`] job is done; the folder comes with the next folder list.
    FolderCreated {
        /// Its path on the server.
        path: FolderPath,
    },
    /// A [`crate::JobKind::Send`] job is done: the submission server took the message for every recipient.
    Sent {
        /// The draft it was made from.
        draft: DraftId,
    },
    /// A [`crate::JobKind::Send`] job failed; the message went to nobody.
    SendFailed {
        /// The draft it was made from.
        draft: DraftId,
        /// Why.
        kind: ErrorKind,
        /// The server's refusal, as it gave it (`550 5.1.1 No such user`), when it refused.
        reply: Option<String>,
    },
    /// A job or the connection failed.
    Failed {
        /// Why.
        kind: ErrorKind,
        /// The folder it was about, if any.
        folder: Option<FolderId>,
    },
}

impl EventKind {
    /// The folder the event is about, if any.
    #[must_use]
    pub fn folder(&self) -> Option<FolderId> {
        match self {
            Self::Folders { .. }
            | Self::CheckStep(_)
            | Self::Connection(_)
            | Self::FolderCreated { .. }
            | Self::Sent { .. }
            | Self::SendFailed { .. } => None,
            Self::MessagesAdded { folder, .. }
            | Self::MessagesChanged { folder, .. }
            | Self::MessagesExpunged { folder, .. }
            | Self::UidValidityReset { folder, .. }
            | Self::BodyFetched { folder, .. }
            | Self::FolderSynced { folder, .. }
            | Self::FlagsStored { folder, .. }
            | Self::Moved { folder, .. }
            | Self::Expunged { folder, .. }
            | Self::Appended { folder, .. } => Some(*folder),
            Self::Failed { folder, .. } => *folder,
        }
    }
}

/// An event from the `net` worker of one account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// The account.
    pub account: AccountId,
    /// The job that caused it; `None` for what the server or the network did on its own.
    pub job: Option<JobId>,
    /// What happened.
    pub kind: EventKind,
}

impl Event {
    /// The event `kind` for `account`, caused by `job`.
    #[must_use]
    pub fn new(account: AccountId, job: Option<JobId>, kind: EventKind) -> Self {
        Self { account, job, kind }
    }
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
