// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UID FETCH: flags, arrival time, size, and the whole message, its header or its text.

use crate::imap::fixture::Message;
use crate::imap::reader::Token;
use crate::imap::session::{Session, State};
use crate::imap::set::UidSet;
use crate::imap::write::{flags, literal, ok};

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
    /// A body item: the part, whether it leaves `\Seen` alone, and its name in the answer.
    Body(Section, bool, &'static str),
}

impl Item {
    fn read(token: &Token) -> Option<Self> {
        let item = match token.atom()?.to_ascii_uppercase().as_str() {
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
        let (Some(Token::Atom(set)), Some(items), None) = (args.first(), args.get(1), args.get(2)) else {
            return self.bad(out, Some(tag), "UID FETCH needs a UID set and items");
        };
        let items: Vec<&Token> = match items {
            Token::List(list) if !list.is_empty() => list.iter().collect(),
            Token::List(_) => return self.bad(out, Some(tag), "UID FETCH needs a UID set and items"),
            single => vec![single],
        };
        let Some(items) = items.into_iter().map(Item::read).collect::<Option<Vec<Item>>>() else {
            return self.bad(out, Some(tag), "Unknown FETCH item");
        };
        let highest = self.selected().and_then(|(_, mailbox)| mailbox.messages.last()).map_or(0, |message| message.uid);
        let Some(set) = UidSet::parse(set, highest) else { return self.bad(out, Some(tag), "Bad UID set") };
        let State::Selected(selected) = &mut self.state else { return };
        let Some(mailbox) = self.fake.mailboxes.get_mut(selected.index) else { return };
        let marks_seen = !selected.read_only && items.iter().any(|item| matches!(item, Item::Body(_, false, _)));
        for (seq, uid) in selected.view.iter().enumerate().filter(|(_, uid)| set.contains(**uid)) {
            let Some(message) = mailbox.messages.iter_mut().find(|message| message.uid == *uid) else { continue };
            let seen_now = marks_seen && !message.has_flag("\\Seen");
            if seen_now {
                message.flags.push("\\Seen".to_owned());
            }
            let mut parts = vec![answer(message, Item::Uid)];
            parts.extend(items.iter().filter(|item| **item != Item::Uid).map(|item| answer(message, *item)));
            if seen_now && !items.contains(&Item::Flags) {
                parts.push(answer(message, Item::Flags));
            }
            out.extend_from_slice(format!("* {} FETCH (", seq + 1).as_bytes());
            out.extend_from_slice(&parts.join(&b' '));
            out.extend_from_slice(b")\r\n");
        }
        ok(out, tag, "UID FETCH completed");
    }
}

/// One item of `message`'s FETCH answer.
fn answer(message: &Message, item: Item) -> Vec<u8> {
    let text = match item {
        Item::Uid => format!("UID {}", message.uid),
        Item::Flags => format!("FLAGS {}", flags(&message.flags)),
        Item::InternalDate => format!("INTERNALDATE \"{}\"", message.date),
        Item::Size => format!("RFC822.SIZE {}", message.raw.len()),
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
