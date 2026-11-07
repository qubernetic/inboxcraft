// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Messages: the header fields a list and a thread need, and what the server says about a message.

use std::fmt;

use inboxcraft_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::text::clean_line;
use crate::{AccountId, Address, Flags, FolderId, MessageId, ModSeq, ThreadId, Uid, UidValidity};

/// The longest subject kept, in characters.
pub const MAX_SUBJECT_CHARS: usize = 1000;

/// The most addresses kept per address field (`To`, `Cc`, ...).
pub const MAX_ADDRESSES: usize = 1000;

/// The most message ids kept in `In-Reply-To` and in `References`.
pub const MAX_REFERENCES: usize = 64;

/// The longest preview kept, in characters (the PREVIEW limit of RFC 8970).
pub const MAX_PREVIEW_CHARS: usize = 256;

/// A `Message-ID` (RFC 5322 section 3.6.4) without its angle brackets, as used to link replies.
///
/// Compared exactly, as written.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MessageRef(String);

impl MessageRef {
    /// The longest id kept, in bytes (one header line).
    pub const MAX_LEN: usize = 998;

    /// The id in `text` (`<id>` or `id`, spaces around it ignored), or `None` when it is empty, too long, or holds
    /// white space, a control character or another angle bracket.
    #[must_use]
    pub fn new(text: &str) -> Option<Self> {
        let text = text.trim_matches(' ');
        let text = text.strip_prefix('<').and_then(|inner| inner.strip_suffix('>')).unwrap_or(text);
        let bad = |c: char| c.is_whitespace() || c.is_control() || c == '<' || c == '>';
        if text.is_empty() || text.len() > Self::MAX_LEN || text.chars().any(bad) {
            return None;
        }
        Some(Self(text.to_owned()))
    }

    /// The id without angle brackets.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for MessageRef {
    type Error = &'static str;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(&text).ok_or("not a message id")
    }
}

impl From<MessageRef> for String {
    fn from(reference: MessageRef) -> Self {
        reference.0
    }
}

impl fmt::Display for MessageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.0)
    }
}

/// [`Timestamp`]s as `[seconds, nanoseconds]`, checked on the way in.
pub(crate) mod timestamp {
    use inboxcraft_time::Timestamp;
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &Option<Timestamp>, serializer: S) -> Result<S::Ok, S::Error> {
        value.map(|time| (time.unix_seconds(), time.subsec_nanos())).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Timestamp>, D::Error> {
        let raw: Option<(i64, u32)> = Option::deserialize(deserializer)?;
        raw.map(|(seconds, nanos)| Timestamp::from_unix(seconds, nanos).ok_or_else(|| D::Error::custom("timestamp out of range"))).transpose()
    }
}

/// The header fields of a message that lists, threads and the reader's short header use.
///
/// Every text is cleaned on the way in: the subject is one line of at most [`MAX_SUBJECT_CHARS`] characters, each
/// address field keeps its first [`MAX_ADDRESSES`], `In-Reply-To` its first and `References` its last
/// [`MAX_REFERENCES`] ids.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawEnvelope")]
pub struct Envelope {
    #[serde(with = "timestamp")]
    date: Option<Timestamp>,
    subject: String,
    from: Vec<Address>,
    reply_to: Vec<Address>,
    to: Vec<Address>,
    cc: Vec<Address>,
    bcc: Vec<Address>,
    message_id: Option<MessageRef>,
    in_reply_to: Vec<MessageRef>,
    references: Vec<MessageRef>,
}

/// An [`Envelope`] as stored, before it is cleaned.
#[derive(Deserialize)]
struct RawEnvelope {
    #[serde(with = "timestamp")]
    date: Option<Timestamp>,
    subject: String,
    from: Vec<Address>,
    reply_to: Vec<Address>,
    to: Vec<Address>,
    cc: Vec<Address>,
    bcc: Vec<Address>,
    message_id: Option<MessageRef>,
    in_reply_to: Vec<MessageRef>,
    references: Vec<MessageRef>,
}

impl From<RawEnvelope> for Envelope {
    fn from(raw: RawEnvelope) -> Self {
        Self::new(&raw.subject)
            .with_date(raw.date)
            .with_from(raw.from)
            .with_reply_to(raw.reply_to)
            .with_to(raw.to)
            .with_cc(raw.cc)
            .with_bcc(raw.bcc)
            .with_message_id(raw.message_id)
            .with_in_reply_to(raw.in_reply_to)
            .with_references(raw.references)
    }
}

