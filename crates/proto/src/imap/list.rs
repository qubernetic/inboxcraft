// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! LIST responses (RFC 9051 section 7.3.1) with SPECIAL-USE attributes (RFC 6154).
//!
//! The reading of one response follows UCS-Care-Group/mailcraft `crates/proto/src/imap/mod.rs` (`mailbox_from`, see
//! `PROVENANCE.md`).

use inboxcraft_model::{FolderError, FolderListing, FolderPath, SpecialUse};

use super::mailbox::flag_list;
use super::utf7::decode_utf7;
use super::value::Value;

/// Most mailboxes one LIST keeps; the ones after are read and dropped.
pub const MAX_MAILBOXES: usize = 10_000;
/// Longest mailbox name kept, in bytes as the server wrote it. A mailbox with a longer one is left out: a name cut
/// short would name another mailbox or none.
pub const MAX_MAILBOX_NAME: usize = 1_024;

/// One mailbox as LIST names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListEntry {
    /// The name exactly as the server wrote it: what SELECT and EXAMINE take.
    pub raw: String,
    /// The name decoded from modified UTF-7, for people.
    pub name: String,
    /// The hierarchy delimiter, `None` for a flat name space.
    pub delimiter: Option<char>,
    /// The attributes as the server wrote them (`\HasNoChildren`, `\Sent`), at most 64.
    pub attributes: Vec<String>,
    /// The SPECIAL-USE attributes among them, each once.
    pub special_use: Vec<SpecialUse>,
}

impl ListEntry {
    /// Whether the mailbox can be selected: no `\Noselect` and no `\NonExistent`.
    #[must_use]
    pub fn is_selectable(&self) -> bool {
        !self.attributes.iter().any(|a| a.eq_ignore_ascii_case("\\Noselect") || a.eq_ignore_ascii_case("\\NonExistent"))
    }

    /// The mailbox as the model lists a folder.
    ///
    /// # Errors
    ///
    /// The model's [`FolderError`] when the decoded name is not a folder name it keeps.
    pub fn listing(&self) -> Result<FolderListing, FolderError> {
        let path = FolderPath::new(&self.name, self.delimiter)?;
        Ok(FolderListing::new(path, self.special_use.clone(), self.is_selectable()))
    }

    /// From the values of `LIST (attributes) delimiter name [extended data]`; `None` when they are not that.
    pub(crate) fn from_values(values: Vec<Value>) -> Option<Self> {
        let mut values = values.into_iter();
        if !values.next()?.atom()?.eq_ignore_ascii_case("LIST") {
            return None;
        }
        let Value::List(attributes) = values.next()? else { return None };
        let delimiter = match values.next()? {
            Value::Str(d) => match d.as_slice() {
                [b] if b.is_ascii() && !b.is_ascii_control() => Some(char::from(*b)),
                _ => return None,
            },
            Value::Atom(nil) if nil.eq_ignore_ascii_case("NIL") => None,
            _ => return None,
        };
        let raw = values.next()?.into_text()?;
        if raw.is_empty() || raw.len() > MAX_MAILBOX_NAME || raw.chars().any(|c| c.is_control()) {
            return None;
        }
        let attributes = flag_list(attributes);
        let mut special_use: Vec<SpecialUse> = Vec::new();
        for use_ in attributes.iter().filter_map(|a| SpecialUse::from_attribute(a)) {
            if !special_use.contains(&use_) {
                special_use.push(use_);
            }
        }
        let name = if raw.eq_ignore_ascii_case("INBOX") { "INBOX".to_owned() } else { decode_utf7(&raw) };
        Some(Self { raw, name, delimiter, attributes, special_use })
    }
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
