// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UID SEARCH criteria (RFC 9051 section 6.4.4).

use inboxcraft_model::UidSet;
use inboxcraft_time::days_in_month;

use super::capability::is_atom_char;
use super::command::Command;
use crate::ProtoError;

/// Deepest that `Not`, `Or` and `And` may nest.
pub const MAX_SEARCH_DEPTH: usize = 16;
/// Most keys one search may have, counted through every level.
pub const MAX_SEARCH_KEYS: usize = 256;
/// Most ranges a UID set in a command may have.
pub const MAX_SET_RANGES: usize = 1_000;

/// A day, as SEARCH compares dates: `1-Feb-1994`, without time or zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchDate {
    year: u16,
    month: u8,
    day: u8,
}

impl SearchDate {
    /// The day `day` of `month` (1 to 12) in `year` (1 to 9999), or `None` when there is no such day.
    #[must_use]
    pub fn new(year: u16, month: u8, day: u8) -> Option<Self> {
        let valid = (1..=9999).contains(&year) && (1..=12).contains(&month) && day >= 1 && day <= days_in_month(i32::from(year), month);
        valid.then_some(Self { year, month, day })
    }

    fn text(self) -> String {
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        let month = MONTHS.get(usize::from(self.month).saturating_sub(1)).copied().unwrap_or("Jan");
        format!("{}-{month}-{}", self.day, self.year)
    }
}

/// One search key. Texts are matched by the server, as a part of the field and whatever the case.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SearchKey {
    /// Every message.
    All,
    /// `\Seen` set.
    Seen,
    /// `\Seen` not set.
    Unseen,
    /// `\Flagged` set.
    Flagged,
    /// `\Flagged` not set.
    Unflagged,
    /// `\Answered` set.
    Answered,
    /// `\Answered` not set.
    Unanswered,
    /// `\Deleted` set.
    Deleted,
    /// `\Deleted` not set.
    Undeleted,
    /// `\Draft` set.
    Draft,
    /// Arrived on or after the day (INTERNALDATE).
    Since(SearchDate),
    /// Arrived before the day.
    Before(SearchDate),
    /// Arrived on the day.
    On(SearchDate),
    /// The messages with these UIDs.
    Uid(UidSet),
    /// Larger than this many bytes.
    Larger(u32),
    /// Smaller than this many bytes.
    Smaller(u32),
    /// `From` holds the text.
    From(String),
    /// `To` holds the text.
    To(String),
    /// `Subject` holds the text.
    Subject(String),
    /// The body holds the text.
    Body(String),
    /// The header or the body holds the text.
    Text(String),
    /// The header field (its name first) holds the text.
    Header(String, String),
    /// The key does not match.
    Not(Box<SearchKey>),
    /// Either key matches.
    Or(Box<SearchKey>, Box<SearchKey>),
    /// Every key matches; none is [`SearchKey::All`].
    And(Vec<SearchKey>),
}

/// A UID set as a command writes it: `4,7:9`. Refused when empty or longer than [`MAX_SET_RANGES`] ranges.
pub(crate) fn set_text(set: &UidSet) -> Result<String, ProtoError> {
    if set.is_empty() {
        return Err(ProtoError::BadArgument("empty UID set"));
    }
    if set.ranges().len() > MAX_SET_RANGES {
        return Err(ProtoError::BadArgument("UID set with too many ranges"));
    }
    let ranges: Vec<String> =
        set.ranges().iter().map(|(first, last)| if first == last { first.to_string() } else { format!("{first}:{last}") }).collect();
    Ok(ranges.join(","))
}

/// The command `UID SEARCH [CHARSET UTF-8] <key>`: the charset is named when a text is not ASCII.
pub(crate) fn command(key: &SearchKey) -> Result<Command, ProtoError> {
    let mut words = Words { parts: Vec::new(), keys: 0, utf8: false };
    match key {
        // Keys side by side are all to match: the outermost `And` needs no parentheses.
        SearchKey::And(keys) if !keys.is_empty() => {
            words.keys = 1;
            for key in keys {
                words.key(key, 1)?;
            }
        }
        key => words.key(key, 0)?,
    }
    let mut command = Command::new("UID SEARCH");
    if words.utf8 {
        command = command.arg("CHARSET UTF-8");
    }
    let mut glued = false;
    for part in words.parts {
        let open = matches!(part, Part::Open);
        command = match part {
            Part::Word(word) if glued => command.raw(&word),
            Part::Word(word) => command.arg(&word),
            Part::Text(text) => command.string(text.as_bytes()),
            Part::Open => command.arg("("),
            Part::Close => command.raw(")"),
        };
        glued = open;
    }
    Ok(command)
}

