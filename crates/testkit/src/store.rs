// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fixture store: a small invented mailbox written to disk, the same every time, for tests that open a store the
//! way the apps do (the CLI's `--store <dir>`).
//!
//! One account (`fixture@example.org`) with an inbox, a sent folder and an archive, and four messages: an unread one
//! and a read, flagged one in the inbox (both with their bodies kept), an archived one whose body is not kept, and a
//! sent reply. Every name, address and text is made up; the dates count back from [`NOW`].

use std::error::Error;
use std::path::Path;
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, Address, EmailAddress, Envelope, Flag, Flags, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId,
    Security, Server, SpecialUse, Summary, Uid,
};
use inboxcraft_store::{Batch, Body, Change, DATABASE_FILE, Sqlite, Storage, StoreConfig};
use inboxcraft_time::Timestamp;

/// The fixture's account.
pub const ACCOUNT: u64 = 1;
/// The inbox.
pub const INBOX: u64 = 1;
/// The sent folder (`\Sent`).
pub const SENT: u64 = 2;
/// The archive (`\Archive`).
pub const ARCHIVE: u64 = 3;
/// In the inbox, unread, body kept: "Lunch on Thursday".
pub const LUNCH: u64 = 1;
/// In the inbox, read and flagged, body kept: "Minutes of the garden club".
pub const MINUTES: u64 = 2;
/// In the archive, read, body not kept: "Your receipt".
pub const RECEIPT: u64 = 3;
/// In the sent folder: "Re: Lunch on Thursday".
pub const REPLY: u64 = 4;
/// The time the fixture's dates count back from: 2026-11-02T09:00:00Z.
pub const NOW: Timestamp = match Timestamp::from_unix(1_793_610_000, 0) {
    Some(now) => now,
    None => Timestamp::UNIX_EPOCH,
};

/// One invented message: `(id, folder, uid, minutes before NOW, from, to, subject, seen, flagged, body text)`.
type Fixture = (u64, u64, u32, u64, (&'static str, &'static str), (&'static str, &'static str), &'static str, bool, bool, Option<&'static str>);

const OWNER: (&str, &str) = ("Fixture Owner", "fixture@example.org");

const MESSAGES: [Fixture; 4] = [
    (
        LUNCH,
        INBOX,
        1,
        30,
        ("Ada Example", "ada@example.org"),
        OWNER,
        "Lunch on Thursday",
        false,
        false,
        Some("Shall we meet at the allotment cafe at noon on Thursday?\n"),
    ),
    (
        MINUTES,
        INBOX,
        2,
        120,
        ("Rowan Example", "rowan@example.org"),
        OWNER,
        "Minutes of the garden club",
        true,
        true,
        Some("The minutes of last week's meeting: the shed gets a new roof.\n"),
    ),
    (RECEIPT, ARCHIVE, 1, 4320, ("Corner Shop", "shop@example.org"), OWNER, "Your receipt", true, false, None),
    (REPLY, SENT, 1, 10, OWNER, ("Ada Example", "ada@example.org"), "Re: Lunch on Thursday", true, false, Some("Noon suits me.\n")),
];

/// Writes the fixture store into `dir`.
///
/// # Errors
///
/// A directory that already holds a store (so a test never writes over one), or a store that cannot be written.
pub fn fixture(dir: &Path) -> Result<(), Box<dyn Error>> {
    if dir.join(DATABASE_FILE).exists() {
        return Err(format!("{} already holds a store", dir.display()).into());
    }
    let imap = Server::new(Host::new("imap.example.org")?, Security::Tls.imap_port(), Security::Tls);
    let account = Account::new(AccountId::new(ACCOUNT), "Fixture", EmailAddress::new(OWNER.1)?, OWNER.1, imap)?.with_display_name(Some(OWNER.0));
    let mut batch = Batch::new().with(Change::PutAccount(account));
    for (id, path, special_use) in [(INBOX, "INBOX", vec![]), (SENT, "Sent", vec![SpecialUse::Sent]), (ARCHIVE, "Archive", vec![SpecialUse::Archive])]
    {
        let listing = FolderListing::new(FolderPath::new(path, Some('/'))?, special_use, true);
        batch.push(Change::PutFolder(Folder::new(FolderId::new(id), AccountId::new(ACCOUNT), listing)));
    }
    for (id, folder, uid, minutes_ago, from, to, subject, seen, flagged, text) in MESSAGES {
        let date = NOW.checked_sub(Duration::from_secs(minutes_ago.saturating_mul(60)));
        let envelope = Envelope::new(subject)
            .with_date(date)
            .with_from(vec![Address::new(Some(from.0), EmailAddress::new(from.1)?)])
            .with_to(vec![Address::new(Some(to.0), EmailAddress::new(to.1)?)]);
        let mut flags = Flags::default();
        if seen {
            flags.insert(Flag::Seen);
        }
        if flagged {
            flags.insert(Flag::Flagged);
        }
        let uid = Uid::new(uid).ok_or("a UID of 0")?;
        let summary = Summary::new(uid, envelope).with_flags(flags).with_received(date).with_size(512).with_preview(text);
        batch.push(Change::PutMessage(Message::new(MessageId::new(id), AccountId::new(ACCOUNT), FolderId::new(folder), summary)));
        if let Some(text) = text {
            let source = format!(
                "From: {} <{}>\r\nTo: {} <{}>\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
                from.0,
                from.1,
                to.0,
                to.1,
                text.replace('\n', "\r\n")
            );
            batch.push(Change::PutBody(MessageId::new(id), Body::new(source.into_bytes(), text)?));
        }
    }
    Sqlite::open(dir, StoreConfig::default())?.apply(&batch)?;
    Ok(())
}
