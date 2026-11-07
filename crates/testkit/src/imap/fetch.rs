// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UID FETCH: flags, arrival time, size, and the whole message, its header or its text.

use crate::imap::Capability;
use crate::imap::fixture::Message;
use crate::imap::reader::Token;
use crate::imap::session::{Session, State};
use crate::imap::set::{UidSet, compact};
use crate::imap::write::{flags, line, literal, ok, string};

/// The part of a message a body item returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Full,
    Header,
    Text,
}

/// One FETCH item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Uid,
    Flags,
    InternalDate,
    Size,
    ModSeq,
    Preview,
    /// A body item: the part, whether it leaves `\Seen` alone, and its name in the answer.
    Body(Section, bool, &'static str),
}

impl Item {
    fn read(token: &Token, condstore: bool, preview: bool) -> Option<Self> {
        let item = match token.atom()?.to_ascii_uppercase().as_str() {
            "MODSEQ" if condstore => Self::ModSeq,
            "PREVIEW" if preview => Self::Preview,
            "UID" => Self::Uid,
            "FLAGS" => Self::Flags,
            "INTERNALDATE" => Self::InternalDate,
            "RFC822.SIZE" => Self::Size,
            "BODY[]" => Self::Body(Section::Full, false, "BODY[]"),
            "BODY.PEEK[]" => Self::Body(Section::Full, true, "BODY[]"),
            "BODY[HEADER]" => Self::Body(Section::Header, false, "BODY[HEADER]"),
            "BODY.PEEK[HEADER]" => Self::Body(Section::Header, true, "BODY[HEADER]"),
            "BODY[TEXT]" => Self::Body(Section::Text, false, "BODY[TEXT]"),
            "BODY.PEEK[TEXT]" => Self::Body(Section::Text, true, "BODY[TEXT]"),
            "RFC822" => Self::Body(Section::Full, false, "RFC822"),
            "RFC822.HEADER" => Self::Body(Section::Header, true, "RFC822.HEADER"),
            "RFC822.TEXT" => Self::Body(Section::Text, false, "RFC822.TEXT"),
            _ => return None,
        };
        Some(item)
    }
}

impl Session {
    pub(super) fn uid_fetch(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let (Some(Token::Atom(set)), Some(items)) = (args.first(), args.get(1)) else {
            return self.bad(out, Some(tag), "UID FETCH needs a UID set and items");
        };
        let modifiers = match args.get(2..).unwrap_or_default() {
            [] => &[][..],
            [Token::List(modifiers)] => modifiers.as_slice(),
            _ => return self.bad(out, Some(tag), "UID FETCH needs a UID set and items"),
        };
        let items: Vec<&Token> = match items {
            Token::List(list) if !list.is_empty() => list.iter().collect(),
            Token::List(_) => return self.bad(out, Some(tag), "UID FETCH needs a UID set and items"),
            single => vec![single],
        };
        let Some(items) = read_items(items, self.fake.offers(Capability::CondStore), self.fake.offers(Capability::Preview)) else {
            return self.bad(out, Some(tag), "Unknown FETCH item");
        };
        let (changed_since, vanished) = match self.modifiers(modifiers) {
            Ok(modifiers) => modifiers,
            Err(reason) => return self.bad(out, Some(tag), reason),
        };
        let highest = self.selected().and_then(|(_, mailbox)| mailbox.messages.last()).map_or(0, |message| message.uid);
        let Some(set) = UidSet::parse(set, highest) else { return self.bad(out, Some(tag), "Bad UID set") };
        self.condstore |= changed_since.is_some() || items.contains(&Item::ModSeq);
        let condstore = self.condstore;
        let State::Selected(selected) = &mut self.state else { return };
        let Some(mailbox) = self.fake.mailboxes.get_mut(selected.index) else { return };
        if let Some(modseq) = changed_since.filter(|_| vanished) {
            let gone: Vec<u32> = mailbox.vanished_since(modseq).into_iter().filter(|uid| set.contains(*uid)).collect();
            if !gone.is_empty() {
                line(out, &format!("* VANISHED (EARLIER) {}", compact(&gone)));
            }
        }
        let marks_seen = !selected.read_only && items.iter().any(|item| matches!(item, Item::Body(_, false, _)));
        for (seq, uid) in selected.view.iter().enumerate().filter(|(_, uid)| set.contains(**uid)) {
            let Some(index) = mailbox.messages.iter().position(|message| message.uid == *uid) else { continue };
            let changed = mailbox.messages.get(index).is_some_and(|message| changed_since.is_none_or(|modseq| message.modseq > modseq));
            if !changed {
                continue;
            }
            let seen_now = marks_seen && mailbox.messages.get(index).is_some_and(|message| !message.has_flag("\\Seen"));
            if seen_now {
                let modseq = mailbox.bump();
                if let Some(message) = mailbox.messages.get_mut(index) {
                    message.flags.push("\\Seen".to_owned());
                    message.modseq = modseq;
                }
            }
            let Some(message) = mailbox.messages.get(index) else { continue };
            let mut parts = vec![answer(message, Item::Uid)];
            parts.extend(items.iter().filter(|item| **item != Item::Uid).map(|item| answer(message, *item)));
            if seen_now && !items.contains(&Item::Flags) {
                parts.push(answer(message, Item::Flags));
            }
            if (changed_since.is_some() || (seen_now && condstore)) && !items.contains(&Item::ModSeq) {
                parts.push(answer(message, Item::ModSeq));
            }
            out.extend_from_slice(format!("* {} FETCH (", seq + 1).as_bytes());
            out.extend_from_slice(&parts.join(&b' '));
            out.extend_from_slice(b")\r\n");
        }
        ok(out, tag, "UID FETCH completed");
    }

