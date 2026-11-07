// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mailbox management commands: CREATE, DELETE and APPEND (RFC 3501 sections 6.3.3, 6.3.4 and 6.3.11), with
//! APPENDUID when UIDPLUS is offered (RFC 4315).

use crate::imap::Capability;
use crate::imap::fixture::{Mailbox, Message};
use crate::imap::reader::Token;
use crate::imap::session::{Session, State};
use crate::imap::update::Change;
use crate::imap::write::{no, ok};

impl Session {
    /// CREATE: an empty mailbox with a UIDVALIDITY no mailbox of the session had before.
    pub(super) fn create(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let Some(name) = mailbox_name(args) else { return self.bad(out, Some(tag), "CREATE needs a mailbox name") };
        if self.find(name.as_bytes()).is_some() {
            return no(out, tag, "[ALREADYEXISTS] The mailbox exists");
        }
        let highest = self.fake.mailboxes.iter().map(|mailbox| mailbox.uid_validity).max().unwrap_or(0);
        self.fake.last_created = self.fake.last_created.max(highest).saturating_add(1);
        self.fake.mailboxes.push(Mailbox::new(&name).uid_validity(self.fake.last_created));
        ok(out, tag, "CREATE completed");
    }

    /// DELETE: any mailbox but INBOX and the one open in this session.
    pub(super) fn delete(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let Some(name) = mailbox_name(args) else { return self.bad(out, Some(tag), "DELETE needs a mailbox name") };
        let Some(index) = self.find(name.as_bytes()) else { return no(out, tag, "[NONEXISTENT] No such mailbox") };
        if name.eq_ignore_ascii_case("INBOX") {
            return no(out, tag, "[CANNOT] INBOX cannot be deleted");
        }
        if let State::Selected(selected) = &mut self.state {
            if selected.index == index {
                return no(out, tag, "[INUSE] The mailbox is open");
            }
            if selected.index > index {
                selected.index -= 1;
            }
        }
        self.fake.mailboxes.remove(index);
        ok(out, tag, "DELETE completed");
    }

    /// APPEND: the message with its flags and arrival time, if given, under the mailbox's next UID.
    pub(super) fn append(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let parts = match args {
            [name, Token::Str(raw)] => Some((name, None, None, raw)),
            [name, Token::List(flags), Token::Str(raw)] => Some((name, Some(flags), None, raw)),
            [name, Token::Str(date), Token::Str(raw)] => Some((name, None, Some(date), raw)),
            [name, Token::List(flags), Token::Str(date), Token::Str(raw)] => Some((name, Some(flags), Some(date), raw)),
            _ => None,
        };
        let Some((name, flags, date, raw)) = parts.and_then(|(name, flags, date, raw)| Some((name.astring()?, flags, date, raw))) else {
            return self.bad(out, Some(tag), "APPEND needs a mailbox and a message");
        };
        let Some(index) = self.find(name) else { return no(out, tag, "[TRYCREATE] No such mailbox") };
        let mut message = Message::from_raw(raw.clone());
        if let Some(flags) = flags {
            let flags: Vec<&str> = flags.iter().filter_map(Token::atom).collect();
            message = message.flags(&flags);
        }
        if let Some(date) = date {
            message = message.date(&String::from_utf8_lossy(date));
        }
        let Some(mailbox) = self.fake.mailboxes.get(index) else { return };
        let (name, uid_validity, uid) = (mailbox.name.clone(), mailbox.uid_validity, mailbox.uid_next);
        let mut told = Vec::new();
        self.change(&name, Change::Deliver(message), &mut told);
        if self.fake.offers(Capability::UidPlus) {
            ok(out, tag, &format!("[APPENDUID {uid_validity} {uid}] APPEND completed"));
        } else {
            ok(out, tag, "APPEND completed");
        }
        out.extend_from_slice(&told);
    }
}

/// The one mailbox name of CREATE or DELETE; an empty one is none.
fn mailbox_name(args: &[Token]) -> Option<String> {
    let [name] = args else { return None };
    let name = String::from_utf8_lossy(name.astring()?).into_owned();
    (!name.is_empty()).then_some(name)
}
