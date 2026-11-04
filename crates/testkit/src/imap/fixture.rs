// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake's mailboxes and messages, and the invented sample mailboxes of [`crate::imap::FakeImap::sample`]. Every
//! person, address and message in them is made up, on `example.org`.

/// The arrival time a [`Message`] has unless [`Message::date`] sets one.
const DEFAULT_DATE: &str = "01-Jan-2026 00:00:00 +0000";

/// A message in a fake mailbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub(super) uid: u32,
    pub(super) flags: Vec<String>,
    pub(super) date: String,
    pub(super) raw: Vec<u8>,
    pub(super) modseq: u64,
}

impl Message {
    /// A message from its RFC 5322 text; bare LF line ends become CRLF. It has no flags, arrived at
    /// `01-Jan-2026 00:00:00 +0000` and takes the mailbox's next UID.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let raw = text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect::<Vec<_>>().join("\r\n").into_bytes();
        Self { uid: 0, flags: Vec::new(), date: DEFAULT_DATE.to_owned(), raw, modseq: 0 }
    }

    /// Takes this UID when the mailbox adds the message, unless that UID is below the mailbox's next one.
    #[must_use]
    pub fn uid(self, uid: u32) -> Self {
        Self { uid, ..self }
    }

    /// The message's flags (`\Seen`, `\Flagged`, `$Label1`).
    #[must_use]
    pub fn flags(self, flags: &[&str]) -> Self {
        Self { flags: flags.iter().map(|flag| clean(flag)).collect(), ..self }
    }

    /// The arrival time as IMAP writes it (`05-Nov-2026 10:00:00 +0100`).
    #[must_use]
    pub fn date(self, date: &str) -> Self {
        Self { date: clean(date), ..self }
    }

    pub(super) fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|own| own.eq_ignore_ascii_case(flag))
    }

    /// The header block, blank line included, and the body.
    pub(super) fn parts(&self) -> (&[u8], &[u8]) {
        let split = self.raw.windows(4).position(|window| window == b"\r\n\r\n").map_or(self.raw.len(), |at| at + 4);
        self.raw.split_at(split)
    }

    /// The unfolded values of the header fields called `name`.
    pub(super) fn header(&self, name: &str) -> Vec<String> {
        let text = String::from_utf8_lossy(self.parts().0).into_owned();
        let mut fields: Vec<String> = Vec::new();
        for line in text.split("\r\n") {
            match fields.last_mut() {
                Some(last) if line.starts_with([' ', '\t']) => last.push_str(line),
                _ => fields.push(line.to_owned()),
            }
        }
        fields
            .iter()
            .filter_map(|field| field.split_once(':'))
            .filter(|(field, _)| field.trim().eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().to_owned())
            .collect()
    }
}

/// A fake mailbox: its name, its SPECIAL-USE attribute, its UIDVALIDITY and its messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mailbox {
    pub(super) name: String,
    pub(super) special_use: Option<String>,
    pub(super) uid_validity: u32,
    pub(super) uid_next: u32,
    pub(super) highest_modseq: u64,
    pub(super) messages: Vec<Message>,
    /// The UIDs removed so far, with the modification sequence of their removal, for QRESYNC.
    pub(super) vanished: Vec<(u32, u64)>,
}

