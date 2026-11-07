// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The commands that change the open mailbox: UID STORE (with UNCHANGEDSINCE), UID COPY, UID MOVE (RFC 6851),
//! UID EXPUNGE (RFC 4315) and EXPUNGE, with COPYUID when UIDPLUS is offered.

use crate::imap::Capability;
use crate::imap::fixture::Message;
use crate::imap::reader::Token;
use crate::imap::session::{Session, State};
use crate::imap::set::{UidSet, compact};
use crate::imap::update::Update;
use crate::imap::write::{flags, line, no, ok};

const STORE_USAGE: &str = "UID STORE needs a UID set, an operation and flags";

/// How UID STORE changes the flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Set,
    Add,
    Remove,
}

impl Mode {
    fn apply(self, own: &[String], given: &[String]) -> Vec<String> {
        let has = |list: &[String], flag: &str| list.iter().any(|other| other.eq_ignore_ascii_case(flag));
        match self {
            Self::Set => {
                given.iter().enumerate().filter(|(at, flag)| !has(given.get(..*at).unwrap_or_default(), flag)).map(|(_, flag)| flag.clone()).collect()
            }
            Self::Add => own.iter().chain(given.iter().filter(|flag| !has(own, flag))).cloned().collect(),
            Self::Remove => own.iter().filter(|flag| !has(given, flag)).cloned().collect(),
        }
    }
}

impl Session {
    /// The highest UID of the open mailbox, for `*`.
    fn highest(&self) -> u32 {
        self.selected().and_then(|(_, mailbox)| mailbox.messages.last()).map_or(0, |message| message.uid)
    }

    fn read_only(&self) -> bool {
        self.selected().is_some_and(|(selected, _)| selected.read_only)
    }

    pub(super) fn uid_store(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let Some(Token::Atom(set)) = args.first() else { return self.bad(out, Some(tag), STORE_USAGE) };
        let (unchanged_since, rest) = match args.get(1..).unwrap_or_default() {
            [Token::List(modifier), rest @ ..] => {
                let value = match modifier.as_slice() {
                    [Token::Atom(word), Token::Atom(value)]
                        if word.eq_ignore_ascii_case("UNCHANGEDSINCE") && self.fake.offers(Capability::CondStore) =>
                    {
                        value.parse::<u64>().ok()
                    }
                    _ => None,
                };
                let Some(value) = value else { return self.bad(out, Some(tag), "Bad UNCHANGEDSINCE") };
                (Some(value), rest)
            }
            rest => (None, rest),
        };
        let Some((Token::Atom(operation), given)) = rest.split_first() else { return self.bad(out, Some(tag), STORE_USAGE) };
        let operation = operation.to_ascii_uppercase();
        let (mode, silent) = operation.strip_suffix(".SILENT").map_or((operation.as_str(), false), |mode| (mode, true));
        let mode = match mode {
            "FLAGS" => Mode::Set,
            "+FLAGS" => Mode::Add,
            "-FLAGS" => Mode::Remove,
            _ => return self.bad(out, Some(tag), STORE_USAGE),
        };
        let given = match given {
            [] => None,
            [Token::List(list)] => list.iter().map(|flag| flag.atom().map(str::to_owned)).collect::<Option<Vec<String>>>(),
            atoms => atoms.iter().map(|flag| flag.atom().map(str::to_owned)).collect(),
        };
        let Some(given) = given else { return self.bad(out, Some(tag), STORE_USAGE) };
        let Some(set) = UidSet::parse(set, self.highest()) else { return self.bad(out, Some(tag), "Bad UID set") };
        if self.read_only() {
            return no(out, tag, "[READ-ONLY] The mailbox is open read-only");
        }
        self.condstore |= unchanged_since.is_some();
        let condstore = self.condstore;
        let State::Selected(selected) = &mut self.state else { return };
        let Some(mailbox) = self.fake.mailboxes.get_mut(selected.index) else { return };
        let mut modified = Vec::new();
        for (seq, uid) in selected.view.iter().enumerate().filter(|(_, uid)| set.contains(**uid)) {
            let Some(position) = mailbox.messages.iter().position(|message| message.uid == *uid) else { continue };
            let Some(message) = mailbox.messages.get(position) else { continue };
            if unchanged_since.is_some_and(|modseq| message.modseq > modseq) {
                modified.push(*uid);
                continue;
            }
            let changed = mode.apply(&message.flags, &given);
            if changed != message.flags {
                let modseq = mailbox.bump();
                if let Some(message) = mailbox.messages.get_mut(position) {
                    message.flags = changed;
                    message.modseq = modseq;
                }
            }
            let Some(message) = mailbox.messages.get(position).filter(|_| !silent) else { continue };
            let modseq = if condstore { format!(" MODSEQ ({})", message.modseq) } else { String::new() };
            line(out, &format!("* {} FETCH (UID {uid} FLAGS {}{modseq})", seq + 1, flags(&message.flags)));
        }
        if modified.is_empty() {
            ok(out, tag, "UID STORE completed");
        } else {
            modified.sort_unstable();
            ok(out, tag, &format!("[MODIFIED {}] Conditional STORE failed", compact(&modified)));
        }
    }