fn first_addresses(mut addresses: Vec<Address>) -> Vec<Address> {
    addresses.truncate(MAX_ADDRESSES);
    addresses
}

impl Envelope {
    /// An envelope with `subject` (cleaned) and nothing else.
    #[must_use]
    pub fn new(subject: &str) -> Self {
        Self { subject: clean_line(subject, MAX_SUBJECT_CHARS).unwrap_or_default(), ..Self::default() }
    }

    /// With the `Date` field.
    #[must_use]
    pub fn with_date(mut self, date: Option<Timestamp>) -> Self {
        self.date = date;
        self
    }

    /// With `From`.
    #[must_use]
    pub fn with_from(mut self, from: Vec<Address>) -> Self {
        self.from = first_addresses(from);
        self
    }

    /// With `Reply-To`.
    #[must_use]
    pub fn with_reply_to(mut self, reply_to: Vec<Address>) -> Self {
        self.reply_to = first_addresses(reply_to);
        self
    }

    /// With `To`.
    #[must_use]
    pub fn with_to(mut self, to: Vec<Address>) -> Self {
        self.to = first_addresses(to);
        self
    }

    /// With `Cc`.
    #[must_use]
    pub fn with_cc(mut self, cc: Vec<Address>) -> Self {
        self.cc = first_addresses(cc);
        self
    }

    /// With `Bcc` (known for the owner's own sent mail and drafts).
    #[must_use]
    pub fn with_bcc(mut self, bcc: Vec<Address>) -> Self {
        self.bcc = first_addresses(bcc);
        self
    }

    /// With `Message-ID`.
    #[must_use]
    pub fn with_message_id(mut self, message_id: Option<MessageRef>) -> Self {
        self.message_id = message_id;
        self
    }

    /// With `In-Reply-To`.
    #[must_use]
    pub fn with_in_reply_to(mut self, mut in_reply_to: Vec<MessageRef>) -> Self {
        in_reply_to.truncate(MAX_REFERENCES);
        self.in_reply_to = in_reply_to;
        self
    }

    /// With `References`; the last ids, the closest ancestors, are the ones kept.
    #[must_use]
    pub fn with_references(mut self, mut references: Vec<MessageRef>) -> Self {
        let excess = references.len().saturating_sub(MAX_REFERENCES);
        references.drain(..excess);
        self.references = references;
        self
    }

    /// `Date`.
    #[must_use]
    pub fn date(&self) -> Option<Timestamp> {
        self.date
    }

    /// The subject, one clean line (empty when there is none).
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// `From`.
    #[must_use]
    pub fn from(&self) -> &[Address] {
        &self.from
    }

    /// `Reply-To`.
    #[must_use]
    pub fn reply_to(&self) -> &[Address] {
        &self.reply_to
    }

    /// `To`.
    #[must_use]
    pub fn to(&self) -> &[Address] {
        &self.to
    }

    /// `Cc`.
    #[must_use]
    pub fn cc(&self) -> &[Address] {
        &self.cc
    }

    /// `Bcc`.
    #[must_use]
    pub fn bcc(&self) -> &[Address] {
        &self.bcc
    }

    /// `Message-ID`.
    #[must_use]
    pub fn message_id(&self) -> Option<&MessageRef> {
        self.message_id.as_ref()
    }

    /// `In-Reply-To`.
    #[must_use]
    pub fn in_reply_to(&self) -> &[MessageRef] {
        &self.in_reply_to
    }

    /// `References`.
    #[must_use]
    pub fn references(&self) -> &[MessageRef] {
        &self.references
    }
}

/// What the server lists for one message of a folder: UID, envelope, flags, size, arrival time, PREVIEW text (RFC
/// 8970, cleaned to one line of at most [`MAX_PREVIEW_CHARS`] characters) and modification sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawSummary")]
pub struct Summary {
    uid: Uid,
    envelope: Envelope,
    flags: Flags,
    size: u32,
    #[serde(with = "timestamp")]
    received: Option<Timestamp>,
    preview: Option<String>,
    modseq: Option<ModSeq>,
}

/// A [`Summary`] as stored, before its preview is cleaned.
#[derive(Deserialize)]
struct RawSummary {
    uid: Uid,
    envelope: Envelope,
    flags: Flags,
    size: u32,
    #[serde(with = "timestamp")]
    received: Option<Timestamp>,
    preview: Option<String>,
    modseq: Option<ModSeq>,
}

