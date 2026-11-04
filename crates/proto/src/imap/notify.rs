// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! NOTIFY (RFC 5465): what the server is asked to tell of, and the STATUS responses it tells of other mailboxes with.

use inboxcraft_model::{ModSeq, Uid, UidValidity};

use super::command::Command;
use super::list::{check_mailbox, mailbox_name};
use super::sync::modseq;
use super::value::Value;
use crate::ProtoError;

/// Most mailboxes one NOTIFY names.
pub const MAX_NOTIFY_MAILBOXES: usize = 64;

/// What NOTIFY SET asks the server to tell of, without being asked again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notify {
    /// The open mailbox: new messages with their UID and flags (and modification sequence with CONDSTORE enabled),
    /// removed messages and changed flags, as EXISTS, FETCH, EXPUNGE and VANISHED events.
    pub selected: bool,
    /// Other mailboxes: new and removed messages, as [`Event::Status`](super::Event::Status).
    pub mailboxes: Option<NotifyMailboxes>,
    /// Mailboxes among [`mailboxes`](Self::mailboxes) made, removed or renamed, as
    /// [`Event::MailboxChanged`](super::Event::MailboxChanged).
    pub mailbox_names: bool,
    /// A STATUS of each of [`mailboxes`](Self::mailboxes) right away.
    pub status: bool,
}

/// Which other mailboxes NOTIFY watches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotifyMailboxes {
    /// Every mailbox of the person's own (`PERSONAL`).
    Personal,
    /// The mailboxes new mail is delivered to (`INBOXES`).
    Inboxes,
    /// The subscribed mailboxes (`SUBSCRIBED`).
    Subscribed,
    /// These mailboxes and every one below them (`SUBTREE`), names as the server writes them, 1 to
    /// [`MAX_NOTIFY_MAILBOXES`].
    Subtree(Vec<String>),
    /// These mailboxes (`MAILBOXES`), as [`Subtree`](Self::Subtree).
    Mailboxes(Vec<String>),
}

impl Notify {
    /// `NOTIFY SET ...`; `modseq` when CONDSTORE is enabled.
    pub(crate) fn command(&self, modseq: bool) -> Result<Command, ProtoError> {
        if !self.selected && self.mailboxes.is_none() {
            return Err(ProtoError::BadArgument("NOTIFY events"));
        }
        let mut command = Command::new("NOTIFY SET");
        if self.status {
            command = command.arg("STATUS");
        }
        if self.selected {
            let fetch = if modseq { "(UID FLAGS MODSEQ)" } else { "(UID FLAGS)" };
            command = command.arg(&format!("(SELECTED (MessageNew {fetch} MessageExpunge FlagChange))"));
        }
        let Some(mailboxes) = &self.mailboxes else {
            return if self.mailbox_names { Err(ProtoError::BadArgument("MailboxName without mailboxes")) } else { Ok(command) };
        };
        let events = if self.mailbox_names { "(MessageNew MessageExpunge MailboxName))" } else { "(MessageNew MessageExpunge))" };
        let (filter, names) = match mailboxes {
            NotifyMailboxes::Personal => return Ok(command.arg("(PERSONAL").arg(events)),
            NotifyMailboxes::Inboxes => return Ok(command.arg("(INBOXES").arg(events)),
            NotifyMailboxes::Subscribed => return Ok(command.arg("(SUBSCRIBED").arg(events)),
            NotifyMailboxes::Subtree(names) => ("(SUBTREE", names),
            NotifyMailboxes::Mailboxes(names) => ("(MAILBOXES", names),
        };
        if names.is_empty() || names.len() > MAX_NOTIFY_MAILBOXES {
            return Err(ProtoError::BadArgument("NOTIFY mailboxes"));
        }
        // One group a mailbox: a list of names would need a space after its `(`, which a command cannot write here.
        for name in names {
            check_mailbox(name)?;
            command = command.arg(filter).string(name.as_bytes()).arg(events);
        }
        Ok(command)
    }
}

/// What a STATUS response says of a mailbox; only what it says is `Some`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailboxCounts {
    /// The name exactly as the server wrote it.
    pub raw: String,
    /// The name decoded from modified UTF-7, for people.
    pub name: String,
    /// How many messages it holds (`MESSAGES`).
    pub messages: Option<u32>,
    /// The UID the next message will get (`UIDNEXT`).
    pub uid_next: Option<Uid>,
    /// Its UIDVALIDITY.
    pub uid_validity: Option<UidValidity>,
    /// How many messages have no `\Seen` (`UNSEEN`).
    pub unseen: Option<u32>,
    /// Its highest modification sequence (`HIGHESTMODSEQ`, CONDSTORE).
    pub highest_modseq: Option<ModSeq>,
}

impl MailboxCounts {
    /// From the values of `STATUS <mailbox> (name value ...)`; `None` when they are not that or the name is not one
    /// LIST would keep.
    pub(crate) fn from_values(values: Vec<Value>) -> Option<Self> {
        let mut values = values.into_iter();
        if !values.next()?.atom()?.eq_ignore_ascii_case("STATUS") {
            return None;
        }
        let (raw, name) = mailbox_name(values.next()?.into_text()?)?;
        let Value::List(pairs) = values.next()? else { return None };
        let mut counts = Self { raw, name, ..Self::default() };
        let mut pairs = pairs.into_iter();
        while let (Some(item), Some(value)) = (pairs.next(), pairs.next()) {
            let (Some(item), Some(value)) = (item.atom().map(str::to_ascii_uppercase), value.atom().map(str::to_owned)) else { continue };
            let number = || super::value::number(&value);
            match item.as_str() {
                "MESSAGES" => counts.messages = number(),
                "UIDNEXT" => counts.uid_next = number().and_then(Uid::new),
                "UIDVALIDITY" => counts.uid_validity = number().and_then(UidValidity::new),
                "UNSEEN" => counts.unseen = number(),
                "HIGHESTMODSEQ" => counts.highest_modseq = modseq(&value),
                _ => {}
            }
        }
        Some(counts)
    }
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
