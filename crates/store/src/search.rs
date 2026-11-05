// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The simple offline search: from, to, subject, date and body words.

use std::cmp::Reverse;

use inboxcraft_model::{AccountId, Address, FolderId, Message, MessageId};
use inboxcraft_time::Timestamp;

/// The longest text a search criterion keeps, in characters.
pub const MAX_QUERY_CHARS: usize = 256;

/// The most words a body text criterion keeps.
pub const MAX_QUERY_WORDS: usize = 32;

/// The most results one search gives.
pub const MAX_RESULTS: usize = 1000;

/// A search over the held messages: every criterion given must match.
///
/// From, to and subject match a piece of the text, without case: `from` looks at the sender names and addresses,
/// `to` at the To and Cc names and addresses. The date is the server's arrival date, or the `Date:` header when the
/// server gave none; `since` is inclusive, `before` exclusive, both to the second, and a message without a date
/// matches neither. Body text matches the words in this order, without case (a phrase); a word is a run of letters
/// and digits. Results come newest first, messages without a date last.
///
/// ```
/// use inboxcraft_store::Search;
///
/// let search = Search::new().with_from("anna").with_text("budget, 2026");
/// assert_eq!(search, Search::new().with_from("ANNA").with_text("Budget 2026"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    account: Option<AccountId>,
    folder: Option<FolderId>,
    from: Option<String>,
    to: Option<String>,
    subject: Option<String>,
    text: Vec<String>,
    since: Option<Timestamp>,
    before: Option<Timestamp>,
    limit: usize,
}

impl Default for Search {
    fn default() -> Self {
        Self::new()
    }
}

impl Search {
    /// A search that matches every message, up to [`MAX_RESULTS`] of them.
    #[must_use]
    pub fn new() -> Self {
        Self { account: None, folder: None, from: None, to: None, subject: None, text: Vec::new(), since: None, before: None, limit: MAX_RESULTS }
    }

    /// Only messages of `account`.
    #[must_use]
    pub fn with_account(mut self, account: AccountId) -> Self {
        self.account = Some(account);
        self
    }

    /// Only messages of `folder`.
    #[must_use]
    pub fn with_folder(mut self, folder: FolderId) -> Self {
        self.folder = Some(folder);
        self
    }

    /// Sender names or addresses that contain `text`.
    #[must_use]
    pub fn with_from(mut self, text: &str) -> Self {
        self.from = Some(needle(text));
        self
    }

    /// To or Cc names or addresses that contain `text`.
    #[must_use]
    pub fn with_to(mut self, text: &str) -> Self {
        self.to = Some(needle(text));
        self
    }

    /// Subjects that contain `text`.
    #[must_use]
    pub fn with_subject(mut self, text: &str) -> Self {
        self.subject = Some(needle(text));
        self
    }

    /// Bodies with the words of `text` in this order; a text without words leaves the criterion out.
    #[must_use]
    pub fn with_text(mut self, text: &str) -> Self {
        let cut: String = text.chars().take(MAX_QUERY_CHARS).collect();
        self.text = words(&cut).into_iter().take(MAX_QUERY_WORDS).collect();
        self
    }

    /// Messages dated at or after `since`.
    #[must_use]
    pub fn with_since(mut self, since: Timestamp) -> Self {
        self.since = Some(since);
        self
    }

    /// Messages dated before `before`.
    #[must_use]
    pub fn with_before(mut self, before: Timestamp) -> Self {
        self.before = Some(before);
        self
    }

    /// At most `limit` results, between 1 and [`MAX_RESULTS`].
    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.clamp(1, MAX_RESULTS);
        self
    }

    pub(crate) fn account(&self) -> Option<AccountId> {
        self.account
    }

    pub(crate) fn folder(&self) -> Option<FolderId> {
        self.folder
    }

    pub(crate) fn from(&self) -> Option<&str> {
        self.from.as_deref()
    }

    pub(crate) fn to(&self) -> Option<&str> {
        self.to.as_deref()
    }

    pub(crate) fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    pub(crate) fn text(&self) -> &[String] {
        &self.text
    }

    pub(crate) fn since(&self) -> Option<i64> {
        self.since.map(Timestamp::unix_seconds)
    }

    pub(crate) fn before(&self) -> Option<i64> {
        self.before.map(Timestamp::unix_seconds)
    }

    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    /// Whether the held fields of a message match; the body words are checked by the caller.
    pub(crate) fn matches_fields(&self, message: &Message, fields: &Fields) -> bool {
        let contains = |haystack: &str, needle: Option<&str>| needle.is_none_or(|needle| haystack.contains(needle));
        self.account().is_none_or(|account| account == message.account())
            && self.folder().is_none_or(|folder| folder == message.folder())
            && contains(&fields.from, self.from())
            && contains(&fields.to, self.to())
            && contains(&fields.subject, self.subject())
            && self.since().is_none_or(|since| fields.date.is_some_and(|date| date >= since))
            && self.before().is_none_or(|before| fields.date.is_some_and(|date| date < before))
    }
}

/// The searched fields of a message, folded for matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fields {
    /// The date in Unix seconds: arrival, else the `Date:` header.
    pub(crate) date: Option<i64>,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) subject: String,
}

impl Fields {
    pub(crate) fn of(message: &Message) -> Self {
        let summary = message.summary();
        let envelope = summary.envelope();
        let date = summary.received().or(envelope.date()).map(Timestamp::unix_seconds);
        let to: Vec<Address> = envelope.to().iter().chain(envelope.cc()).cloned().collect();
        Self { date, from: fold_addresses(envelope.from()), to: fold_addresses(&to), subject: fold(envelope.subject()) }
    }
}

/// The order of the results: newest first, undated last, then the higher id first.
pub(crate) fn order_key(date: Option<i64>, id: MessageId) -> (bool, Reverse<Option<i64>>, Reverse<MessageId>) {
    (date.is_none(), Reverse(date), Reverse(id))
}

/// Whether `words` holds `phrase` as a run.
pub(crate) fn has_phrase(words: &[String], phrase: &[String]) -> bool {
    phrase.is_empty() || words.windows(phrase.len()).any(|window| window == phrase)
}

/// The words of `text`: runs of letters and digits, folded.
pub(crate) fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(fold).collect()
}

fn needle(text: &str) -> String {
    fold(&text.chars().take(MAX_QUERY_CHARS).collect::<String>())
}

fn fold(text: &str) -> String {
    text.to_lowercase()
}

fn fold_addresses(addresses: &[Address]) -> String {
    let parts: Vec<String> = addresses
        .iter()
        .map(|address| match address.name() {
            Some(name) => format!("{name} <{}>", address.email().as_str()),
            None => address.email().as_str().to_owned(),
        })
        .collect();
    fold(&parts.join("\n"))
}