impl From<RawSummary> for Summary {
    fn from(raw: RawSummary) -> Self {
        Self::new(raw.uid, raw.envelope)
            .with_flags(raw.flags)
            .with_size(raw.size)
            .with_received(raw.received)
            .with_preview(raw.preview.as_deref())
            .with_modseq(raw.modseq)
    }
}

impl Summary {
    /// The message `uid` with `envelope`, no flags, size 0 and nothing else known.
    #[must_use]
    pub fn new(uid: Uid, envelope: Envelope) -> Self {
        Self { uid, envelope, flags: Flags::default(), size: 0, received: None, preview: None, modseq: None }
    }

    /// With another UID: the same message in another folder, or before the server named its UID there.
    #[must_use]
    pub fn with_uid(mut self, uid: Uid) -> Self {
        self.uid = uid;
        self
    }

    /// With the flags.
    #[must_use]
    pub fn with_flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
        self
    }

    /// With the size in bytes (`RFC822.SIZE`).
    #[must_use]
    pub fn with_size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    /// With the time the server received it (`INTERNALDATE`).
    #[must_use]
    pub fn with_received(mut self, received: Option<Timestamp>) -> Self {
        self.received = received;
        self
    }

    /// With the preview text, cleaned.
    #[must_use]
    pub fn with_preview(mut self, preview: Option<&str>) -> Self {
        self.preview = preview.and_then(|text| clean_line(text, MAX_PREVIEW_CHARS));
        self
    }

    /// With the modification sequence.
    #[must_use]
    pub fn with_modseq(mut self, modseq: Option<ModSeq>) -> Self {
        self.modseq = modseq;
        self
    }

    /// The UID.
    #[must_use]
    pub fn uid(&self) -> Uid {
        self.uid
    }

    /// The envelope.
    #[must_use]
    pub fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// The flags.
    #[must_use]
    pub fn flags(&self) -> &Flags {
        &self.flags
    }

    /// The size in bytes.
    #[must_use]
    pub fn size(&self) -> u32 {
        self.size
    }

    /// When the server received it.
    #[must_use]
    pub fn received(&self) -> Option<Timestamp> {
        self.received
    }

    /// The preview text.
    #[must_use]
    pub fn preview(&self) -> Option<&str> {
        self.preview.as_deref()
    }

    /// The modification sequence.
    #[must_use]
    pub fn modseq(&self) -> Option<ModSeq> {
        self.modseq
    }
}

/// Where the server keeps a message the owner moved, until the server's answer names its UID in the folder it went to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moving {
    /// The folder the server keeps it in.
    pub folder: FolderId,
    /// That folder's UIDVALIDITY.
    pub uid_validity: UidValidity,
    /// Its UID there.
    pub uid: Uid,
    /// Its flags there.
    pub flags: Flags,
}

/// One copy of a message in one folder: our ids, its thread and the server's summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    id: MessageId,
    account: AccountId,
    folder: FolderId,
    thread: Option<ThreadId>,
    summary: Summary,
    /// Set while a move the owner made is on its way: the folder is where the owner put it, the UID only ours. Boxed,
    /// as most messages have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    moving: Option<Box<Moving>>,
}

impl Message {
    /// The message `id` in `folder` of `account`, in no thread yet.
    #[must_use]
    pub fn new(id: MessageId, account: AccountId, folder: FolderId, summary: Summary) -> Self {
        Self { id, account, folder, thread: None, summary, moving: None }
    }

    /// With its thread.
    #[must_use]
    pub fn with_thread(mut self, thread: Option<ThreadId>) -> Self {
        self.thread = thread;
        self
    }

    /// With where the server keeps it while a move is on its way, or `None` once the folder and UID are the server's.
    #[must_use]
    pub fn with_moving(mut self, moving: Option<Moving>) -> Self {
        self.moving = moving.map(Box::new);
        self
    }

    /// Where the server keeps it while a move the owner made is on its way; its UID here is then only ours.
    #[must_use]
    pub fn moving(&self) -> Option<&Moving> {
        self.moving.as_deref()
    }

    /// Our id.
    #[must_use]
    pub fn id(&self) -> MessageId {
        self.id
    }

    /// The account.
    #[must_use]
    pub fn account(&self) -> AccountId {
        self.account
    }

    /// The folder.
    #[must_use]
    pub fn folder(&self) -> FolderId {
        self.folder
    }

    /// The thread.
    #[must_use]
    pub fn thread(&self) -> Option<ThreadId> {
        self.thread
    }

    /// What the server said about it.
    #[must_use]
    pub fn summary(&self) -> &Summary {
        &self.summary
    }
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
