// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Accounts and their servers.
//!
//! An account holds no secret: the password or token lives in the OS credential store, found by the account's id.

use std::fmt;
use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};

use crate::address::clean_name;
use crate::{AccountId, EmailAddress};

/// Why an account setting is not usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountError {
    /// The server is neither a host name nor an IP address.
    BadHost,
    /// The user name is empty, longer than [`Account::MAX_USERNAME_LEN`] bytes or holds a control character.
    BadUsername,
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadHost => "the server name is not a host name or an IP address",
            Self::BadUsername => "the user name is empty, too long or holds a control character",
        })
    }
}

impl std::error::Error for AccountError {}

/// A server's host: a DNS name (Unicode labels allowed), an IPv4 address, or an IPv6 address in brackets.
///
/// Compared without case; a final dot is dropped.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Host(String);

impl Host {
    /// The longest host name, in bytes (RFC 1035 section 2.3.4, without the final dot).
    pub const MAX_LEN: usize = 253;

    /// The host `text`, spaces around it ignored.
    ///
    /// # Errors
    ///
    /// [`AccountError::BadHost`].
    pub fn new(text: &str) -> Result<Self, AccountError> {
        let text = text.trim_matches(' ');
        let text = text.strip_suffix('.').unwrap_or(text);
        let valid = if let Some(literal) = text.strip_prefix('[') {
            literal.strip_suffix(']').is_some_and(|inner| !inner.is_empty() && inner.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.'))
        } else {
            !text.is_empty() && text.len() <= Self::MAX_LEN && text.split('.').all(valid_label)
        };
        if !valid {
            return Err(AccountError::BadHost);
        }
        Ok(Self(text.to_owned()))
    }

    /// The host as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One label of a host name: 1 to 63 bytes of letters, digits, `-` and `_`, not starting or ending with `-`.
fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

impl TryFrom<String> for Host {
    type Error = AccountError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(&text)
    }
}

impl From<Host> for String {
    fn from(host: Host) -> Self {
        host.0
    }
}

impl PartialEq for Host {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_lowercase() == other.0.to_lowercase()
    }
}

impl Eq for Host {}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How the connection is encrypted. There is no unencrypted choice: encrypt or refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Security {
    /// TLS from the first byte (IMAP 993, submission 465; RFC 8314).
    Tls,
    /// A plain connection upgraded with STARTTLS before anything else is sent; refused if the server does not offer it.
    StartTls,
}

/// A port number as a constant (no literal below is zero, so the `MIN` arm is never taken).
macro_rules! port {
    ($number:literal) => {
        match NonZeroU16::new($number) {
            Some(port) => port,
            None => NonZeroU16::MIN,
        }
    };
}

const IMAPS: NonZeroU16 = port!(993);
const IMAP: NonZeroU16 = port!(143);
const SUBMISSIONS: NonZeroU16 = port!(465);
const SUBMISSION: NonZeroU16 = port!(587);

impl Security {
    /// The usual IMAP port.
    #[must_use]
    pub const fn imap_port(self) -> NonZeroU16 {
        match self {
            Self::Tls => IMAPS,
            Self::StartTls => IMAP,
        }
    }

    /// The usual submission (SMTP) port.
    #[must_use]
    pub const fn submission_port(self) -> NonZeroU16 {
        match self {
            Self::Tls => SUBMISSIONS,
            Self::StartTls => SUBMISSION,
        }
    }
}

/// A server: host, port and encryption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    host: Host,
    port: NonZeroU16,
    security: Security,
}

impl Server {
    /// The server at `host` and `port`, encrypted with `security`.
    #[must_use]
    pub fn new(host: Host, port: NonZeroU16, security: Security) -> Self {
        Self { host, port, security }
    }

    /// The host.
    #[must_use]
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// The port.
    #[must_use]
    pub fn port(&self) -> NonZeroU16 {
        self.port
    }

    /// The encryption.
    #[must_use]
    pub fn security(&self) -> Security {
        self.security
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let security = match self.security {
            Security::Tls => "TLS",
            Security::StartTls => "STARTTLS",
        };
        write!(f, "{}:{} ({security})", self.host, self.port)
    }
}

/// One of the owner's mail accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawAccount")]
pub struct Account {
    id: AccountId,
    label: String,
    address: EmailAddress,
    display_name: Option<String>,
    username: String,
    imap: Server,
    submission: Option<Server>,
}

/// An [`Account`] as stored, before it is checked.
#[derive(Deserialize)]
struct RawAccount {
    id: AccountId,
    label: String,
    address: EmailAddress,
    display_name: Option<String>,
    username: String,
    imap: Server,
    submission: Option<Server>,
}

impl TryFrom<RawAccount> for Account {
    type Error = AccountError;

    fn try_from(raw: RawAccount) -> Result<Self, Self::Error> {
        Ok(Self::new(raw.id, &raw.label, raw.address, &raw.username, raw.imap)?
            .with_display_name(raw.display_name.as_deref())
            .with_submission(raw.submission))
    }
}

impl Account {
    /// The longest user name, in bytes.
    pub const MAX_USERNAME_LEN: usize = 320;

    /// The account `id` for `address`, shown as `label` (cleaned like a display name; empty means the address),
    /// logging in to `imap` as `username`.
    ///
    /// # Errors
    ///
    /// [`AccountError::BadUsername`].
    pub fn new(id: AccountId, label: &str, address: EmailAddress, username: &str, imap: Server) -> Result<Self, AccountError> {
        if username.is_empty() || username.len() > Self::MAX_USERNAME_LEN || username.chars().any(char::is_control) {
            return Err(AccountError::BadUsername);
        }
        let label = clean_name(label).unwrap_or_else(|| address.as_str().to_owned());
        Ok(Self { id, label, address, display_name: None, username: username.to_owned(), imap, submission: None })
    }

    /// The same account with the owner's name for sent mail, cleaned like any display name.
    #[must_use]
    pub fn with_display_name(mut self, name: Option<&str>) -> Self {
        self.display_name = name.and_then(clean_name);
        self
    }

    /// The same account with a submission (SMTP) server.
    #[must_use]
    pub fn with_submission(mut self, server: Option<Server>) -> Self {
        self.submission = server;
        self
    }

    /// Our id.
    #[must_use]
    pub fn id(&self) -> AccountId {
        self.id
    }

    /// The owner's name for the account.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The account's address.
    #[must_use]
    pub fn address(&self) -> &EmailAddress {
        &self.address
    }

    /// The owner's name on sent mail.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// The login name.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// The IMAP server.
    #[must_use]
    pub fn imap(&self) -> &Server {
        &self.imap
    }

    /// The submission server, if set.
    #[must_use]
    pub fn submission(&self) -> Option<&Server> {
        self.submission.as_ref()
    }
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
