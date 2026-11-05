// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Folder roles: which folder is the inbox, the sent folder, the trash.
//!
//! A role comes from the folder's SPECIAL-USE attribute first, then from its name (English and Hungarian), then from
//! the owner, asked once ([`RoleMap::detect`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Folder, FolderId, FolderListing, SpecialUse};

/// What a folder is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Role {
    /// The inbox.
    Inbox,
    /// Copies of sent mail.
    Sent,
    /// Unfinished mail.
    Drafts,
    /// Deleted mail.
    Trash,
    /// Spam.
    Junk,
    /// Kept mail.
    Archive,
    /// Every message (a virtual folder; SPECIAL-USE only).
    All,
    /// The flagged messages (a virtual folder; SPECIAL-USE only).
    Flagged,
    /// Our own `InboxCraft/Screener`: first mail from unknown senders.
    Screener,
    /// Our own `InboxCraft/Snoozed`.
    Snoozed,
    /// Our own `InboxCraft/Blocked`.
    Blocked,
}

impl Role {
    /// The roles the owner is asked about when no folder has them.
    pub const ASKED: [Self; 5] = [Self::Sent, Self::Drafts, Self::Trash, Self::Junk, Self::Archive];

    /// Whether the role stays with its folder once set: the inbox, sent and drafts, and our own folders. Trash, junk,
    /// archive and the virtual folders can be changed.
    #[must_use]
    pub const fn is_fixed(self) -> bool {
        matches!(self, Self::Inbox | Self::Sent | Self::Drafts) || self.is_hidden()
    }

    /// Whether the role is one of our own hidden folders.
    #[must_use]
    pub const fn is_hidden(self) -> bool {
        matches!(self, Self::Screener | Self::Snoozed | Self::Blocked)
    }

