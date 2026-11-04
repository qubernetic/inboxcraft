// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Mail addresses and the people behind them.
//!
//! A contact is its address: two people with the same display name are two contacts, and wherever both could appear
//! they are told apart by the address ([`labels`]).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

/// The longest display name kept, in characters; the rest is cut.
pub const MAX_NAME_CHARS: usize = 256;

/// Why a text is not a mail address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressError {
    /// Nothing but spaces.
    Empty,
    /// Longer than [`EmailAddress::MAX_LEN`] bytes.
    TooLong,
    /// A control character (CR, LF, NUL and the like) anywhere.
    ControlCharacter,
    /// No `@`.
    NoAtSign,
    /// Nothing before the `@`.
    EmptyLocalPart,
    /// The part before the `@` holds white space or a special character outside quotes.
    BadLocalPart,
    /// Nothing after the `@`.
    EmptyDomain,
    /// The part after the `@` is not a domain name or a bracketed address literal.
    BadDomain,
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "the address is empty",
            Self::TooLong => "the address is longer than 254 bytes",
            Self::ControlCharacter => "the address holds a control character",
            Self::NoAtSign => "the address has no @",
            Self::EmptyLocalPart => "the address has nothing before the @",
            Self::BadLocalPart => "the address has white space or a special character before the @",
            Self::EmptyDomain => "the address has nothing after the @",
            Self::BadDomain => "the address has no valid domain after the @",
        })
    }
}

impl std::error::Error for AddressError {}

/// A mail address (`local-part@domain`, RFC 5322 section 3.4.1), checked.
///
/// Kept as written; compared, ordered and hashed without case, so `Anna@Example.org` and `anna@example.org` are one
/// contact. RFC 5321 lets a server tell local parts apart by case; mail servers in use do not, and telling them apart
/// here would split one person in two.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EmailAddress {
    text: String,
    at: usize,
    key: String,
}

impl EmailAddress {
    /// The longest address, in bytes (RFC 5321 section 4.5.3.1.3, a path without its angle brackets).
    pub const MAX_LEN: usize = 254;

    /// The address in `text`, spaces around it ignored.
    ///
    /// # Errors
    ///
    /// An [`AddressError`] naming what is wrong.
    pub fn new(text: &str) -> Result<Self, AddressError> {
        if text.chars().any(char::is_control) {
            return Err(AddressError::ControlCharacter);
        }
        let text = text.trim_matches(' ');
        if text.is_empty() {
            return Err(AddressError::Empty);
        }
        if text.len() > Self::MAX_LEN {
            return Err(AddressError::TooLong);
        }
        let at = text.rfind('@').ok_or(AddressError::NoAtSign)?;
        let (local, domain) = (text.get(..at).unwrap_or_default(), text.get(at.saturating_add(1)..).unwrap_or_default());
        check_local_part(local)?;
        check_domain(domain)?;
        Ok(Self { text: text.to_owned(), at, key: text.to_lowercase() })
    }

    /// The address as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The part before the last `@`.
    #[must_use]
    pub fn local_part(&self) -> &str {
        self.text.get(..self.at).unwrap_or_default()
    }

    /// The part after the last `@`.
    #[must_use]
    pub fn domain(&self) -> &str {
        self.text.get(self.at.saturating_add(1)..).unwrap_or_default()
    }
}

/// Characters that end an unquoted local part or a domain (RFC 5322 `specials`, `@` and `.` aside).
const SPECIALS: &[char] = &['<', '>', '(', ')', '[', ']', ',', ';', ':', '\\', '"', '@'];

fn check_local_part(local: &str) -> Result<(), AddressError> {
    if local.is_empty() {
        return Err(AddressError::EmptyLocalPart);
    }
    let quoted = local.len() >= 2 && local.starts_with('"') && local.ends_with('"');
    if !quoted && local.chars().any(|c| c.is_whitespace() || SPECIALS.contains(&c)) {
        return Err(AddressError::BadLocalPart);
    }
    Ok(())
}

