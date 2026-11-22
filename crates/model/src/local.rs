// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The local records: what InboxCraft keeps beside the mail it mirrors from the servers.
//!
//! Drafts, the outbox, the schedule, what we know of each sender, each message's own state and the calendars. The
//! engine makes them and checks what goes in; the store keeps them as given. Every field after the ids has a default,
//! so a record written by an older or newer build reads back and a new field needs no store migration.

use std::fmt;

use inboxcraft_time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::message::timestamp as optional;
use crate::{AccountId, AlertId, CalendarEventId, CalendarId, DraftId, EmailAddress, FolderId, MessageId, MessageRef, ScheduleId, Uid, UidValidity};

/// A message the owner is writing, kept on every change so a kill loses none of it.
///
/// The recipients are kept as typed, an impossible address too, so the owner sees what they wrote. `Debug` shows the
/// counts and lengths, never the words.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    /// Our id.
    pub id: DraftId,
    /// The account it is sent from.
    pub account: AccountId,
    /// When it last changed.
    #[serde(with = "required")]
    pub saved: Timestamp,
    /// `To`, as typed.
    #[serde(default)]
    pub to: Vec<String>,
    /// `Cc`, as typed.
    #[serde(default)]
    pub cc: Vec<String>,
    /// `Bcc`, as typed; only ever in the envelope.
    #[serde(default)]
    pub bcc: Vec<String>,
    /// What is typed in the To, Cc and Bcc lines and not (yet) a recipient: an address being typed, or what a line
    /// could not take (past its most, or too long). Kept so that closing or a kill loses none of it; never sent. Read
    /// within [`MAX_TYPED_CHARS`] a line, whatever the record holds ([`typed_lines`]).
    #[serde(default, deserialize_with = "typed_lines::deserialize")]
    pub typed: [String; 3],
    /// The subject.
    #[serde(default)]
    pub subject: String,
    /// The text.
    #[serde(default)]
    pub text: String,
    /// The message it answers or forwards.
    #[serde(default)]
    pub answers: Option<MessageId>,
    /// Its copy in the server's Drafts folder, which the next upload replaces.
    #[serde(default)]
    pub uploaded: Option<DraftCopy>,
    /// The `saved` time of the version last handed to the server; while it differs from `saved`, the draft waits for
    /// an upload.
    #[serde(default, with = "optional")]
    pub synced: Option<Timestamp>,
    /// A copy the server did not name (no UIDPLUS) is in the account's Drafts folder. Nothing can replace or take it
    /// back, so no further copy goes up while the draft stays on this account.
    #[serde(default)]
    pub unnamed: bool,
    /// The `Message-ID` of the message it answers, for `In-Reply-To`.
    #[serde(default)]
    pub in_reply_to: Option<MessageRef>,
    /// The ids of the thread up to the message it answers, oldest first, for `References`.
    #[serde(default)]
    pub references: Vec<MessageRef>,
    /// The message it forwards, whose attachments go with it.
    #[serde(default)]
    pub forwards: Option<MessageId>,
    /// The marks of the blanks a template left in it; while one of them is still in the subject or the text, the
    /// draft cannot be sent.
    #[serde(default)]
    pub blanks: Vec<String>,
    /// An iTIP object (RFC 5546) that goes with it as its `text/calendar` part, the answer to an invitation; its
    /// `METHOD` names the part's `method`.
    #[serde(default)]
    pub calendar: Option<String>,
}

/// Where the server keeps a copy of a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftCopy {
    /// The Drafts folder it went to.
    pub folder: FolderId,
    /// The folder's UIDVALIDITY when it went there.
    pub uid_validity: UidValidity,
    /// Its UID there.
    pub uid: Uid,
}

impl Draft {
    /// An empty draft `id` of `account`, saved at `saved`.
    #[must_use]
    pub fn new(id: DraftId, account: AccountId, saved: Timestamp) -> Self {
        Self {
            id,
            account,
            saved,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            typed: [String::new(), String::new(), String::new()],
            subject: String::new(),
            text: String::new(),
            answers: None,
            uploaded: None,
            synced: None,
            unnamed: false,
            in_reply_to: None,
            references: Vec::new(),
            forwards: None,
            blanks: Vec::new(),
            calendar: None,
        }
    }
}

impl fmt::Debug for Draft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Draft")
            .field("id", &self.id)
            .field("account", &self.account)
            .field("saved", &self.saved)
            .field("recipients", &(self.to.len(), self.cc.len(), self.bcc.len()))
            .field("typed_bytes", &self.typed.each_ref().map(String::len))
            .field("subject_chars", &self.subject.chars().count())
            .field("text_bytes", &self.text.len())
            .field("answers", &self.answers)
            .field("uploaded", &self.uploaded)
            .field("synced", &self.synced)
            .field("unnamed", &self.unnamed)
            .field("threaded", &(self.in_reply_to.is_some(), self.references.len()))
            .field("forwards", &self.forwards)
            .field("blanks", &self.blanks.len())
            .field("calendar_bytes", &self.calendar.as_ref().map(String::len))
            .finish()
    }
}

