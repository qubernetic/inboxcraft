// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mailbox commands: LIST (with SPECIAL-USE), SELECT, EXAMINE, STATUS and CLOSE.

use crate::imap::Capability;
use crate::imap::fixture::Mailbox;
use crate::imap::reader::Token;
use crate::imap::session::{Selected, Session, State};
use crate::imap::write::{flags, line, no, ok, string};

/// The flags every fake mailbox knows.
const FLAGS: &str = "(\\Answered \\Flagged \\Deleted \\Seen \\Draft)";

impl Session {
    /// The place of the mailbox called `name`; INBOX in any case.
    pub(super) fn find(&self, name: &[u8]) -> Option<usize> {
        let name = String::from_utf8_lossy(name);
        let name = if name.eq_ignore_ascii_case("INBOX") { "INBOX" } else { &name };
        self.fake.mailboxes.iter().position(|mailbox| mailbox.name == name)
    }

    /// The open mailbox.
    pub(super) fn selected(&self) -> Option<(&Selected, &Mailbox)> {
        let State::Selected(selected) = &self.state else { return None };
        Some((selected, self.fake.mailboxes.get(selected.index)?))
    }

    pub(super) fn list(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let (Some(reference), Some(pattern)) = (args.first().and_then(Token::astring), args.get(1).and_then(Token::astring)) else {
            return self.bad(out, Some(tag), "LIST needs a reference and a pattern");
        };
        match args.get(2..).unwrap_or_default() {
            [] => {}
            [Token::Atom(word), Token::List(options)] if word.eq_ignore_ascii_case("RETURN") => {
                for option in options {
                    if !option.atom().is_some_and(|option| option.eq_ignore_ascii_case("SPECIAL-USE")) {
                        return self.bad(out, Some(tag), "Unknown LIST return option");
                    }
                    if !self.fake.offers(Capability::SpecialUse) {
                        return self.bad(out, Some(tag), "SPECIAL-USE is not offered");
                    }
                }
            }
            _ => return self.bad(out, Some(tag), "LIST needs a reference and a pattern"),
        }
        let pattern = String::from_utf8_lossy(&[reference, pattern].concat()).into_owned();
        if pattern.is_empty() {
            line(out, "* LIST (\\Noselect) \"/\" \"\"");
            return ok(out, tag, "LIST completed");
        }
        for mailbox in &self.fake.mailboxes {
            let pattern = if mailbox.name == "INBOX" { pattern.to_ascii_uppercase() } else { pattern.clone() };
            if !matches(&pattern, &mailbox.name) {
                continue;
            }
            let parent = format!("{}/", mailbox.name);
            let children = self.fake.mailboxes.iter().any(|other| other.name.starts_with(&parent));
            let mut attributes = vec![if children { "\\HasChildren" } else { "\\HasNoChildren" }.to_owned()];
            attributes.extend(mailbox.special_use.clone().filter(|_| self.fake.offers(Capability::SpecialUse)));
            out.extend_from_slice(format!("* LIST {} \"/\" ", flags(&attributes)).as_bytes());
            out.extend_from_slice(&string(mailbox.name.as_bytes()));
            out.extend_from_slice(b"\r\n");
        }
        ok(out, tag, "LIST completed");
    }

    pub(super) fn select(&mut self, tag: &str, command: &str, args: &[Token], out: &mut Vec<u8>) {
        let read_only = command == "EXAMINE";
        let Some(name) = args.first().and_then(Token::astring) else {
            return self.bad(out, Some(tag), &format!("{command} needs a mailbox name"));
        };
        if args.len() > 1 {
            return self.bad(out, Some(tag), &format!("{command} parameters are not offered"));
        }
        self.state = State::Authenticated;
        let Some(index) = self.find(name) else { return no(out, tag, "[NONEXISTENT] No such mailbox") };
        let Some(mailbox) = self.fake.mailboxes.get(index) else { return };
        line(out, &format!("* {} EXISTS", mailbox.messages.len()));
        line(out, "* 0 RECENT");
        if let Some(seq) = mailbox.messages.iter().position(|message| !message.has_flag("\\Seen")) {
            line(out, &format!("* OK [UNSEEN {}] First unseen", seq + 1));
        }
        line(out, &format!("* OK [UIDVALIDITY {}] UIDs valid", mailbox.uid_validity));
        line(out, &format!("* OK [UIDNEXT {}] Predicted next UID", mailbox.uid_next));
        line(out, &format!("* FLAGS {FLAGS}"));
        if read_only {
            line(out, "* OK [PERMANENTFLAGS ()] Read-only");
        } else {
            line(out, "* OK [PERMANENTFLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft \\*)] Flags allowed");
        }
        if self.fake.offers(Capability::CondStore) {
            line(out, &format!("* OK [HIGHESTMODSEQ {}] Highest", mailbox.highest_modseq));
        }
        let view = mailbox.messages.iter().map(|message| message.uid).collect();
        self.state = State::Selected(Selected { index, read_only, view });
        ok(out, tag, &format!("[{}] {command} completed", if read_only { "READ-ONLY" } else { "READ-WRITE" }));
    }

