// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An invitation in mail (iMIP): any bytes kept as a message's body, and as the calendar part of an invitation, are
//! read into the message's invitation mark and its card, and answered and removed: none panics, so the session's
//! panic net is never used.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_engine::{EngineError, Session};
use inboxcraft_model::{
    Account, AccountId, Calendar, CalendarId, EmailAddress, Envelope, Event, EventKind, Folder, FolderId, FolderListing, FolderPath, FolderState,
    Host, Security, Server, Signs, Summary, Uid, UidValidity,
};
use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use libfuzzer_sys::fuzz_target;

/// One account that can send, with a synced inbox and a calendar.
fn mailbox() -> Option<Memory> {
    let imap = Server::new(Host::new("imap.example.org").ok()?, Security::Tls.imap_port(), Security::Tls);
    let submission = Server::new(Host::new("smtp.example.org").ok()?, Security::Tls.submission_port(), Security::Tls);
    let account = Account::new(AccountId::new(1), "Home", EmailAddress::new("anna@example.org").ok()?, "anna", imap)
        .ok()?
        .with_submission(Some(submission))
        .with_caldav(Some("https://dav.example.org/"))
        .ok()?;
    let inbox = Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", Some('/')).ok()?, vec![], true));
    let calendar = Calendar {
        id: CalendarId::new(1),
        account: AccountId::new(1),
        href: "https://dav.example.org/calendars/anna/home/".to_owned(),
        name: "Home".to_owned(),
        colour: None,
    };
    let mut store = Memory::new(StoreConfig::default());
    let batch = Batch::new()
        .with(Change::PutAccount(account))
        .with(Change::PutFolder(inbox))
        .with(Change::SetFolderState(FolderId::new(1), FolderState::new(UidValidity::new(7)?)))
        .with(Change::PutCalendar(calendar));
    store.apply(&batch).ok()?;
    Some(store)
}

/// `calendar` as the calendar part of an invitation from Odile.
fn wrapped(calendar: &[u8]) -> Vec<u8> {
    let mut raw = b"From: odile@example.org\r\nTo: anna@example.org\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=b\r\n\r\n\
                    --b\r\nContent-Type: text/plain\r\n\r\nInvitation\r\n--b\r\nContent-Type: text/calendar; method=REQUEST\r\n\r\n"
        .to_vec();
    raw.extend_from_slice(calendar);
    raw.extend_from_slice(b"\r\n--b--\r\n");
    raw
}

/// The message with UID `n` arrives, its body `raw` is kept, and its card is read, answered and removed.
fn play(session: &mut Session, n: u32, raw: Vec<u8>) {
    let Some(uid) = Uid::new(n) else { return };
    let Some(validity) = UidValidity::new(7) else { return };
    let envelope = Envelope::new("Invitation").with_signs(Signs::default().with_calendar(true));
    let summary = Summary::new(uid, envelope).with_received(Timestamp::from_unix(1_000_000, 0));
    let added = EventKind::MessagesAdded { folder: FolderId::new(1), uid_validity: validity, messages: vec![summary] };
    let fetched = EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity, uid, raw };
    for kind in [added, fetched] {
        let applied = session.apply(&Event::new(AccountId::new(1), None, kind));
        assert!(!matches!(applied, Err(EngineError::Panicked(_))), "an event panicked");
    }
    let Some(id) = session
        .store()
        .messages(FolderId::new(1))
        .ok()
        .and_then(|entries| entries.into_iter().find(|entry| entry.message().summary().uid() == uid))
        .map(|entry| entry.message().id().get())
    else {
        return;
    };
    for (command, params) in [
        ("invite.show", serde_json::json!({ "id": id })),
        ("mail.read", serde_json::json!({ "id": id })),
        ("invite.answer", serde_json::json!({ "id": id, "answer": "accepted" })),
        ("invite.answer", serde_json::json!({ "id": id, "answer": "declined" })),
        ("invite.remove", serde_json::json!({ "id": id })),
        ("invite.show", serde_json::json!({ "id": id })),
    ] {
        let result = session.run(command, &params);
        assert!(!matches!(result, Err(EngineError::Panicked(_))), "{command} panicked");
    }
}

fuzz_target!(|data: &[u8]| {
    let Some(store) = mailbox() else { return };
    let Ok(mut session) = Session::new(Box::new(store), Box::new(FakeClock::new(Timestamp::UNIX_EPOCH))) else { return };
    play(&mut session, 1, data.to_vec());
    play(&mut session, 2, wrapped(data));
});