    /// The role's English name, for logs and errors (UI text goes through the i18n macros).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Sent => "Sent",
            Self::Drafts => "Drafts",
            Self::Trash => "Trash",
            Self::Junk => "Junk",
            Self::Archive => "Archive",
            Self::All => "All",
            Self::Flagged => "Flagged",
            Self::Screener => "Screener",
            Self::Snoozed => "Snoozed",
            Self::Blocked => "Blocked",
        }
    }

    fn from_special_use(special_use: SpecialUse) -> Self {
        match special_use {
            SpecialUse::All => Self::All,
            SpecialUse::Archive => Self::Archive,
            SpecialUse::Drafts => Self::Drafts,
            SpecialUse::Flagged => Self::Flagged,
            SpecialUse::Junk => Self::Junk,
            SpecialUse::Sent => Self::Sent,
            SpecialUse::Trash => Self::Trash,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Where a role came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleSource {
    /// The folder's SPECIAL-USE attribute.
    SpecialUse,
    /// The folder's name.
    Name,
    /// The owner's choice.
    Owner,
}

/// The folder names, without case, that give a role: English, then Hungarian. Matched against the whole last part of
/// the path, so `INBOX.Sent` and `[Mail]/Sent Mail` are sent folders and `Sent/2026` is not.
const NAMES: &[(Role, &[&str])] = &[
    (Role::Sent, &["sent", "sent items", "sent mail", "sent messages"]),
    (Role::Sent, &["elküldött", "elküldött elemek", "elküldött levelek", "elküldött üzenetek", "küldött", "küldött elemek", "küldött levelek"]),
    (Role::Drafts, &["drafts", "draft"]),
    (Role::Drafts, &["piszkozatok", "piszkozat", "vázlatok"]),
    (Role::Trash, &["trash", "deleted", "deleted items", "deleted messages", "bin"]),
    (Role::Trash, &["kuka", "törölt", "törölt elemek", "törölt levelek", "törölt üzenetek", "lomtár"]),
    (Role::Junk, &["junk", "spam", "junk e-mail", "junk email", "junk mail", "bulk mail"]),
    (Role::Junk, &["levélszemét", "levélszemét e-mail", "kéretlen", "kéretlen levelek", "kéretlen e-mail"]),
    (Role::Archive, &["archive", "archives"]),
    (Role::Archive, &["archívum", "archív", "archivált"]),
];

/// The parent folder of our own hidden folders, and their names under it.
const OWN_PARENT: &str = "InboxCraft";
const OWN: [(Role, &str); 3] = [(Role::Screener, "Screener"), (Role::Snoozed, "Snoozed"), (Role::Blocked, "Blocked")];

/// The role the protocol or we ourselves give a path: the inbox, or one of our hidden folders (`InboxCraft/<name>` at
/// the top or under the inbox, in our own spelling).
fn own_role(folder: &FolderListing) -> Option<Role> {
    let path = folder.path();
    if path.is_inbox() {
        return Some(Role::Inbox);
    }
    let parts: Vec<&str> = path.components().take(4).collect();
    let (parent, leaf) = match parts.as_slice() {
        [parent, leaf] => (*parent, *leaf),
        [inbox, parent, leaf] if inbox.eq_ignore_ascii_case("INBOX") => (*parent, *leaf),
        _ => return None,
    };
    if parent != OWN_PARENT {
        return None;
    }
    OWN.into_iter().find_map(|(role, name)| (leaf == name).then_some(role))
}

fn special_use_role(folder: &FolderListing) -> Option<Role> {
    folder.special_use().first().map(|special_use| Role::from_special_use(*special_use))
}

fn name_role(folder: &FolderListing) -> Option<Role> {
    let leaf = folder.path().leaf().trim().to_lowercase();
    NAMES.iter().find_map(|(role, names)| names.contains(&leaf.as_str()).then_some(*role))
}

/// The role one folder has on its own, and where it comes from: the inbox and our own folders by their names, then
/// the first SPECIAL-USE attribute, then the English and Hungarian names. A folder that cannot hold mail has none.
///
/// ```
/// use inboxcraft_model::{FolderListing, FolderPath, Role, RoleSource, detect_role};
///
/// let kuka = FolderListing::new(FolderPath::new("Kuka", Some('/'))?, vec![], true);
/// assert_eq!(detect_role(&kuka), Some((Role::Trash, RoleSource::Name)));
/// # Ok::<(), inboxcraft_model::FolderError>(())
/// ```
#[must_use]
pub fn detect_role(folder: &FolderListing) -> Option<(Role, RoleSource)> {
    if !folder.is_selectable() {
        return None;
    }
    own_role(folder)
        .map(|role| (role, RoleSource::Name))
        .or_else(|| special_use_role(folder).map(|role| (role, RoleSource::SpecialUse)))
        .or_else(|| name_role(folder).map(|role| (role, RoleSource::Name)))
}

/// Why the owner's choice of a role was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleError {
    /// The role is fixed once set, and it is set.
    Fixed(Role),
    /// The role is one of our own folders.
    OurOwn(Role),
    /// The folder has another role already.
    FolderTaken(Role),
}

impl fmt::Display for RoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(role) => write!(f, "the {role} folder is fixed once set"),
            Self::OurOwn(role) => write!(f, "the {role} folder is InboxCraft's own"),
            Self::FolderTaken(role) => write!(f, "the folder is the {role} folder already"),
        }
    }
}

impl std::error::Error for RoleError {}

/// A role's folder and where the role came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct RoleEntry {
    folder: FolderId,
    source: RoleSource,
}

/// The roles of one account's folders, and which roles the owner was asked about.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawRoleMap")]
pub struct RoleMap {
    entries: BTreeMap<Role, RoleEntry>,
    asked: BTreeSet<Role>,
}

/// A [`RoleMap`] as stored, before it is checked.
#[derive(Deserialize)]
struct RawRoleMap {
    entries: BTreeMap<Role, RoleEntry>,
    asked: BTreeSet<Role>,
}

impl From<RawRoleMap> for RoleMap {
    /// Keeps what [`RoleMap`] itself allows: the inbox and our own folders by name only, and one role per folder (the
    /// first in [`Role`] order).
    fn from(raw: RawRoleMap) -> Self {
        let mut map = Self { entries: BTreeMap::new(), asked: raw.asked };
        for (role, entry) in raw.entries {
            if own_or_inbox(role) && entry.source != RoleSource::Name {
                continue;
            }
            if map.role_of(entry.folder).is_none() {
                map.entries.insert(role, entry);
            }
        }
        map
    }
}