    /// Reads the FETCH modifiers (RFC 7162): CHANGEDSINCE, and VANISHED with it once QRESYNC is enabled.
    fn modifiers(&self, modifiers: &[Token]) -> Result<(Option<u64>, bool), &'static str> {
        let mut changed_since = None;
        let mut vanished = false;
        let mut tokens = modifiers.iter();
        while let Some(token) = tokens.next() {
            match token.atom().map(str::to_ascii_uppercase).as_deref() {
                Some("CHANGEDSINCE") if self.fake.offers(Capability::CondStore) => {
                    changed_since = Some(tokens.next().and_then(Token::atom).and_then(|modseq| modseq.parse().ok()).ok_or("Bad CHANGEDSINCE")?);
                }
                Some("VANISHED") if self.fake.offers(Capability::QResync) => vanished = true,
                _ => return Err("Unknown FETCH modifier"),
            }
        }
        if vanished && (!self.qresync || changed_since.is_none()) {
            return Err("VANISHED needs QRESYNC and CHANGEDSINCE");
        }
        Ok((changed_since, vanished))
    }
}

/// The FETCH items; PREVIEW may be followed by `(LAZY)`, which the fake takes like PREVIEW alone.
fn read_items(tokens: Vec<&Token>, condstore: bool, preview: bool) -> Option<Vec<Item>> {
    let mut items = Vec::new();
    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        let item = Item::read(token, condstore, preview)?;
        if item == Item::Preview
            && let Some(Token::List(modifiers)) = tokens.peek()
        {
            if !matches!(modifiers.as_slice(), [lazy] if lazy.atom().is_some_and(|lazy| lazy.eq_ignore_ascii_case("LAZY"))) {
                return None;
            }
            tokens.next();
        }
        items.push(item);
    }
    Some(items)
}

/// One item of `message`'s FETCH answer.
fn answer(message: &Message, item: Item) -> Vec<u8> {
    let text = match item {
        Item::Uid => format!("UID {}", message.uid),
        Item::Flags => format!("FLAGS {}", flags(&message.flags)),
        Item::InternalDate => format!("INTERNALDATE \"{}\"", message.date),
        Item::Size => format!("RFC822.SIZE {}", message.raw.len()),
        Item::ModSeq => format!("MODSEQ ({})", message.modseq),
        Item::Preview => {
            let Some(text) = message.preview_text() else { return b"PREVIEW NIL".to_vec() };
            let mut out = b"PREVIEW ".to_vec();
            out.extend_from_slice(&string(text.as_bytes()));
            return out;
        }
        Item::Body(section, _, name) => {
            let (header, text) = message.parts();
            let bytes = match section {
                Section::Full => message.raw.as_slice(),
                Section::Header => header,
                Section::Text => text,
            };
            let mut out = format!("{name} ").into_bytes();
            out.extend_from_slice(&literal(bytes));
            return out;
        }
    };
    text.into_bytes()
}