    pub(super) fn status(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let (Some(name), Some(Token::List(items)), None) = (args.first().and_then(Token::astring), args.get(1), args.get(2)) else {
            return self.bad(out, Some(tag), "STATUS needs a mailbox name and items");
        };
        let Some(index) = self.find(name) else { return no(out, tag, "[NONEXISTENT] No such mailbox") };
        let condstore = self.fake.offers(Capability::CondStore);
        let Some(mailbox) = self.fake.mailboxes.get(index) else { return };
        let Some(counts) = items.iter().map(|item| count(mailbox, item, condstore)).collect::<Option<Vec<String>>>() else {
            return self.bad(out, Some(tag), "Unknown STATUS item");
        };
        out.extend_from_slice(b"* STATUS ");
        out.extend_from_slice(&string(mailbox.name.as_bytes()));
        line(out, &format!(" ({})", counts.join(" ")));
        ok(out, tag, "STATUS completed");
    }

    /// CLOSE: removes the `\Deleted` messages without telling, unless the mailbox is open read-only.
    pub(super) fn close(&mut self, tag: &str, out: &mut Vec<u8>) {
        if let State::Selected(Selected { index, read_only: false, .. }) = self.state
            && let Some(mailbox) = self.fake.mailboxes.get_mut(index)
        {
            mailbox.messages.retain(|message| !message.has_flag("\\Deleted"));
        }
        self.state = State::Authenticated;
        ok(out, tag, "CLOSE completed");
    }
}

/// One STATUS item and its value, or `None` for an item the fake does not know.
fn count(mailbox: &Mailbox, item: &Token, condstore: bool) -> Option<String> {
    let item = item.atom()?.to_ascii_uppercase();
    let value = match item.as_str() {
        "MESSAGES" => mailbox.messages.len() as u64,
        "UIDNEXT" => u64::from(mailbox.uid_next),
        "UIDVALIDITY" => u64::from(mailbox.uid_validity),
        "UNSEEN" => mailbox.unseen() as u64,
        "RECENT" => 0,
        "HIGHESTMODSEQ" if condstore => mailbox.highest_modseq,
        _ => return None,
    };
    Some(format!("{item} {value}"))
}

/// Whether `name` matches a LIST `pattern`: `*` stands for anything, `%` for anything without the `/` delimiter.
fn matches(pattern: &str, name: &str) -> bool {
    let name: Vec<char> = name.chars().collect();
    // row[j]: the pattern read so far matches the first j characters of the name.
    let mut row: Vec<bool> = (0..=name.len()).map(|j| j == 0).collect();
    for wanted in pattern.chars() {
        let mut carry = false;
        let next = (0..=name.len())
            .map(|j| {
                let here = row.get(j).copied().unwrap_or(false);
                let before = j.checked_sub(1);
                let last = before.and_then(|k| name.get(k)).copied();
                match wanted {
                    '*' => {
                        carry |= here;
                        carry
                    }
                    '%' => {
                        carry = (carry && last != Some('/')) || here;
                        carry
                    }
                    _ => last == Some(wanted) && before.and_then(|k| row.get(k)).copied().unwrap_or(false),
                }
            })
            .collect();
        row = next;
    }
    row.last().copied().unwrap_or(false)
}
