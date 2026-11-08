// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Accounts and their servers.
//!
//! An account holds no secret: the password or token lives in the OS credential store, found by the account's id.

use std::fmt;
use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};

use crate::address::clean_name;
use crate::text::clean_line;
use crate::{AccountId, EmailAddress};

/// Why an account setting is not usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountError {
    /// The server is neither a host name nor an IP address.
    BadHost,
    /// The user name is empty, longer than [`Account::MAX_USERNAME_LEN`] bytes or holds a control character.
    BadUsername,
    /// The calendar (CalDAV) address is not an `https` URL with a host, at most [`Account::MAX_CALDAV_LEN`] bytes,
    /// without a user name, white space or control characters.
    BadCalendarUrl,
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadHost => "the server name is not a host name or an IP address",
            Self::BadUsername => "the user name is empty, too long or holds a control character",
            Self::BadCalendarUrl => "the calendar address is not an https URL",
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
    signature: Option<String>,
    grouped: bool,
    caldav: Option<String>,
    templates: Vec<Template>,
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
    #[serde(default)]
    signature: Option<String>,
    #[serde(default = "on")]
    grouped: bool,
    #[serde(default)]
    caldav: Option<String>,
    #[serde(default)]
    templates: Vec<RawTemplate>,
}

/// A record written before the grouped inbox has it on.
const fn on() -> bool {
    true
}

impl TryFrom<RawAccount> for Account {
    type Error = AccountError;

    fn try_from(raw: RawAccount) -> Result<Self, Self::Error> {
        Self::new(raw.id, &raw.label, raw.address, &raw.username, raw.imap)?
            .with_display_name(raw.display_name.as_deref())
            .with_submission(raw.submission)
            .with_signature(raw.signature.as_deref())
            .with_grouped(raw.grouped)
            .with_templates(raw.templates.iter().filter_map(|template| Template::new(&template.name, &template.subject, &template.text)).collect())
            .with_caldav(raw.caldav.as_deref())
    }
}

impl Account {
    /// The longest user name, in bytes.
    pub const MAX_USERNAME_LEN: usize = 320;

    /// The longest signature, in characters; a longer one is cut.
    pub const MAX_SIGNATURE_CHARS: usize = 2_000;

    /// The longest calendar address, in bytes.
    pub const MAX_CALDAV_LEN: usize = 2_048;

    /// The most templates an account keeps.
    pub const MAX_TEMPLATES: usize = 100;

    /// The account `id` for `address`, shown as `label` (cleaned like a display name; empty means the address),
    /// logging in to `imap` as `username`.
    ///
    /// # Errors
    ///
    /// [`AccountError::BadUsername`].
    pub fn new(id: AccountId, label: &str, address: EmailAddress, username: &str, imap: Server) -> Result<Self, AccountError> {
        Self::check_username(username)?;
        let label = clean_name(label).unwrap_or_else(|| address.as_str().to_owned());
        Ok(Self {
            id,
            label,
            address,
            display_name: None,
            username: username.to_owned(),
            imap,
            submission: None,
            signature: None,
            grouped: true,
            caldav: None,
            templates: Vec::new(),
        })
    }

    fn check_username(username: &str) -> Result<(), AccountError> {
        if username.is_empty() || username.len() > Self::MAX_USERNAME_LEN || username.chars().any(char::is_control) {
            return Err(AccountError::BadUsername);
        }
        Ok(())
    }

    /// The same account shown as `label` (cleaned like a display name; empty means the address), logging in to `imap`
    /// as `username`, checked as [`Self::new`] checks them. Everything else stays.
    ///
    /// # Errors
    ///
    /// [`AccountError::BadUsername`].
    pub fn with_sign_in(mut self, label: &str, username: &str, imap: Server) -> Result<Self, AccountError> {
        Self::check_username(username)?;
        self.label = clean_name(label).unwrap_or_else(|| self.address.as_str().to_owned());
        username.clone_into(&mut self.username);
        self.imap = imap;
        Ok(self)
    }

    /// Whether [`Self::with_caldav`] takes `url`: blank (none), or an `https` address without a user name.
    #[must_use]
    pub fn takes_caldav(url: &str) -> bool {
        let url = url.trim();
        url.is_empty() || is_https_url(url)
    }

    /// The same account with the owner's name for sent mail, cleaned like any display name.
    #[must_use]
    pub fn with_display_name(mut self, name: Option<&str>) -> Self {
        self.display_name = name.and_then(clean_name);
        self
    }