    /// UID COPY, or UID MOVE when `moving`: copies, then for a move expunges the originals.
    pub(super) fn uid_copy(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>, moving: bool) {
        let command = if moving { "UID MOVE" } else { "UID COPY" };
        let (Some(Token::Atom(set)), Some(target), None) = (args.first(), args.get(1).and_then(Token::astring), args.get(2)) else {
            return self.bad(out, Some(tag), &format!("{command} needs a UID set and a mailbox"));
        };
        let Some(set) = UidSet::parse(set, self.highest()) else { return self.bad(out, Some(tag), "Bad UID set") };
        if moving && self.read_only() {
            return no(out, tag, "[READ-ONLY] The mailbox is open read-only");
        }
        let Some(target) = self.find(target) else { return no(out, tag, "[TRYCREATE] No such mailbox") };
        let Some((selected, source)) = self.selected() else { return };
        let open = selected.index;
        let picked: Vec<Message> = selected
            .view
            .iter()
            .filter(|uid| set.contains(**uid))
            .filter_map(|uid| source.messages.iter().find(|message| message.uid == *uid))
            .cloned()
            .collect();
        let Some(mailbox) = self.fake.mailboxes.get_mut(target) else { return };
        let from: Vec<u32> = picked.iter().map(|message| message.uid).collect();
        let to: Vec<u32> = picked.into_iter().map(|message| mailbox.add(Message { uid: 0, ..message })).collect();
        let copy_uid = format!("[COPYUID {} {} {}] ", mailbox.uid_validity, compact(&from), compact(&to));
        let copy_uid = if self.fake.offers(Capability::UidPlus) && !from.is_empty() { copy_uid } else { String::new() };
        if target == open && !from.is_empty() {
            self.pending.push(Update::Exists);
        }
        if !moving {
            self.flush(out);
            return ok(out, tag, &format!("{copy_uid}UID COPY completed"));
        }
        if !copy_uid.is_empty() {
            line(out, &format!("* OK {}Moved", copy_uid));
        }
        self.expunge_open(|message| from.contains(&message.uid), out);
        ok(out, tag, "UID MOVE completed");
    }

    pub(super) fn uid_expunge(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let Some(Token::Atom(set)) = args.first() else { return self.bad(out, Some(tag), "UID EXPUNGE needs a UID set") };
        let Some(set) = UidSet::parse(set, self.highest()) else { return self.bad(out, Some(tag), "Bad UID set") };
        if self.read_only() {
            return no(out, tag, "[READ-ONLY] The mailbox is open read-only");
        }
        self.expunge_open(|message| message.has_flag("\\Deleted") && set.contains(message.uid), out);
        ok(out, tag, "UID EXPUNGE completed");
    }

    pub(super) fn expunge(&mut self, tag: &str, out: &mut Vec<u8>) {
        if self.read_only() {
            return no(out, tag, "[READ-ONLY] The mailbox is open read-only");
        }
        self.expunge_open(|message| message.has_flag("\\Deleted"), out);
        ok(out, tag, "EXPUNGE completed");
    }

    /// Removes the messages `remove` picks from the open mailbox and tells the client.
    fn expunge_open(&mut self, remove: impl Fn(&Message) -> bool, out: &mut Vec<u8>) {
        let State::Selected(selected) = &self.state else { return };
        let Some(mailbox) = self.fake.mailboxes.get_mut(selected.index) else { return };
        let gone = mailbox.expunge(remove);
        self.pending.extend(gone.into_iter().map(Update::Expunge));
        self.flush(out);
    }
}
