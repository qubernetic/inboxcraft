// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Message flags: the IMAP system flags and keywords (RFC 9051 section 2.3.2).

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

/// Why a text is not a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagError {
    /// A `\` flag other than the five of [`Flag`] (`\Recent` is gone from IMAP4rev2).
    UnknownSystemFlag,
    /// Not an IMAP atom of at most [`Keyword::MAX_LEN`] bytes.
    BadKeyword,
}

impl fmt::Display for FlagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownSystemFlag => "the flag is a system flag this program does not know",
            Self::BadKeyword => "the flag is not a valid keyword",
        })
    }
}

impl std::error::Error for FlagError {}

/// A keyword flag (`$Forwarded`, `$Junk`, ...): an IMAP atom, compared without case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Keyword(String);

impl Keyword {
    /// The longest keyword kept, in bytes.
    pub const MAX_LEN: usize = 128;

    /// The keyword `text`.
    ///
    /// # Errors
    ///
    /// [`FlagError::BadKeyword`] for an empty or too long text, or one with a byte outside the IMAP `atom-char`s
    /// (printable ASCII but `(`, `)`, `{`, space, `%`, `*`, `"`, `\` and `]`).
    pub fn new(text: &str) -> Result<Self, FlagError> {
        let atom_char = |b: u8| (0x21..0x7f).contains(&b) && !b"(){%*\"\\]".contains(&b);
        if text.is_empty() || text.len() > Self::MAX_LEN || !text.bytes().all(atom_char) {
            return Err(FlagError::BadKeyword);
        }
        Ok(Self(text.to_owned()))
    }

    /// The keyword as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn folded(&self) -> impl Iterator<Item = u8> + '_ {
        self.0.bytes().map(|b| b.to_ascii_lowercase())
    }
}

impl TryFrom<String> for Keyword {
    type Error = FlagError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(&text)
    }
}

impl From<Keyword> for String {
    fn from(keyword: Keyword) -> Self {
        keyword.0
    }
}

impl PartialEq for Keyword {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Eq for Keyword {}

impl PartialOrd for Keyword {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Keyword {
    fn cmp(&self, other: &Self) -> Ordering {
        self.folded().cmp(other.folded())
    }
}

impl Hash for Keyword {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for b in self.folded() {
            state.write_u8(b);
        }
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One flag of a message.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Flag {
    /// `\Seen`: read.
    Seen,
    /// `\Answered`: replied to.
    Answered,
    /// `\Flagged`: marked for attention.
    Flagged,
    /// `\Deleted`: to be expunged.
    Deleted,
    /// `\Draft`: not finished.
    Draft,
    /// A keyword.
    Keyword(Keyword),
}

impl Flag {
    /// The flag written as `text` (`\Seen` or a keyword), system flags without case.
    ///
    /// # Errors
    ///
    /// A [`FlagError`].
    pub fn parse(text: &str) -> Result<Self, FlagError> {
        let Some(name) = text.strip_prefix('\\') else {
            return Keyword::new(text).map(Self::Keyword);
        };
        const SYSTEM: [(&str, Flag); 5] =
            [("seen", Flag::Seen), ("answered", Flag::Answered), ("flagged", Flag::Flagged), ("deleted", Flag::Deleted), ("draft", Flag::Draft)];
        SYSTEM.into_iter().find_map(|(known, flag)| name.eq_ignore_ascii_case(known).then_some(flag)).ok_or(FlagError::UnknownSystemFlag)
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Seen => "\\Seen",
            Self::Answered => "\\Answered",
            Self::Flagged => "\\Flagged",
            Self::Deleted => "\\Deleted",
            Self::Draft => "\\Draft",
            Self::Keyword(keyword) => keyword.as_str(),
        })
    }
}

/// The flags of one message: the five system flags and at most [`Flags::MAX_KEYWORDS`] keywords.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawFlags")]
pub struct Flags {
    seen: bool,
    answered: bool,
    flagged: bool,
    deleted: bool,
    draft: bool,
    keywords: BTreeSet<Keyword>,
}

/// [`Flags`] as stored, before the keyword cap.
#[derive(Deserialize)]
struct RawFlags {
    seen: bool,
    answered: bool,
    flagged: bool,
    deleted: bool,
    draft: bool,
    keywords: Vec<Keyword>,
}

impl From<RawFlags> for Flags {
    fn from(raw: RawFlags) -> Self {
        let mut flags =
            Self { seen: raw.seen, answered: raw.answered, flagged: raw.flagged, deleted: raw.deleted, draft: raw.draft, keywords: BTreeSet::new() };
        flags.extend(raw.keywords.into_iter().map(Flag::Keyword));
        flags
    }
}

impl Flags {
    /// The most keywords one message keeps; a server that sends more has the rest dropped.
    pub const MAX_KEYWORDS: usize = 64;