impl Mailbox {
    /// An empty mailbox with UIDVALIDITY 1. A name that is INBOX in any case is `INBOX`; `/` separates levels.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let name = if name.eq_ignore_ascii_case("INBOX") { "INBOX".to_owned() } else { clean(name) };
        Self { name, special_use: None, uid_validity: 1, uid_next: 1, highest_modseq: 1, messages: Vec::new(), vanished: Vec::new() }
    }

    /// The SPECIAL-USE attribute (`\Sent`, `\Drafts`, `\Archive`, `\Junk`, `\Trash`, `\All`, `\Flagged`).
    #[must_use]
    pub fn special_use(self, attribute: &str) -> Self {
        Self { special_use: Some(clean(attribute)), ..self }
    }

    /// The UIDVALIDITY.
    #[must_use]
    pub fn uid_validity(self, value: u32) -> Self {
        Self { uid_validity: value, ..self }
    }

    /// Adds a message under its own UID or the next one.
    #[must_use]
    pub fn message(mut self, message: Message) -> Self {
        self.add(message);
        self
    }

    /// Records `uids` as removed before the test, for QRESYNC to report as vanished; the next UID is above them.
    #[must_use]
    pub fn removed(mut self, uids: &[u32]) -> Self {
        for &uid in uids {
            let modseq = self.bump();
            self.vanished.push((uid, modseq));
            self.uid_next = self.uid_next.max(uid.saturating_add(1));
        }
        self
    }

    /// The name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Adds a message under its own UID or the next one and returns the UID it got.
    pub(super) fn add(&mut self, mut message: Message) -> u32 {
        message.uid = message.uid.max(self.uid_next);
        message.modseq = self.bump();
        self.uid_next = message.uid.saturating_add(1);
        let uid = message.uid;
        self.messages.push(message);
        uid
    }

    /// Removes the messages `remove` picks and returns their UIDs, in UID order.
    pub(super) fn expunge(&mut self, remove: impl Fn(&Message) -> bool) -> Vec<u32> {
        let uids: Vec<u32> = self.messages.iter().filter(|message| remove(message)).map(|message| message.uid).collect();
        self.messages.retain(|message| !uids.contains(&message.uid));
        for &uid in &uids {
            let modseq = self.bump();
            self.vanished.push((uid, modseq));
        }
        uids
    }

    /// The UIDs removed after `modseq`, in UID order.
    pub(super) fn vanished_since(&self, modseq: u64) -> Vec<u32> {
        let mut uids: Vec<u32> = self.vanished.iter().filter(|(_, at)| *at > modseq).map(|(uid, _)| *uid).collect();
        uids.sort_unstable();
        uids
    }

    /// Makes the mailbox again: a new UIDVALIDITY, UIDs from 1, nothing removed.
    pub(super) fn renumber(&mut self, uid_validity: u32) {
        self.uid_validity = uid_validity;
        let mut uid: u32 = 0;
        for message in &mut self.messages {
            uid = uid.saturating_add(1);
            message.uid = uid;
        }
        self.uid_next = uid.saturating_add(1);
        self.vanished.clear();
    }

    /// The next modification sequence, which is now the highest.
    pub(super) fn bump(&mut self) -> u64 {
        self.highest_modseq = self.highest_modseq.saturating_add(1);
        self.highest_modseq
    }

    pub(super) fn unseen(&self) -> usize {
        self.messages.iter().filter(|message| !message.has_flag("\\Seen")).count()
    }
}

/// `text` without CR, LF and NUL.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect()
}

/// The sample mailboxes.
pub(super) fn sample() -> Vec<Mailbox> {
    vec![
        Mailbox::new("INBOX")
            .message(
                Message::new(
                    "From: Anna Example <anna@example.org>\nTo: ada@example.org\nSubject: Lunch on Friday\nDate: Mon, 2 Nov 2026 09:14:00 +0000\n\nShall we try the new place?\n",
                )
                .flags(&["\\Seen"])
                .date("02-Nov-2026 09:15:00 +0000"),
            )
            .message(
                Message::new(
                    "From: builds@example.org\nTo: ada@example.org\nSubject: Nightly build passed\nList-Id: Builds <builds.example.org>\nDate: Tue, 3 Nov 2026 02:00:00 +0000\n\nAll 412 tests passed.\n",
                )
                .date("03-Nov-2026 02:00:00 +0000"),
            )
            .message(
                Message::new(
                    "From: Bob Example <bob@example.org>\nTo: ada@example.org\nSubject: Report draft\nDate: Wed, 4 Nov 2026 16:30:00 +0000\n\nHere is the draft of the report for the last quarter.\nComments welcome.\n",
                )
                .flags(&["\\Flagged"])
                .date("04-Nov-2026 16:31:00 +0000"),
            ),
        Mailbox::new("Sent").special_use("\\Sent").message(
            Message::new("From: ada@example.org\nTo: Anna Example <anna@example.org>\nSubject: Re: Lunch on Friday\n\nYes, at noon.\n")
                .flags(&["\\Seen"])
                .date("02-Nov-2026 10:02:00 +0000"),
        ),
        Mailbox::new("Drafts").special_use("\\Drafts"),
        Mailbox::new("Archive").special_use("\\Archive").uid_validity(7).message(
            Message::new("From: anna@example.org\nTo: ada@example.org\nSubject: Old news\n\nFrom last year.\n").uid(12).flags(&["\\Seen"]).date("10-Oct-2025 08:00:00 +0000"),
        ),
        Mailbox::new("Junk").special_use("\\Junk").message(Message::new("From: prize@example.org\nTo: ada@example.org\nSubject: You won\n\nClaim it now.\n")),
        Mailbox::new("Trash").special_use("\\Trash"),
        Mailbox::new("Lists"),
        Mailbox::new("Lists/Rust").message(Message::new("From: list@example.org\nTo: ada@example.org\nSubject: Weekly digest\n\nThree threads this week.\n")),
        Mailbox::new("Elküldött"),
    ]
}
