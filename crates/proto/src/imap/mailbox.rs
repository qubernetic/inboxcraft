// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What SELECT and EXAMINE report about a mailbox (RFC 9051 section 6.3.2), and flags.

use inboxcraft_model::{ModSeq, Uid, UidValidity};

use super::capability::is_atom_char;
use super::sync::modseq;
use super::value::{StatusLine, Value, number};

/// Most flags kept for one message or mailbox.
pub const MAX_FLAGS: usize = 64;
/// Longest flag kept, in bytes.
pub const MAX_FLAG_LEN: usize = 128;

/// The mailbox SELECT or EXAMINE opened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailboxStatus {
    /// How many messages it holds (`EXISTS`).
    pub exists: u32,
    /// How many are new since the last session (`RECENT`, IMAP4rev1 only).
    pub recent: u32,
    /// The flags its messages can have (`FLAGS`).
    pub flags: Vec<String>,
    /// The flags that are kept for good (`PERMANENTFLAGS`); `\*` means new keywords can be made.
    pub permanent_flags: Vec<String>,
    /// Its UIDVALIDITY; without one, no UID can be kept across sessions.
    pub uid_validity: Option<UidValidity>,
    /// The UID the next message will get.
    pub uid_next: Option<Uid>,
    /// The sequence number of the first unseen message (IMAP4rev1 `UNSEEN`).
    pub unseen: Option<u32>,
    /// The highest modification sequence (RFC 7162), when the server keeps them.
    pub highest_modseq: Option<ModSeq>,
    /// Whether it is open read-only: EXAMINE, or a SELECT the server answered with `READ-ONLY`.
    pub read_only: bool,
}

impl MailboxStatus {
    /// Takes in a response code of an untagged OK during SELECT or EXAMINE.
    pub(crate) fn note_code(&mut self, line: &StatusLine) {
        match line.code.as_str() {
            "UIDVALIDITY" => self.uid_validity = number(&line.arg).and_then(UidValidity::new),
            "UIDNEXT" => self.uid_next = number(&line.arg).and_then(Uid::new),
            "UNSEEN" => self.unseen = number(&line.arg),
            "HIGHESTMODSEQ" => self.highest_modseq = modseq(&line.arg),
            "PERMANENTFLAGS" => self.permanent_flags = flag_words(line.arg.trim_start_matches('(').trim_end_matches(')')),
            _ => {}
        }
    }
}

/// Whether `flag` is one: `\Seen`, `$Forwarded`, `\*` (PERMANENTFLAGS), at most [`MAX_FLAG_LEN`] bytes.
fn is_flag(flag: &str) -> bool {
    let word = flag.strip_prefix('\\').unwrap_or(flag);
    flag.len() <= MAX_FLAG_LEN && (word == "*" || (!word.is_empty() && word.bytes().all(is_atom_char)))
}

/// The flags of a list from the server, at most [`MAX_FLAGS`]; what is no flag is left out.
pub(crate) fn flag_list(values: Vec<Value>) -> Vec<String> {
    values.into_iter().filter_map(|value| value.atom().map(str::to_owned)).filter(|flag| is_flag(flag)).take(MAX_FLAGS).collect()
}

/// The flags in a text such as `\Seen \Deleted \*`, as [`flag_list`] keeps them.
pub(crate) fn flag_words(text: &str) -> Vec<String> {
    text.split_ascii_whitespace().filter(|flag| is_flag(flag)).take(MAX_FLAGS).map(str::to_owned).collect()
}

#[cfg(test)]
#[path = "mailbox_tests.rs"]
mod tests;
