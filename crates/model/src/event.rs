// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The events of the read path: what an account's `net` worker reports back to the engine.
//!
//! The worker turns everything the server says into these, in our own model; the engine applies them and never sees
//! a protocol type. Every UID in an event belongs to the UIDVALIDITY it carries: an event for an older UIDVALIDITY is
//! stale and dropped.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AccountId, Flags, FolderId, FolderListing, FolderState, JobId, ModSeq, StepReport, Summary, Uid, UidSet, UidValidity};

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
            Self::Folders { .. } | Self::CheckStep(_) | Self::Connection(_) => None,
            Self::MessagesAdded { folder, .. }
            | Self::MessagesChanged { folder, .. }
            | Self::MessagesExpunged { folder, .. }
            | Self::UidValidityReset { folder, .. }
            | Self::BodyFetched { folder, .. }
            | Self::FolderSynced { folder, .. } => Some(*folder),
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
