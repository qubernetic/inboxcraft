// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scripted changes to the mailboxes, and how the client hears of them: untagged EXISTS, EXPUNGE or VANISHED and
//! FETCH for the open mailbox, at the next command or at once while it idles or NOTIFY asks; STATUS for the others
//! when NOTIFY covers them.

use crate::imap::fixture::{Mailbox, Message};
use crate::imap::session::{Session, State};
use crate::imap::write::{flags, line, string};

/// A change a script makes to a mailbox.
pub(super) enum Change {
    Deliver(Message),
    Expunge(u32),
    Flags(u32, Vec<String>),
    UidValidity(u32),
}

/// What the client of the open mailbox has not been told yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Update {
    /// New messages.
    Exists,
    Expunge(u32),
    /// The message's flags changed.
    Fetch(u32),
    UidValidity,
}

impl Session {
    /// Makes `change` to the mailbox called `name` (none when there is no such mailbox) and adds what the client is
    /// told of it at once to `out`.
    pub(super) fn change(&mut self, name: &str, change: Change, out: &mut Vec<u8>) {
        let Some(index) = self.find(name.as_bytes()) else { return };
        let Some(mailbox) = self.fake.mailboxes.get_mut(index) else { return };
        let update = match change {
            Change::Deliver(message) => {
                mailbox.add(message);
                Some(Update::Exists)
            }
            Change::Expunge(uid) => mailbox.expunge(|message| message.uid == uid).first().map(|&uid| Update::Expunge(uid)),
            Change::Flags(uid, flags) => mailbox.messages.iter().position(|message| message.uid == uid).map(|position| {
                let modseq = mailbox.bump();
                if let Some(message) = mailbox.messages.get_mut(position) {
                    message.flags = flags;
                    message.modseq = modseq;
                }
                Update::Fetch(uid)
            }),
            Change::UidValidity(value) => {
                mailbox.renumber(value);
                Some(Update::UidValidity)
            }
        };
        let Some(update) = update else { return };
        let open = matches!(&self.state, State::Selected(selected) if selected.index == index);
        if open {
            self.pending.push(update);
            if self.idling || self.notify.as_ref().is_some_and(|notify| notify.selected.is_some()) {
                self.flush(out);
            }
        } else if let Some(mailbox) = self.fake.mailboxes.get(index)
            && self.notify.as_ref().is_some_and(|notify| notify.covers(&mailbox.name))
        {
            status(mailbox, self.condstore, out);
        }
    }

    /// Tells the client what changed in the open mailbox since it was last told.
    pub(super) fn flush(&mut self, out: &mut Vec<u8>) {
        let (qresync, condstore) = (self.qresync, self.condstore);
        let fetch: Vec<String> =
            self.notify.as_ref().and_then(|notify| notify.selected.as_ref()).map(|events| events.fetch.clone()).unwrap_or_default();
        let State::Selected(selected) = &mut self.state else { return };
        let Some(mailbox) = self.fake.mailboxes.get(selected.index) else { return };
        for update in std::mem::take(&mut self.pending) {
            match update {
                Update::Exists => {
                    let new: Vec<&Message> = mailbox.messages.iter().filter(|message| !selected.view.contains(&message.uid)).collect();
                    selected.view.extend(new.iter().map(|message| message.uid));
                    line(out, &format!("* {} EXISTS", selected.view.len()));
                    for message in new.iter().filter(|_| !fetch.is_empty()) {
                        let seq = selected.view.iter().position(|uid| *uid == message.uid).map_or(0, |at| at + 1);
                        let items: Vec<String> = fetch.iter().map(|item| fetch_item(message, item)).collect();
                        line(out, &format!("* {seq} FETCH ({})", items.join(" ")));
                    }
                }
                Update::Expunge(uid) => {
                    let Some(at) = selected.view.iter().position(|known| *known == uid) else { continue };
                    selected.view.remove(at);
                    line(out, &if qresync { format!("* VANISHED {uid}") } else { format!("* {} EXPUNGE", at + 1) });
                }
                Update::Fetch(uid) => {
                    let Some(at) = selected.view.iter().position(|known| *known == uid) else { continue };
                    let Some(message) = mailbox.messages.iter().find(|message| message.uid == uid) else { continue };
                    let modseq = if condstore { format!(" MODSEQ ({})", message.modseq) } else { String::new() };
                    line(out, &format!("* {} FETCH (UID {uid} FLAGS {}{modseq})", at + 1, flags(&message.flags)));
                }
                Update::UidValidity => {
                    selected.view = mailbox.messages.iter().map(|message| message.uid).collect();
                    line(out, &format!("* OK [UIDVALIDITY {}] UIDs changed", mailbox.uid_validity));
                    line(out, &format!("* {} EXISTS", selected.view.len()));
                }
            }
        }
    }
}

/// One item NOTIFY asks for with MessageNew (UID, FLAGS or MODSEQ).
fn fetch_item(message: &Message, item: &str) -> String {
    match item {
        "UID" => format!("UID {}", message.uid),
        "FLAGS" => format!("FLAGS {}", flags(&message.flags)),
        _ => format!("MODSEQ ({})", message.modseq),
    }
}

/// The STATUS line NOTIFY sends for a mailbox that is not open.
pub(super) fn status(mailbox: &Mailbox, condstore: bool, out: &mut Vec<u8>) {
    let modseq = if condstore { format!(" HIGHESTMODSEQ {}", mailbox.highest_modseq) } else { String::new() };
    out.extend_from_slice(b"* STATUS ");
    out.extend_from_slice(&string(mailbox.name.as_bytes()));
    let counts = format!(
        " (MESSAGES {} UIDNEXT {} UIDVALIDITY {} UNSEEN {}{modseq})",
        mailbox.messages.len(),
        mailbox.uid_next,
        mailbox.uid_validity,
        mailbox.unseen()
    );
    line(out, &counts);
}