    /// Whether no flag is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Whether `\Seen` is set.
    #[must_use]
    pub fn is_seen(&self) -> bool {
        self.seen
    }

    /// Whether `\Flagged` is set.
    #[must_use]
    pub fn is_flagged(&self) -> bool {
        self.flagged
    }

    /// Whether `\Deleted` is set.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted
    }

    /// Whether `flag` is set.
    #[must_use]
    pub fn contains(&self, flag: &Flag) -> bool {
        match flag {
            Flag::Seen => self.seen,
            Flag::Answered => self.answered,
            Flag::Flagged => self.flagged,
            Flag::Deleted => self.deleted,
            Flag::Draft => self.draft,
            Flag::Keyword(keyword) => self.keywords.contains(keyword),
        }
    }

    /// Sets `flag`; `false` when it was set already, or when it is a keyword and the message holds
    /// [`Flags::MAX_KEYWORDS`] of them.
    pub fn insert(&mut self, flag: Flag) -> bool {
        match self.field(&flag) {
            Ok(set) => !std::mem::replace(set, true),
            Err(keyword) => self.keywords.len() < Self::MAX_KEYWORDS && self.keywords.insert(keyword.clone()),
        }
    }

    /// Clears `flag`; `false` when it was not set.
    pub fn remove(&mut self, flag: &Flag) -> bool {
        match self.field(flag) {
            Ok(set) => std::mem::replace(set, false),
            Err(keyword) => self.keywords.remove(keyword),
        }
    }

    /// The flags set: the system flags in the order of [`Flag`], then the keywords in order.
    pub fn iter(&self) -> impl Iterator<Item = Flag> + '_ {
        let system = [
            (self.seen, Flag::Seen),
            (self.answered, Flag::Answered),
            (self.flagged, Flag::Flagged),
            (self.deleted, Flag::Deleted),
            (self.draft, Flag::Draft),
        ];
        system.into_iter().filter_map(|(set, flag)| set.then_some(flag)).chain(self.keywords.iter().cloned().map(Flag::Keyword))
    }

    /// The field of a system flag, or the keyword.
    fn field<'f>(&mut self, flag: &'f Flag) -> Result<&mut bool, &'f Keyword> {
        match flag {
            Flag::Seen => Ok(&mut self.seen),
            Flag::Answered => Ok(&mut self.answered),
            Flag::Flagged => Ok(&mut self.flagged),
            Flag::Deleted => Ok(&mut self.deleted),
            Flag::Draft => Ok(&mut self.draft),
            Flag::Keyword(keyword) => Err(keyword),
        }
    }
}

impl Extend<Flag> for Flags {
    fn extend<T: IntoIterator<Item = Flag>>(&mut self, flags: T) {
        for flag in flags {
            self.insert(flag);
        }
    }
}

impl FromIterator<Flag> for Flags {
    fn from_iter<T: IntoIterator<Item = Flag>>(flags: T) -> Self {
        let mut set = Self::default();
        set.extend(flags);
        set
    }
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod tests;