    /// The same account with the signature new drafts get: line breaks become `\n`, other control characters but
    /// tabs are dropped, white space at the end goes, and it is cut at [`Self::MAX_SIGNATURE_CHARS`]; a blank one is
    /// none.
    #[must_use]
    pub fn with_signature(mut self, signature: Option<&str>) -> Self {
        self.signature = signature.and_then(|text| {
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            let kept: String = text.chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).take(Self::MAX_SIGNATURE_CHARS).collect();
            let kept = kept.trim_end();
            if kept.trim_start().is_empty() { None } else { Some(kept.to_owned()) }
        });
        self
    }

    /// The same account with `templates`: the first of each name (without case), at most [`Self::MAX_TEMPLATES`].
    #[must_use]
    pub fn with_templates(mut self, templates: Vec<Template>) -> Self {
        let mut kept: Vec<Template> = Vec::new();
        for template in templates {
            if kept.len() < Self::MAX_TEMPLATES && !kept.iter().any(|held| held.name.to_lowercase() == template.name.to_lowercase()) {
                kept.push(template);
            }
        }
        self.templates = kept;
        self
    }

    /// The same account with its inbox grouped (People, Updates, Newsletters, Invitations) or not.
    #[must_use]
    pub fn with_grouped(mut self, grouped: bool) -> Self {
        self.grouped = grouped;
        self
    }

    /// The same account with the address of its calendar (CalDAV) server, where discovery starts (RFC 6764: a bare
    /// host starts at its well-known path). Trimmed; a blank one is none. The password goes there, so it is `https`
    /// only.
    ///
    /// # Errors
    ///
    /// [`AccountError::BadCalendarUrl`].
    pub fn with_caldav(mut self, url: Option<&str>) -> Result<Self, AccountError> {
        self.caldav = match url.map(str::trim).filter(|url| !url.is_empty()) {
            Some(url) if is_https_url(url) => Some(url.to_owned()),
            Some(_) => return Err(AccountError::BadCalendarUrl),
            None => None,
        };
        Ok(self)
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

    /// The signature new drafts get, if set.
    #[must_use]
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }

    /// Whether the account's inbox is grouped; on unless the owner turned it off.
    #[must_use]
    pub fn grouped(&self) -> bool {
        self.grouped
    }

    /// The address of the calendar (CalDAV) server, if set.
    #[must_use]
    pub fn caldav(&self) -> Option<&str> {
        self.caldav.as_deref()
    }

    /// The owner's templates for new text, in the order kept.
    #[must_use]
    pub fn templates(&self) -> &[Template] {
        &self.templates
    }

    /// The template called `name`, without case.
    #[must_use]
    pub fn template(&self, name: &str) -> Option<&Template> {
        let name = name.to_lowercase();
        self.templates.iter().find(|template| template.name.to_lowercase() == name)
    }
}

/// Whether `url` is an `https` URL with a host and no user name, query, fragment, white space or control character,
/// short enough: a token typed into a query would be repeated wherever the address is shown. The host and port follow
/// the rules the CalDAV client parses them by, so an address saved here is one it can use: an ASCII host name or an
/// IPv6 address in brackets, and a port of digits from 1 to 65535.
fn is_https_url(url: &str) -> bool {
    let Some(rest) = url.get(..8).filter(|scheme| scheme.eq_ignore_ascii_case("https://")).and_then(|_| url.get(8..)) else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    url.len() <= Account::MAX_CALDAV_LEN
        && is_authority(authority)
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#'))
}

/// Whether `authority` is a host and an optional port, without a user name.
fn is_authority(authority: &str) -> bool {
    const MAX_HOST: usize = 253;
    let (good_host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let Some((address, after)) = v6.split_once(']') else { return false };
        let good = !address.is_empty() && address.chars().all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.'));
        let port = match after.strip_prefix(':') {
            Some(port) => Some(port),
            None if after.is_empty() => None,
            None => return false,
        };
        (good, port)
    } else {
        let (host, port) = authority.split_once(':').map_or((authority, None), |(host, port)| (host, Some(port)));
        (!host.is_empty() && host.len() <= MAX_HOST && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')), port)
    };
    good_host && port.is_none_or(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()) && digits.parse::<u16>().is_ok_and(|port| port != 0))
}

/// A text the owner keeps to start a message with: a name to pick it by, a subject (may be empty) and the text. The
/// subject and text may hold placeholders and blanks, which the engine fills in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Template {
    name: String,
    subject: String,
    text: String,
}

/// A [`Template`] as stored in an account record, before it is checked.
#[derive(Deserialize)]
struct RawTemplate {
    #[serde(default)]
    name: String,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    text: String,
}

impl Template {
    /// The longest name, in characters.
    pub const MAX_NAME_CHARS: usize = 100;

    /// The longest subject, in characters.
    pub const MAX_SUBJECT_CHARS: usize = 500;

    /// The longest text, in characters.
    pub const MAX_TEXT_CHARS: usize = 16_000;

    /// A template, or `None` when the name is blank. The name and the subject are one line each: control characters
    /// go and white space runs become one space. The text keeps its lines (`\n`) and tabs and loses other control
    /// characters. Each is cut at its longest.
    #[must_use]
    pub fn new(name: &str, subject: &str, text: &str) -> Option<Self> {
        let name = clean_line(name, Self::MAX_NAME_CHARS)?;
        let subject = clean_line(subject, Self::MAX_SUBJECT_CHARS).unwrap_or_default();
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let text = text.chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).take(Self::MAX_TEXT_CHARS).collect();
        Some(Self { name, subject, text })
    }

    /// The name the owner picks it by.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The subject; empty when the template has none.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