/// A draft on its way out.
///
/// `Debug` shows the length of the message, never its words.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outgoing {
    /// The draft that is sent; the outbox item goes with it.
    pub draft: DraftId,
    /// Where it stands.
    pub state: OutboxState,
    /// The message as it goes, with its own `Message-ID` and `Date`, for the copy in the Sent folder.
    #[serde(default)]
    pub message: Vec<u8>,
    /// The draft as the owner sent it, so a refused send that changed since is not sent again unseen; `None` in a
    /// record from a build that did not keep it, which counts as changed.
    #[serde(default)]
    pub sent_as: Option<SentAs>,
}

impl Outgoing {
    /// `draft` in the outbox at `state`, its message not made yet, nothing kept of how it was sent.
    #[must_use]
    pub fn new(draft: DraftId, state: OutboxState) -> Self {
        Self { draft, state, message: Vec::new(), sent_as: None }
    }

    /// `draft` in the outbox at `state`, kept as it is now.
    #[must_use]
    pub fn of(draft: &Draft, state: OutboxState) -> Self {
        Self { sent_as: Some(SentAs::of(draft)), ..Self::new(draft.id, state) }
    }

    /// What of `draft` differs from the draft as it was sent, in [`SentAs::changes`]' words; `["unknown"]` when the
    /// record kept nothing of it.
    #[must_use]
    pub fn changes(&self, draft: &Draft) -> Vec<&'static str> {
        self.sent_as.as_ref().map_or_else(|| vec!["unknown"], |sent| sent.changes(draft))
    }
}

impl fmt::Debug for Outgoing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Outgoing")
            .field("draft", &self.draft)
            .field("state", &self.state)
            .field("message_bytes", &self.message.len())
            .field("sent_as", &self.sent_as)
            .finish()
    }
}

/// The parts of a draft that make what is sent and to whom, as they were when the owner sent it: the account it goes
/// from, To, Cc, Bcc, the subject, the text, the message whose attachments go with it and an invitation's answer.
///
/// `Debug` shows the counts and lengths, never the words.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SentAs {
    /// The account it goes from.
    pub account: AccountId,
    /// `To`.
    #[serde(default)]
    pub to: Vec<String>,
    /// `Cc`.
    #[serde(default)]
    pub cc: Vec<String>,
    /// `Bcc`.
    #[serde(default)]
    pub bcc: Vec<String>,
    /// The subject.
    #[serde(default)]
    pub subject: String,
    /// The text.
    #[serde(default)]
    pub text: String,
    /// The message it forwards, whose attachments go with it.
    #[serde(default)]
    pub forwards: Option<MessageId>,
    /// The answer to an invitation that goes with it.
    #[serde(default)]
    pub calendar: Option<String>,
}

impl SentAs {
    /// The parts of `draft` that are sent.
    #[must_use]
    pub fn of(draft: &Draft) -> Self {
        Self {
            account: draft.account,
            to: draft.to.clone(),
            cc: draft.cc.clone(),
            bcc: draft.bcc.clone(),
            subject: draft.subject.clone(),
            text: draft.text.clone(),
            forwards: draft.forwards,
            calendar: draft.calendar.clone(),
        }
    }

    /// What of `draft` differs from these parts, in order: `from`, `to`, `cc`, `bcc`, `subject`, `text`,
    /// `attachments`, `invitation`; empty when it would be sent as it was.
    #[must_use]
    pub fn changes(&self, draft: &Draft) -> Vec<&'static str> {
        [
            ("from", self.account != draft.account),
            ("to", self.to != draft.to),
            ("cc", self.cc != draft.cc),
            ("bcc", self.bcc != draft.bcc),
            ("subject", self.subject != draft.subject),
            ("text", self.text != draft.text),
            ("attachments", self.forwards != draft.forwards),
            ("invitation", self.calendar != draft.calendar),
        ]
        .into_iter()
        .filter_map(|(part, changed)| changed.then_some(part))
        .collect()
    }
}

impl fmt::Debug for SentAs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SentAs")
            .field("account", &self.account)
            .field("recipients", &(self.to.len(), self.cc.len(), self.bcc.len()))
            .field("subject_chars", &self.subject.chars().count())
            .field("text_bytes", &self.text.len())
            .field("forwards", &self.forwards)
            .field("calendar_bytes", &self.calendar.as_ref().map(String::len))
            .finish()
    }
}

