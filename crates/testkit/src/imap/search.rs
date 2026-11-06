// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UID SEARCH: flags, dates, sizes, UIDs, header and body text, NOT, OR and lists.

use crate::imap::fixture::Message;
use crate::imap::reader::Token;
use crate::imap::session::Session;
use crate::imap::set::UidSet;
use crate::imap::write::{line, no, ok};

/// How deep NOT, OR and lists may nest.
const MAX_DEPTH: usize = 32;

/// A day as `(year, month, day)`.
type Day = (u32, u32, u32);

/// One search key.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    All,
    /// Whether the message has the flag, or not.
    Flag(&'static str, bool),
    Since(Day),
    Before(Day),
    On(Day),
    Larger(u64),
    Smaller(u64),
    Uid(UidSet),
    /// A header field's value contains the text.
    Header(String, String),
    Body(String),
    Text(String),
    Not(Box<Key>),
    Or(Box<Key>, Box<Key>),
    And(Vec<Key>),
}

impl Key {
    fn matches(&self, message: &Message) -> bool {
        let day = || read_day(message.date.split(' ').next().unwrap_or_default());
        match self {
            Self::All => true,
            Self::Flag(flag, has) => message.has_flag(flag) == *has,
            Self::Since(wanted) => day().is_some_and(|day| day >= *wanted),
            Self::Before(wanted) => day().is_some_and(|day| day < *wanted),
            Self::On(wanted) => day() == Some(*wanted),
            Self::Larger(size) => message.raw.len() as u64 > *size,
            Self::Smaller(size) => (message.raw.len() as u64) < *size,
            Self::Uid(set) => set.contains(message.uid),
            Self::Header(field, text) => message.header(field).iter().any(|value| contains(value.as_bytes(), text)),
            Self::Body(text) => contains(message.parts().1, text),
            Self::Text(text) => contains(&message.raw, text),
            Self::Not(key) => !key.matches(message),
            Self::Or(left, right) => left.matches(message) || right.matches(message),
            Self::And(keys) => keys.iter().all(|key| key.matches(message)),
        }
    }
}

/// Whether `haystack` contains `text`, ignoring ASCII case.
fn contains(haystack: &[u8], text: &str) -> bool {
    String::from_utf8_lossy(haystack).to_lowercase().contains(&text.to_lowercase())
}

/// Reads an IMAP date (`3-Nov-2026`).
fn read_day(text: &str) -> Option<Day> {
    const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let mut parts = text.split('-');
    let (day, month, year) = (parts.next()?, parts.next()?, parts.next()?);
    let month = MONTHS.iter().position(|name| name.eq_ignore_ascii_case(month))?;
    Some((year.parse().ok()?, u32::try_from(month).ok()? + 1, day.parse().ok()?))
}

/// The search keys being read.
struct Keys<'a> {
    tokens: std::slice::Iter<'a, Token>,
    highest: u32,
}

impl Keys<'_> {
    fn key(&mut self, depth: usize) -> Result<Key, &'static str> {
        if depth > MAX_DEPTH {
            return Err("Search keys nested too deep");
        }
        let token = self.tokens.next().ok_or("UID SEARCH needs search keys")?;
        let word = match token {
            Token::List(inner) => return all(inner, self.highest, depth + 1),
            Token::Str(_) => return Err("Unknown search key"),
            Token::Atom(word) => word.to_ascii_uppercase(),
        };
        let flag = |flag, has| Ok(Key::Flag(flag, has));
        match word.as_str() {
            "ALL" => Ok(Key::All),
            "SEEN" => flag("\\Seen", true),
            "UNSEEN" => flag("\\Seen", false),
            "FLAGGED" => flag("\\Flagged", true),
            "UNFLAGGED" => flag("\\Flagged", false),
            "ANSWERED" => flag("\\Answered", true),
            "UNANSWERED" => flag("\\Answered", false),
            "DELETED" => flag("\\Deleted", true),
            "UNDELETED" => flag("\\Deleted", false),
            "DRAFT" => flag("\\Draft", true),
            "UNDRAFT" => flag("\\Draft", false),
            "SINCE" => Ok(Key::Since(self.day()?)),
            "BEFORE" => Ok(Key::Before(self.day()?)),
            "ON" => Ok(Key::On(self.day()?)),
            "LARGER" => Ok(Key::Larger(self.number()?)),
            "SMALLER" => Ok(Key::Smaller(self.number()?)),
            "UID" => {
                let set = self.tokens.next().and_then(Token::atom).and_then(|set| UidSet::parse(set, self.highest));
                Ok(Key::Uid(set.ok_or("Bad UID set")?))
            }
            "FROM" | "TO" | "CC" | "BCC" | "SUBJECT" => Ok(Key::Header(word.clone(), self.text()?)),
            "HEADER" => Ok(Key::Header(self.text()?, self.text()?)),
            "BODY" => Ok(Key::Body(self.text()?)),
            "TEXT" => Ok(Key::Text(self.text()?)),
            "NOT" => Ok(Key::Not(Box::new(self.key(depth + 1)?))),
            "OR" => Ok(Key::Or(Box::new(self.key(depth + 1)?), Box::new(self.key(depth + 1)?))),
            _ => Err("Unknown search key"),
        }
    }

    fn text(&mut self) -> Result<String, &'static str> {
        let text = self.tokens.next().and_then(Token::astring).ok_or("Missing search text")?;
        Ok(String::from_utf8_lossy(text).into_owned())
    }

    fn day(&mut self) -> Result<Day, &'static str> {
        self.tokens.next().and_then(Token::astring).and_then(|text| read_day(&String::from_utf8_lossy(text))).ok_or("Bad date")
    }

    fn number(&mut self) -> Result<u64, &'static str> {
        self.tokens.next().and_then(Token::atom).and_then(|number| number.parse().ok()).ok_or("Bad number")
    }
}

/// All the keys in `tokens`, at least one.
fn all(tokens: &[Token], highest: u32, depth: usize) -> Result<Key, &'static str> {
    let mut keys = Keys { tokens: tokens.iter(), highest };
    let mut all = vec![keys.key(depth)?];
    while keys.tokens.len() > 0 {
        all.push(keys.key(depth)?);
    }
    Ok(Key::And(all))
}

impl Session {
    pub(super) fn uid_search(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let mut args = args;
        if let [Token::Atom(word), rest @ ..] = args
            && word.eq_ignore_ascii_case("CHARSET")
        {
            let [charset, rest @ ..] = rest else { return self.bad(out, Some(tag), "UID SEARCH needs search keys") };
            let charset = charset.astring().map(String::from_utf8_lossy).unwrap_or_default();
            if !charset.eq_ignore_ascii_case("UTF-8") && !charset.eq_ignore_ascii_case("US-ASCII") {
                return no(out, tag, "[BADCHARSET (UTF-8 US-ASCII)] Unknown charset");
            }
            args = rest;
        }
        let highest = self.selected().and_then(|(_, mailbox)| mailbox.messages.last()).map_or(0, |message| message.uid);
        let key = match all(args, highest, 0) {
            Ok(key) => key,
            Err(reason) => return self.bad(out, Some(tag), reason),
        };
        let Some((selected, mailbox)) = self.selected() else { return };
        let found: Vec<String> = selected
            .view
            .iter()
            .filter_map(|uid| mailbox.messages.iter().find(|message| message.uid == *uid))
            .filter(|message| key.matches(message))
            .map(|message| message.uid.to_string())
            .collect();
        line(out, &["* SEARCH".to_owned()].into_iter().chain(found).collect::<Vec<_>>().join(" "));
        ok(out, tag, "UID SEARCH completed");
    }
}