fn check_domain(domain: &str) -> Result<(), AddressError> {
    if domain.is_empty() {
        return Err(AddressError::EmptyDomain);
    }
    if let Some(literal) = domain.strip_prefix('[') {
        let inner = literal.strip_suffix(']').ok_or(AddressError::BadDomain)?;
        if inner.is_empty() || inner.chars().any(|c| c.is_whitespace() || matches!(c, '[' | ']' | '\\')) {
            return Err(AddressError::BadDomain);
        }
        return Ok(());
    }
    let bad_label = |label: &str| label.is_empty() || label.chars().any(|c| c.is_whitespace() || SPECIALS.contains(&c));
    if domain.split('.').any(bad_label) {
        return Err(AddressError::BadDomain);
    }
    Ok(())
}

impl TryFrom<String> for EmailAddress {
    type Error = AddressError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(&text)
    }
}

impl From<EmailAddress> for String {
    fn from(address: EmailAddress) -> Self {
        address.text
    }
}

impl PartialEq for EmailAddress {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for EmailAddress {}

impl PartialOrd for EmailAddress {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EmailAddress {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

impl Hash for EmailAddress {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// A person or a list as a message names them: an optional display name and the address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "RawAddress")]
pub struct Address {
    name: Option<String>,
    email: EmailAddress,
}

/// An [`Address`] as stored, before its name is cleaned.
#[derive(Deserialize)]
struct RawAddress {
    name: Option<String>,
    email: EmailAddress,
}

impl From<RawAddress> for Address {
    fn from(raw: RawAddress) -> Self {
        Self::new(raw.name.as_deref(), raw.email)
    }
}

impl Address {
    /// `email` with the display name `name`, cleaned: white space (CR and LF included) becomes one space, other
    /// control characters and the invisible direction and joiner marks are dropped, the name is cut at
    /// [`MAX_NAME_CHARS`] characters, and a name left empty is none.
    #[must_use]
    pub fn new(name: Option<&str>, email: EmailAddress) -> Self {
        Self { name: name.and_then(clean_name), email }
    }

    /// The same address with another display name, cleaned as in [`Address::new`].
    #[must_use]
    pub fn with_name(self, name: Option<&str>) -> Self {
        Self::new(name, self.email)
    }

    /// The display name, if there is one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The address.
    #[must_use]
    pub fn email(&self) -> &EmailAddress {
        &self.email
    }

    /// The display name when it alone is safe to show: it is set, it is not an address other than this one.
    fn plain_name(&self) -> Option<&str> {
        let name = self.name.as_deref()?;
        if name.contains('@') && name.to_lowercase() != self.email.key {
            return None;
        }
        Some(name)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{name} <{}>", self.email),
            None => write!(f, "{}", self.email),
        }
    }
}

/// A mark that changes how text runs or joins without showing itself (bidi controls, zero-width characters).
fn is_invisible_mark(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

pub(crate) fn clean_name(name: &str) -> Option<String> {
    let mut cleaned = String::new();
    let mut kept = 0_usize;
    let mut space = false;
    for c in name.chars() {
        if kept >= MAX_NAME_CHARS {
            break;
        }
        if c.is_whitespace() {
            space = !cleaned.is_empty();
            continue;
        }
        if c.is_control() || is_invisible_mark(c) {
            continue;
        }
        if space {
            cleaned.push(' ');
            kept = kept.saturating_add(1);
            space = false;
            if kept >= MAX_NAME_CHARS {
                break;
            }
        }
        cleaned.push(c);
        kept = kept.saturating_add(1);
    }
    let cleaned = cleaned.trim_end().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// How to show each address of `addresses` where they appear together (suggestions, rows): the display name alone,
/// unless another address in the list carries the same name (without case), or the name looks like an address that
/// is not this one; then `Name <address>`. An address without a name shows as the address.
#[must_use]
pub fn labels(addresses: &[Address]) -> Vec<String> {
    // Each name (without case) and the one address that carries it, or `None` once a second address does.
    let mut owners: HashMap<String, Option<&EmailAddress>> = HashMap::new();
    for address in addresses {
        if let Some(name) = &address.name {
            owners
                .entry(name.to_lowercase())
                .and_modify(|owner| {
                    if owner.is_some_and(|email| *email != address.email) {
                        *owner = None;
                    }
                })
                .or_insert(Some(&address.email));
        }
    }
    addresses
        .iter()
        .map(|address| match address.plain_name() {
            Some(name) if owners.get(&name.to_lowercase()).is_some_and(Option::is_some) => name.to_owned(),
            _ => address.to_string(),
        })
        .collect()
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