/// Where an outbox item stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboxState {
    /// Waiting to be sent: held for undo, or for a connection.
    Waiting,
    /// Handed to the submission server.
    Sending,
    /// The server refused it; its reply, with the enhanced status code where it gave one.
    Refused {
        /// The server's reply.
        reply: String,
    },
}

/// Something to do at a set time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheduled {
    /// Our id.
    pub id: ScheduleId,
    /// When.
    #[serde(with = "required")]
    pub due: Timestamp,
    /// What.
    pub what: Due,
}

/// What a schedule entry does when it is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Due {
    /// Send the draft (send later).
    Send(DraftId),
    /// Bring the snoozed message back.
    Unsnooze(MessageId),
    /// Remind the owner that the sent message got no reply.
    FollowUp(MessageId),
    /// Show the calendar alert.
    Alert(AlertId),
}

/// What we know of one sender address, across accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sender {
    /// The address; one record per address, told apart without case.
    pub address: EmailAddress,
    /// The owner received mail from it, wrote to it or accepted it.
    #[serde(default)]
    pub known: bool,
    /// The screener's decision, once the owner made one.
    #[serde(default)]
    pub screening: Option<Screening>,
    /// The group its mail goes to, when the owner chose one.
    #[serde(default)]
    pub group: Option<Group>,
}

impl Sender {
    /// `address`, nothing known of it.
    #[must_use]
    pub fn new(address: EmailAddress) -> Self {
        Self { address, known: false, screening: None, group: None }
    }
}

/// The screener's decision on a sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screening {
    /// Its mail comes in.
    Accepted,
    /// Its mail goes to the blocked folder, its remote content is never fetched.
    Blocked,
}

/// A group of the grouped inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Group {
    /// Mail from people.
    People,
    /// Notifications, receipts and other automatic mail.
    Updates,
    /// Mailing lists and newsletters.
    Newsletters,
    /// Calendar invitations.
    Invitations,
}

/// The owner's own state of one message, which no server keeps. The default is no state at all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageState {
    /// Done, and the folder it was in before, where "show done" lists it.
    #[serde(default)]
    pub done: Option<FolderId>,
    /// Put on the park pile.
    #[serde(default)]
    pub parked: bool,
    /// Pinned.
    #[serde(default)]
    pub pinned: bool,
    /// Snoozed until then.
    #[serde(default, with = "optional")]
    pub snoozed_until: Option<Timestamp>,
    /// Remind the owner then if no reply came.
    #[serde(default, with = "optional")]
    pub follow_up: Option<Timestamp>,
    /// Back from a snooze then: marked as returned, and listed as if it arrived then.
    #[serde(default, with = "optional")]
    pub returned: Option<Timestamp>,
    /// A message the owner sent that a delivery report says did not get through.
    #[serde(default)]
    pub bounced: Option<Bounce>,
    /// The invitation the message carries, once its body was read.
    #[serde(default)]
    pub invitation: Option<Box<Invitation>>,
}

/// What a delivery report says of a message the owner sent: it failed, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounce {
    /// The delivery report.
    pub report: MessageId,
    /// Why: each recipient it failed for, with the remote server's reply or the status code.
    pub reason: String,
}

/// What an iTIP message (RFC 5546) does, by its `METHOD`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItipMethod {
    /// `REQUEST`: an invitation, new or changed.
    Request,
    /// `CANCEL`: the event, or the owner's place in it, is off.
    Cancel,
    /// `REPLY`: a guest's answer.
    Reply,
    /// Any other method (`PUBLISH`, `COUNTER`, `REFRESH` ...), or none.
    #[default]
    Other,
}

/// The owner's answer to an invitation, a `PARTSTAT` of RFC 5545.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rsvp {
    /// Going.
    Accepted,
    /// Maybe.
    Tentative,
    /// Not going.
    Declined,
}

/// The invitation a message carries (iMIP, RFC 6047), as its body said once it was kept: the event by `UID`,
/// `RECURRENCE-ID` and `SEQUENCE`, what the owner sees of it, and the owner's answer. `Debug` shows no words of it.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invitation {
    /// What it does.
    #[serde(default)]
    pub method: ItipMethod,
    /// The event's `UID`.
    #[serde(default)]
    pub uid: String,
    /// The one occurrence it is about, as its `RECURRENCE-ID` writes it; `None` for the whole event.
    #[serde(default)]
    pub recurrence: Option<String>,
    /// The event's `SEQUENCE`: a higher one is a later version.
    #[serde(default)]
    pub sequence: u32,
    /// Its `DTSTAMP`, which orders versions of one `SEQUENCE`.
    #[serde(default, with = "optional")]
    pub stamp: Option<Timestamp>,
    /// The title.
    #[serde(default)]
    pub summary: Option<String>,
    /// When it starts.
    #[serde(default, with = "optional")]
    pub start: Option<Timestamp>,
    /// When it ends.
    #[serde(default, with = "optional")]
    pub end: Option<Timestamp>,
    /// Whole days.
    #[serde(default)]
    pub all_day: bool,
    /// Its repeat rule, as its `RRULE` writes it.
    #[serde(default)]
    pub repeat: Option<String>,
    /// Where.
    #[serde(default)]
    pub location: Option<String>,
    /// The answer the owner sent to it.
    #[serde(default)]
    pub answer: Option<Rsvp>,
}

