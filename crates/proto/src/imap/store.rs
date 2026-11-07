// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UID STORE (RFC 9051 section 6.4.6), with UNCHANGEDSINCE and MODIFIED (CONDSTORE, RFC 7162 section 3.1.3).

use inboxcraft_model::{Flags, ModSeq, UidSet};

use super::command::Command;
use super::search::set_text;
use super::uidplus::ranges;
use crate::ProtoError;

/// How a [`FlagStore`] changes the flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreMode {
    /// `+FLAGS`: sets these, keeps the others.
    Add,
    /// `-FLAGS`: clears these, keeps the others.
    Remove,
    /// `FLAGS`: these and no other.
    Replace,
}

/// What a [`crate::imap::Client::uid_store`] does to the messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagStore {
    /// Add, remove or replace.
    pub mode: StoreMode,
    /// The flags; no flag with [`StoreMode::Replace`] clears every one.
    pub flags: Flags,
    /// `.SILENT`: the server does not tell the flags the messages have now (it may still tell of changes made by
    /// others).
    pub silent: bool,
    /// `UNCHANGEDSINCE`: only messages whose modification sequence is not above this one change; the server names the
    /// others in [`crate::imap::Answer::Stored`].
    pub unchanged_since: Option<ModSeq>,
}

impl FlagStore {
    /// `+FLAGS`, `-FLAGS` or `FLAGS` of `flags`, telling the flags after it, unconditional.
    #[must_use]
    pub fn new(mode: StoreMode, flags: Flags) -> Self {
        Self { mode, flags, silent: false, unchanged_since: None }
    }

    /// The command for the messages in `set`.
    pub(crate) fn command(&self, set: &UidSet) -> Result<Command, ProtoError> {
        let mut command = Command::new("UID STORE").arg(&set_text(set)?);
        if let Some(modseq) = self.unchanged_since {
            command = command.arg(&format!("(UNCHANGEDSINCE {modseq})"));
        }
        let mode = match self.mode {
            StoreMode::Add => "+FLAGS",
            StoreMode::Remove => "-FLAGS",
            StoreMode::Replace => "FLAGS",
        };
        let silent = if self.silent { ".SILENT" } else { "" };
        // The flags are checked atoms already (`Flag`); nothing in them needs quoting.
        let flags: Vec<String> = self.flags.iter().map(|flag| flag.to_string()).collect();
        Ok(command.arg(&format!("{mode}{silent}")).arg(&format!("({})", flags.join(" "))))
    }
}

/// The UIDs a `MODIFIED` code names among those `asked` for; all of them when the code cannot be read, as nothing
/// can then be taken as stored.
pub(crate) fn modified(arg: &str, asked: &UidSet) -> UidSet {
    let Some(named) = ranges(arg) else { return asked.clone() };
    let named: UidSet = named.into_iter().collect();
    let mut kept = Vec::new();
    for &(first, last) in named.ranges() {
        for &(low, high) in asked.ranges() {
            let (from, to) = (first.max(low), last.min(high));
            if from <= to {
                kept.push((from, to));
            }
        }
    }
    UidSet::from(kept)
}