impl RoleMap {
    /// Brings the map up to date with the account's folder list and returns the roles to ask the owner about now.
    ///
    /// A role whose folder is gone or can no longer hold mail is freed. Then every free role goes to the first folder
    /// that has it by SPECIAL-USE, else to the first that has it by name ([`detect_role`]); a folder takes one role,
    /// and a role that is set is never moved. Every role of [`Role::ASKED`] still free and never asked about is
    /// returned, once: the next call does not return it again.
    pub fn detect(&mut self, folders: &[Folder]) -> Vec<Role> {
        let alive = |id: FolderId| folders.iter().any(|folder| folder.id() == id && folder.listing().is_selectable());
        self.entries.retain(|_, entry| alive(entry.folder));
        let detected: Vec<(FolderId, Role, RoleSource)> =
            folders.iter().filter_map(|folder| detect_role(folder.listing()).map(|(role, source)| (folder.id(), role, source))).collect();
        // The inbox, our own folders and SPECIAL-USE first, the names after.
        let (first, names): (Vec<_>, Vec<_>) =
            detected.into_iter().partition(|(_, role, source)| *source == RoleSource::SpecialUse || own_or_inbox(*role));
        for (folder, role, source) in first.into_iter().chain(names) {
            if !self.entries.contains_key(&role) && self.role_of(folder).is_none() {
                self.entries.insert(role, RoleEntry { folder, source });
            }
        }
        let questions: Vec<Role> = Role::ASKED.into_iter().filter(|role| !self.entries.contains_key(role) && !self.asked.contains(role)).collect();
        self.asked.extend(questions.iter().copied());
        questions
    }

    /// The owner's answer: `folder` has `role`.
    ///
    /// # Errors
    ///
    /// A [`RoleError`] when the role is our own, fixed and set, or the folder has another role.
    pub fn choose(&mut self, role: Role, folder: FolderId) -> Result<(), RoleError> {
        self.changeable(role)?;
        match self.role_of(folder) {
            Some(other) if other != role => return Err(RoleError::FolderTaken(other)),
            _ => {}
        }
        self.entries.insert(role, RoleEntry { folder, source: RoleSource::Owner });
        self.asked.insert(role);
        Ok(())
    }

    /// The owner's answer: no folder has `role`.
    ///
    /// # Errors
    ///
    /// A [`RoleError`] when the role is our own, or fixed and set.
    pub fn clear(&mut self, role: Role) -> Result<(), RoleError> {
        self.changeable(role)?;
        self.entries.remove(&role);
        self.asked.insert(role);
        Ok(())
    }

    fn changeable(&self, role: Role) -> Result<(), RoleError> {
        if role.is_hidden() {
            return Err(RoleError::OurOwn(role));
        }
        if role.is_fixed() && self.entries.contains_key(&role) {
            return Err(RoleError::Fixed(role));
        }
        Ok(())
    }

    /// The folder with `role`.
    #[must_use]
    pub fn folder(&self, role: Role) -> Option<FolderId> {
        self.entries.get(&role).map(|entry| entry.folder)
    }

    /// Where `role` came from.
    #[must_use]
    pub fn source(&self, role: Role) -> Option<RoleSource> {
        self.entries.get(&role).map(|entry| entry.source)
    }

    /// The role of `folder`.
    #[must_use]
    pub fn role_of(&self, folder: FolderId) -> Option<Role> {
        self.entries.iter().find_map(|(role, entry)| (entry.folder == folder).then_some(*role))
    }

    /// Whether the owner was asked about `role`.
    #[must_use]
    pub fn was_asked(&self, role: Role) -> bool {
        self.asked.contains(&role)
    }
}

fn own_or_inbox(role: Role) -> bool {
    role == Role::Inbox || role.is_hidden()
}

#[cfg(test)]
#[path = "role_tests.rs"]
mod tests;
