// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The resync of a known folder: changed flags, removed and new messages since the last sync, with QRESYNC, with
//! CONDSTORE only, and with neither; and a folder whose UIDVALIDITY changed is reloaded.

mod support;

use inboxcraft_model::{EventKind, Flag, JobKind};
use inboxcraft_testkit::imap::{Action, Capability, FakeImap, Message, When};
use support::Result;
use support::run::{INBOX, Run, added, changed, expunged, state, sync_inbox, uid, validity};

/// The sample INBOX (UIDs 1 to 3), changed once the folder list is asked for: UID 1 flagged, UID 2 removed, a new
/// message delivered as UID 4.
fn changing(fake: FakeImap) -> FakeImap {
    let inbox = || "INBOX".to_owned();
    fake.on(
        When::answered("LIST"),
        [
            Action::Flags { mailbox: inbox(), uid: 1, flags: vec!["\\Seen".to_owned(), "\\Flagged".to_owned()] },
            Action::Expunge { mailbox: inbox(), uid: 2 },
            Action::Deliver { mailbox: inbox(), message: Message::new("From: dora@example.org\nSubject: New one\n\nHello.\n") },
        ],
    )
}

/// Syncs INBOX, lets the fake change it, and resyncs it from the first sync's state; the resync's events and the
/// lines the server got.
fn resync(fake: FakeImap) -> Result<(Vec<EventKind>, Vec<String>)> {
    let mut run = Run::start(changing(fake), Some(inboxcraft_testkit::imap::PASSWORD), inboxcraft_net::SyncConfig::default())?;
    run.job(sync_inbox(None)?)?;
    let first = state(&run.synced()?).ok_or("no first sync")?;
    assert_eq!(first.uid_next(), Some(uid(4)?));
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    run.job(sync_inbox(Some(first))?)?;
    let events = run.synced()?;
    Ok((events, run.finish()?))
}

#[test]
fn qresync_brings_the_changes_and_the_removed_with_the_examine() -> Result {
    let (events, lines) = resync(FakeImap::sample())?;
    assert_eq!(expunged(&events), [2]);
    let changes = changed(&events);
    assert_eq!(changes.iter().map(|change| change.uid.get()).collect::<Vec<_>>(), [1]);
    assert!(changes[0].flags.contains(&Flag::Flagged) && changes[0].modseq.is_some());
    assert_eq!(added(&events).iter().map(|summary| summary.uid().get()).collect::<Vec<_>>(), [4]);
    let next = state(&events).ok_or("not synced")?;
    assert_eq!(next.uid_next(), Some(uid(5)?));
    assert!(lines.iter().any(|line| line.contains("EXAMINE \"INBOX\" (QRESYNC (1 4))")), "{lines:?}");
    // The resync needs no flags fetch of the known messages.
    assert!(!lines.iter().any(|line| line.contains("UID FETCH 1:3 (UID FLAGS")), "{lines:?}");
    Ok(())
}

#[test]
fn condstore_alone_fetches_the_flags_and_reports_only_what_changed() -> Result {
    let (events, lines) = resync(FakeImap::sample().without(Capability::QResync))?;
    assert_eq!(expunged(&events), [2]);
    assert_eq!(changed(&events).iter().map(|change| change.uid.get()).collect::<Vec<_>>(), [1]);
    assert_eq!(added(&events).iter().map(|summary| summary.uid().get()).collect::<Vec<_>>(), [4]);
    assert!(lines.iter().any(|line| line.contains("UID FETCH 1:3 (UID FLAGS MODSEQ)")), "{lines:?}");
    Ok(())
}

#[test]
fn without_modification_sequences_every_known_message_is_reported() -> Result {
    let fake = FakeImap::sample().without(Capability::QResync).without(Capability::CondStore);
    let (events, lines) = resync(fake)?;
    assert_eq!(expunged(&events), [2]);
    assert_eq!(changed(&events).iter().map(|change| change.uid.get()).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(added(&events).iter().map(|summary| summary.uid().get()).collect::<Vec<_>>(), [4]);
    assert!(lines.iter().any(|line| line.contains("UID FETCH 1:3 (UID FLAGS)")), "{lines:?}");
    Ok(())
}

#[test]
fn a_changed_uidvalidity_reloads_the_folder() -> Result {
    let fake = FakeImap::sample().on(When::answered("LIST"), [Action::UidValidity { mailbox: "INBOX".to_owned(), value: 42 }]);
    let mut run = Run::start(fake, Some(inboxcraft_testkit::imap::PASSWORD), inboxcraft_net::SyncConfig::default())?;
    run.job(sync_inbox(None)?)?;
    let first = state(&run.synced()?).ok_or("no first sync")?;
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    run.job(sync_inbox(Some(first))?)?;
    let events = run.synced()?;
    assert!(events.contains(&EventKind::UidValidityReset { folder: INBOX, old: Some(validity(1)?), new: validity(42)? }), "{events:?}");
    assert!(expunged(&events).is_empty() && changed(&events).is_empty(), "{events:?}");
    assert_eq!(added(&events).len(), 3);
    assert_eq!(state(&events).map(|state| state.uid_validity()), Some(validity(42)?));
    run.finish()?;
    Ok(())
}

#[test]
fn an_emptied_folder_reports_every_known_message_removed() -> Result {
    let inbox = || "INBOX".to_owned();
    let fake = FakeImap::sample().without(Capability::QResync).on(
        When::answered("LIST"),
        [Action::Expunge { mailbox: inbox(), uid: 1 }, Action::Expunge { mailbox: inbox(), uid: 2 }, Action::Expunge { mailbox: inbox(), uid: 3 }],
    );
    let mut run = Run::start(fake, Some(inboxcraft_testkit::imap::PASSWORD), inboxcraft_net::SyncConfig::default())?;
    run.job(sync_inbox(None)?)?;
    let first = state(&run.synced()?).ok_or("no first sync")?;
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    run.job(sync_inbox(Some(first))?)?;
    let events = run.synced()?;
    assert_eq!(expunged(&events), [1, 2, 3]);
    assert!(added(&events).is_empty());
    let lines = run.finish()?;
    assert_eq!(lines.iter().filter(|line| line.contains("UID FETCH")).count(), 2, "{lines:?}");
    Ok(())
}
