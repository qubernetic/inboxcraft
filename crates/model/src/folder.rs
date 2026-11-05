// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Folders: their paths on the server and the SPECIAL-USE attributes (RFC 6154) the server gives them.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AccountId, FolderId};

/// Why a folder name is not usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderError {
    /// The name is empty.
    Empty,
    /// The name is longer than [`FolderPath::MAX_LEN`] bytes.
    TooLong,
    /// The name holds a control character.
    ControlCharacter,
    /// The hierarchy delimiter is a control character.
    BadDelimiter,
}

impl fmt::Display for FolderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "the folder name is empty",
            Self::TooLong => "the folder name is longer than 1024 bytes",
            Self::ControlCharacter => "the folder name holds a control character",
            Self::BadDelimiter => "the folder delimiter is a control character",
        })
    }
}

impl std::error::Error for FolderError {}

/// A folder's full name on the server, decoded to UTF-8 (`proto` undoes the modified UTF-7 of RFC 3501), and the
/// hierarchy delimiter the server uses (`None` for a flat server).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawPath")]
pub struct FolderPath {
    name: String,
    delimiter: Option<char>,
}

/// A [`FolderPath`] as stored, before it is checked.
#[derive(Deserialize)]
struct RawPath {
    name: String,
    delimiter: Option<char>,
}

impl TryFrom<RawPath> for FolderPath {
    type Error = FolderError;

    fn try_from(raw: RawPath) -> Result<Self, Self::Error> {
        Self::new(&raw.name, raw.delimiter)
    }
}

impl FolderPath {
    /// The longest name kept, in bytes.
    pub const MAX_LEN: usize = 1024;

    /// The folder `name` with hierarchy `delimiter`.
    ///
    /// # Errors
    ///
    /// A [`FolderError`].
    pub fn new(name: &str, delimiter: Option<char>) -> Result<Self, FolderError> {
        if name.is_empty() {
            return Err(FolderError::Empty);
        }
        if name.len() > Self::MAX_LEN {
            return Err(FolderError::TooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(FolderError::ControlCharacter);
        }
        if delimiter.is_some_and(char::is_control) {
            return Err(FolderError::BadDelimiter);
        }
        Ok(Self { name: name.to_owned(), delimiter })
    }

    /// The full name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The hierarchy delimiter.
    #[must_use]
    pub fn delimiter(&self) -> Option<char> {
        self.delimiter
    }

    /// The parts of the name between delimiters, from the top.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        let (name, delimiter) = (self.name.as_str(), self.delimiter);
        let mut rest = Some(name);
        std::iter::from_fn(move || {
            let text = rest?;
            match delimiter.and_then(|d| text.split_once(d)) {
                Some((head, tail)) => {
                    rest = Some(tail);
                    Some(head)
                }
                None => rest.take(),
            }
        })
    }

    /// The last part of the name.
    #[must_use]
    pub fn leaf(&self) -> &str {
        self.components().last().unwrap_or(&self.name)
    }

    /// Whether this is the inbox: `INBOX` in any case, at the top (RFC 9051 section 5.1).
    #[must_use]
    pub fn is_inbox(&self) -> bool {
        self.name.eq_ignore_ascii_case("INBOX")
    }
}

impl fmt::Display for FolderPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// A SPECIAL-USE attribute (RFC 6154 section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SpecialUse {
    /// `\All`: every message, a virtual folder.
    All,
    /// `\Archive`.
    Archive,
    /// `\Drafts`.
    Drafts,
    /// `\Flagged`: the flagged messages, a virtual folder.
    Flagged,
    /// `\Junk`.
    Junk,
    /// `\Sent`.
    Sent,
    /// `\Trash`.
    Trash,
}

impl SpecialUse {
    const ALL: [(Self, &'static str); 7] = [
        (Self::All, "\\All"),
        (Self::Archive, "\\Archive"),
        (Self::Drafts, "\\Drafts"),
        (Self::Flagged, "\\Flagged"),
        (Self::Junk, "\\Junk"),
        (Self::Sent, "\\Sent"),
        (Self::Trash, "\\Trash"),
    ];

    /// The SPECIAL-USE meaning of a LIST attribute (`\Sent`, without case), or `None` for any other attribute.
    #[must_use]
    pub fn from_attribute(attribute: &str) -> Option<Self> {
        Self::ALL.into_iter().find_map(|(special_use, text)| attribute.eq_ignore_ascii_case(text).then_some(special_use))
    }
}

/// A folder as the server lists it: its path, its SPECIAL-USE attributes and whether it can hold messages (no
/// `\Noselect` or `\NonExistent`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawListing")]
pub struct FolderListing {
    path: FolderPath,
    special_use: Vec<SpecialUse>,
    selectable: bool,
}

/// A [`FolderListing`] as stored.
#[derive(Deserialize)]
struct RawListing {
    path: FolderPath,
    special_use: Vec<SpecialUse>,
    selectable: bool,
}

impl From<RawListing> for FolderListing {
    fn from(raw: RawListing) -> Self {
        Self::new(raw.path, raw.special_use, raw.selectable)
    }
}

impl FolderListing {
    /// The listing of `path`; each SPECIAL-USE attribute is kept once, in the server's order.
    #[must_use]
    pub fn new(path: FolderPath, special_use: Vec<SpecialUse>, selectable: bool) -> Self {
        let mut kept: Vec<SpecialUse> = Vec::new();
        for attribute in special_use {
            if !kept.contains(&attribute) {
                kept.push(attribute);
            }
        }
        Self { path, special_use: kept, selectable }
    }

    /// The path.
    #[must_use]
    pub fn path(&self) -> &FolderPath {
        &self.path
    }

    /// The SPECIAL-USE attributes, in the server's order.
    #[must_use]
    pub fn special_use(&self) -> &[SpecialUse] {
        &self.special_use
    }

    /// Whether the folder can hold messages.
    #[must_use]
    pub fn is_selectable(&self) -> bool {
        self.selectable
    }
}

/// A folder of one of the owner's accounts: our ids and the server's listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    id: FolderId,
    account: AccountId,
    listing: FolderListing,
}

impl Folder {
    /// The folder `id` of `account`, as `listing` describes it.
    #[must_use]
    pub fn new(id: FolderId, account: AccountId, listing: FolderListing) -> Self {
        Self { id, account, listing }
    }

    /// Our id.
    #[must_use]
    pub fn id(&self) -> FolderId {
        self.id
    }

    /// The account.
    #[must_use]
    pub fn account(&self) -> AccountId {
        self.account
    }

    /// The server's listing.
    #[must_use]
    pub fn listing(&self) -> &FolderListing {
        &self.listing
    }
}

#[cfg(test)]
#[path = "folder_tests.rs"]
mod tests;
