// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Quick resynchronisation: ENABLE (RFC 5161), CONDSTORE and QRESYNC (RFC 7162).

use inboxcraft_model::{FolderState, ModSeq, UidSet};

/// An extension that [`Client::enable`](super::Client::enable) switches on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extension {
    /// `CONDSTORE`: modification sequences in every FETCH.
    CondStore,
    /// `QRESYNC`: CONDSTORE, and VANISHED in the place of EXPUNGE.
    QResync,
}

impl Extension {
    /// The extension as IMAP names it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::CondStore => "CONDSTORE",
            Self::QResync => "QRESYNC",
        }
    }
}

/// How SELECT or EXAMINE opens a mailbox.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectOptions {
    /// Ask for modification sequences (`(CONDSTORE)`); refused when the server offers neither CONDSTORE nor QRESYNC.
    pub condstore: bool,
    /// What the last session knew of the mailbox. Its UIDVALIDITY is checked: when the server's differs, an
    /// [`Event::UidValidityChanged`](super::Event::UidValidityChanged) comes before the answer. With QRESYNC enabled and
    /// a modification sequence known, the changes since then are asked for (`(QRESYNC (...))`).
    pub known: Option<FolderState>,
    /// The UIDs the caller holds, sent with QRESYNC so that only those that are gone come back as VANISHED; an empty
    /// set is not sent.
    pub known_uids: Option<UidSet>,
}

/// A modification sequence as IMAP writes one: digits within `u64`, not zero.
pub(crate) fn modseq(text: &str) -> Option<ModSeq> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().and_then(ModSeq::new)
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