impl Invitation {
    /// An invitation that does `method` to the event `uid`, nothing else known.
    #[must_use]
    pub fn new(method: ItipMethod, uid: &str) -> Self {
        Self { method, uid: uid.to_owned(), ..Self::default() }
    }
}

impl fmt::Debug for Invitation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Invitation")
            .field("method", &self.method)
            .field("uid_bytes", &self.uid.len())
            .field("recurrence", &self.recurrence.is_some())
            .field("sequence", &self.sequence)
            .field("stamp", &self.stamp)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("all_day", &self.all_day)
            .field("repeats", &self.repeat.is_some())
            .field("answer", &self.answer)
            .finish_non_exhaustive()
    }
}

impl MessageState {
    /// Whether nothing is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A CalDAV calendar of one account. Where its next sync starts (the sync token) is kept beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calendar {
    /// Our id.
    pub id: CalendarId,
    /// The account.
    pub account: AccountId,
    /// Its collection on the server.
    pub href: String,
    /// Its name.
    pub name: String,
    /// Its colour as the server gives it.
    #[serde(default)]
    pub colour: Option<String>,
}

/// An event of a calendar as the server keeps it: its resource, ETag and iCalendar text (RFC 5545).
///
/// `Debug` shows the length of the text, never the text.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEvent {
    /// Our id.
    pub id: CalendarEventId,
    /// The calendar.
    pub calendar: CalendarId,
    /// Its resource on the server.
    pub href: String,
    /// The ETag of the copy we hold.
    #[serde(default)]
    pub etag: Option<String>,
    /// The iCalendar text.
    pub ical: String,
}

impl fmt::Debug for CalendarEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalendarEvent")
            .field("id", &self.id)
            .field("calendar", &self.calendar)
            .field("etag", &self.etag)
            .field("ical_bytes", &self.ical.len())
            .finish_non_exhaustive()
    }
}

/// An alert of a calendar event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alert {
    /// Our id.
    pub id: AlertId,
    /// The event.
    pub event: CalendarEventId,
    /// When it goes off.
    #[serde(with = "required")]
    pub at: Timestamp,
    /// The owner dismissed it.
    #[serde(default)]
    pub dismissed: bool,
}

/// The most characters of a [`Draft`]'s typed text in one recipient line: the line's field holds no more.
pub const MAX_TYPED_CHARS: usize = 4096;

/// A [`Draft`]'s typed text as a record holds it, read without failing the draft: the first three entries are the To,
/// Cc and Bcc lines, and a line that is not text or is longer than [`MAX_TYPED_CHARS`] is read as empty, not cut (a cut
/// address could still look like one). Entries past the third, or anything but a list, are left. The composer never
/// writes such a record; a hand-edited store can, and the draft still opens with the rest of it.
mod typed_lines {
    use serde::de::IgnoredAny;
    use serde::{Deserialize, Deserializer};

    use super::MAX_TYPED_CHARS;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Line {
        Text(String),
        Other(IgnoredAny),
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Lines {
        List(Vec<Line>),
        Other(IgnoredAny),
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[String; 3], D::Error> {
        let mut typed: [String; 3] = Default::default();
        if let Lines::List(lines) = Lines::deserialize(deserializer)? {
            for (slot, line) in typed.iter_mut().zip(lines) {
                if let Line::Text(text) = line
                    && text.chars().count() <= MAX_TYPED_CHARS
                {
                    *slot = text;
                }
            }
        }
        Ok(typed)
    }
}

/// A [`Timestamp`] as `[seconds, nanoseconds]`, checked on the way in.
pub(crate) mod required {
    use inboxcraft_time::Timestamp;
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
        (value.unix_seconds(), value.subsec_nanos()).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Timestamp, D::Error> {
        let (seconds, nanos) = <(i64, u32)>::deserialize(deserializer)?;
        Timestamp::from_unix(seconds, nanos).ok_or_else(|| D::Error::custom("timestamp out of range"))
    }
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
