// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The grouped inbox: which group a message belongs in, People, Updates, Newsletters or Invitations.
//!
//! The owner's choice for the sender comes first (`group.set`, or the group `screener.accept` gave it): it holds
//! for every message from that address, the kept mail and the mail still to come. Without one, our own classifier
//! reads the signs the header and the kept body give (`inboxcraft_model::Signs`), first match wins:
//!
//! 1. a `text/calendar` message or part: Invitations;
//! 2. `Auto-Submitted` other than `no` (RFC 3834): Updates;
//! 3. a list field (RFC 2369, RFC 2919) or `Precedence: list` or `bulk`: Newsletters;
//! 4. `Precedence: junk`, or a sender whose local part says it reads no reply (`noreply`, `no-reply`,
//!    `donotreply` ..., separators ignored) or is a delivery report's (`MAILER-DAEMON`, `postmaster`): Updates;
//! 5. anything else: People.
//!
//! The group is worked out when a list asks, never stored, so a choice or a body kept later moves the mail at once.
//! An account whose grouping the owner turned off has no groups.

use std::collections::BTreeMap;

use inboxcraft_mime::{Body, Part};
use inboxcraft_model::{Account, EmailAddress, Envelope, Group, Message, Precedence};
use inboxcraft_store::Storage;

use super::screener;
use crate::EngineError;

/// Every group, in the order lists show them.
pub(crate) const ALL: [Group; 4] = [Group::People, Group::Updates, Group::Newsletters, Group::Invitations];

/// The beginnings of the local parts that read no reply, separators taken out.
const NO_REPLY: [&str; 2] = ["noreply", "donotreply"];

/// The local parts of the senders of delivery reports, separators taken out.
const REPORTERS: [&str; 2] = ["mailerdaemon", "postmaster"];

/// The group's name, as commands write it.
pub(crate) const fn name(group: Group) -> &'static str {
    match group {
        Group::People => "people",
        Group::Updates => "updates",
        Group::Newsletters => "newsletters",
        Group::Invitations => "invitations",
    }
}

/// The group named `text`.
pub(crate) fn named(text: &str) -> Option<Group> {
    ALL.into_iter().find(|group| name(*group) == text)
}

/// The group our classifier gives a message with `envelope`.
pub(crate) fn classify(envelope: &Envelope) -> Group {
    let signs = envelope.signs();
    let precedence = signs.precedence();
    if signs.calendar() || signs.calendar_type() {
        Group::Invitations
    } else if signs.automatic() {
        Group::Updates
    } else if signs.list() || matches!(precedence, Some(Precedence::List | Precedence::Bulk)) {
        Group::Newsletters
    } else if precedence == Some(Precedence::Junk) || envelope.from().first().is_some_and(|from| robot(from.email())) {
        Group::Updates
    } else {
        Group::People
    }
}

/// Whether `address` is a sender no person answers from: a no-reply address or a delivery report's.
pub(crate) fn robot(address: &EmailAddress) -> bool {
    let local: String = address.local_part().chars().filter(|c| !matches!(c, '-' | '_' | '.')).flat_map(char::to_lowercase).collect();
    NO_REPLY.iter().any(|start| local.starts_with(start)) || REPORTERS.contains(&local.as_str())
}

/// Whether the message `raw` holds a `text/calendar` part of its own: itself or a part of its multiparts, not inside
/// an attached message. A message that cannot be read holds none.
pub(crate) fn calendar(raw: &[u8]) -> bool {
    let Ok(message) = inboxcraft_mime::parse(raw) else { return false };
    let mut parts: Vec<&Part<'_>> = vec![&message];
    while let Some(part) = parts.pop() {
        if part.content_type().is("text", "calendar") {
            return true;
        }
        if let Body::Multipart(multipart) = part.body() {
            parts.extend(multipart.parts());
        }
    }
    false
}

/// The groups of messages: the owner's choices of each sender and the accounts' grouping, read once.
pub(crate) struct Grouping<'a> {
    store: &'a dyn Storage,
    accounts: Vec<Account>,
    chosen: BTreeMap<String, Option<Group>>,
}

impl<'a> Grouping<'a> {
    /// The grouping the store holds now.
    pub(crate) fn new(store: &'a dyn Storage) -> Result<Self, EngineError> {
        Ok(Self { store, accounts: store.accounts()?, chosen: BTreeMap::new() })
    }

    /// The group of `message`; `None` when its account is not grouped.
    pub(crate) fn of(&mut self, message: &Message) -> Result<Option<Group>, EngineError> {
        if !self.accounts.iter().any(|account| account.id() == message.account() && account.grouped()) {
            return Ok(None);
        }
        if let Some(address) = screener::sender_of(message) {
            let key = address.as_str().to_lowercase();
            let chosen = match self.chosen.get(&key) {
                Some(chosen) => *chosen,
                None => {
                    let chosen = self.store.sender(address)?.and_then(|sender| sender.group);
                    self.chosen.insert(key, chosen);
                    chosen
                }
            };
            if chosen.is_some() {
                return Ok(chosen);
            }
        }
        Ok(Some(classify(message.summary().envelope())))
    }
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