enum Part {
    /// Our own words: a key's name, a number, a date, a set, a parenthesis.
    Word(String),
    /// A text, quoted or as a literal.
    Text(String),
    /// `(`: the next word follows without a space.
    Open,
    /// `)`, right after the word before.
    Close,
}

struct Words {
    parts: Vec<Part>,
    keys: usize,
    utf8: bool,
}

impl Words {
    fn word(&mut self, word: impl Into<String>) {
        self.parts.push(Part::Word(word.into()));
    }

    fn text(&mut self, text: &str) -> Result<(), ProtoError> {
        if text.is_empty() || text.len() > 1_000 || text.chars().any(char::is_control) {
            return Err(ProtoError::BadArgument("search text"));
        }
        self.utf8 |= !text.is_ascii();
        self.parts.push(Part::Text(text.to_owned()));
        Ok(())
    }

    fn key(&mut self, key: &SearchKey, depth: usize) -> Result<(), ProtoError> {
        self.keys = self.keys.saturating_add(1);
        if depth > MAX_SEARCH_DEPTH || self.keys > MAX_SEARCH_KEYS {
            return Err(ProtoError::BadArgument("search, nested too deeply or with too many keys"));
        }
        match key {
            SearchKey::All => self.word("ALL"),
            SearchKey::Seen => self.word("SEEN"),
            SearchKey::Unseen => self.word("UNSEEN"),
            SearchKey::Flagged => self.word("FLAGGED"),
            SearchKey::Unflagged => self.word("UNFLAGGED"),
            SearchKey::Answered => self.word("ANSWERED"),
            SearchKey::Unanswered => self.word("UNANSWERED"),
            SearchKey::Deleted => self.word("DELETED"),
            SearchKey::Undeleted => self.word("UNDELETED"),
            SearchKey::Draft => self.word("DRAFT"),
            SearchKey::Since(date) => self.word(format!("SINCE {}", date.text())),
            SearchKey::Before(date) => self.word(format!("BEFORE {}", date.text())),
            SearchKey::On(date) => self.word(format!("ON {}", date.text())),
            SearchKey::Uid(set) => self.word(format!("UID {}", set_text(set)?)),
            SearchKey::Larger(size) => self.word(format!("LARGER {size}")),
            SearchKey::Smaller(size) => self.word(format!("SMALLER {size}")),
            SearchKey::From(text) => self.named_text("FROM", text)?,
            SearchKey::To(text) => self.named_text("TO", text)?,
            SearchKey::Subject(text) => self.named_text("SUBJECT", text)?,
            SearchKey::Body(text) => self.named_text("BODY", text)?,
            SearchKey::Text(text) => self.named_text("TEXT", text)?,
            SearchKey::Header(field, text) => {
                if field.is_empty() || field.len() > 100 || !field.bytes().all(|b| is_atom_char(b) && b != b':') {
                    return Err(ProtoError::BadArgument("header field name"));
                }
                self.word(format!("HEADER {field}"));
                self.text(text)?;
            }
            SearchKey::Not(key) => {
                self.word("NOT");
                self.key(key, depth.saturating_add(1))?;
            }
            SearchKey::Or(left, right) => {
                self.word("OR");
                self.key(left, depth.saturating_add(1))?;
                self.key(right, depth.saturating_add(1))?;
            }
            SearchKey::And(keys) => {
                if keys.is_empty() {
                    return Err(ProtoError::BadArgument("search with no keys"));
                }
                self.parts.push(Part::Open);
                for key in keys {
                    self.key(key, depth.saturating_add(1))?;
                }
                self.parts.push(Part::Close);
            }
        }
        Ok(())
    }

    fn named_text(&mut self, name: &str, text: &str) -> Result<(), ProtoError> {
        self.word(name);
        self.text(text)
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
