// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Quick resynchronisation: ENABLE (RFC 5161), CONDSTORE and QRESYNC (RFC 7162).

use inboxcraft_model::{FolderState, ModSeq, Uid, UidSet};

use super::value::{Value, number};

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

/// A UID set as a server writes one (`41,43:116`): ranges in any order and either direction, no `*`, no zero.
pub(crate) fn uid_set(text: &str) -> Option<UidSet> {
    let uid = |part: &str| number(part).and_then(Uid::new);
    text.split(',')
        .map(|range| match range.split_once(':') {
            Some((first, last)) => Some((uid(first)?, uid(last)?)),
            None => uid(range).map(|one| (one, one)),
        })
        .collect::<Option<Vec<_>>>()
        .map(UidSet::from)
}

/// `VANISHED [(EARLIER)] <uid set>` (RFC 7162 section 3.2.10): the UIDs, and whether they went before this command
/// (`EARLIER`) rather than just now.
pub(crate) fn vanished(values: Vec<Value>) -> Option<(UidSet, bool)> {
    let mut values = values.into_iter();
    if !values.next()?.atom()?.eq_ignore_ascii_case("VANISHED") {
        return None;
    }
    let (earlier, set) = match values.next()? {
        Value::List(tag) => match tag.as_slice() {
            [Value::Atom(word)] if word.eq_ignore_ascii_case("EARLIER") => (true, values.next()?),
            _ => return None,
        },
        set => (false, set),
    };
    if values.next().is_some() {
        return None;
    }
    Some((uid_set(set.atom()?)?, earlier))
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
