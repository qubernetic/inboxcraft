// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{
    AccountId, ConnectionState, ErrorKind, Event, EventKind, Flag, FlagChange, Flags, FolderId, FolderListing, FolderPath, FolderState, JobId,
    JobKind, MessageId, SpecialUse, Uid, UidSet, UidValidity,
};
use inboxcraft_store::{Change, MAX_SOURCE_BYTES};
use serde_json::json;

use super::plain_text;
use crate::Applied;
use crate::test_support::{mailbox, message, session, session_with};

fn validity(n: u32) -> UidValidity {
    UidValidity::new(n).unwrap()
}

fn uid(n: u32) -> Uid {
    Uid::new(n).unwrap()
}

fn listing(name: &str, special_use: Vec<SpecialUse>, selectable: bool) -> FolderListing {
    FolderListing::new(FolderPath::new(name, Some('/')).unwrap(), special_use, selectable)
}

fn event(account: u64, kind: EventKind) -> Event {
    Event::new(AccountId::new(account), None, kind)
}

fn uids(store: &dyn inboxcraft_store::Storage, folder: u64) -> Vec<(u64, u32)> {
    store.messages(FolderId::new(folder)).unwrap().iter().map(|entry| (entry.message().id().get(), entry.message().summary().uid().get())).collect()
}

fn summary(n: u32, subject: &str) -> inboxcraft_model::Summary {
    message(0, 1, 1, n, subject, Some(crate::test_support::NOW)).summary().clone()
}

#[test]
fn start_asks_every_account_for_its_folders() {
    let mut session = session();
    session.start();
    let jobs = session.take_jobs();
    assert_eq!(
        jobs.iter().map(|job| (job.account.get(), job.kind.clone())).collect::<Vec<_>>(),
        [(1, JobKind::SyncFolderList), (2, JobKind::SyncFolderList)]
    );
}

#[test]
fn a_folder_list_keeps_ids_adds_removes_and_syncs_the_inbox_first() {
    let mut session = session();
    session.apply(&event(1, EventKind::FolderSynced { folder: FolderId::new(2), state: FolderState::new(validity(9)) })).unwrap();
    let listings = vec![
        listing("Sent", vec![SpecialUse::Sent], true),
        listing("Projects", vec![], true),
        listing("inbox", vec![], true),
        listing("Projects", vec![], true),
        listing("[Gmail]", vec![], false),
    ];
    assert_eq!(session.apply(&event(1, EventKind::Folders { listings })).unwrap(), Applied::Changed);
    let folders = session.store().folders(AccountId::new(1)).unwrap();
    let names: Vec<(u64, &str)> = folders.iter().map(|folder| (folder.id().get(), folder.listing().path().name())).collect();
    assert_eq!(names, [(1, "inbox"), (2, "Sent"), (6, "Projects"), (7, "[Gmail]")], "Archive went, the inbox kept its id");
    assert!(session.store().messages(FolderId::new(1)).unwrap().len() == 2, "the inbox kept its messages");
    let jobs: Vec<JobKind> = session.take_jobs().into_iter().map(|job| job.kind).collect();
    let synced: Vec<(u64, Option<FolderState>)> = jobs
        .iter()
        .filter_map(|kind| match kind {
            JobKind::SyncFolder { folder, known, .. } => Some((folder.get(), *known)),
            _ => None,
        })
        .collect();
    assert_eq!(synced, [(1, None), (2, Some(FolderState::new(validity(9)))), (6, None)], "inbox first, no sync of a folder without mail");
    assert!(matches!(jobs.last(), Some(JobKind::Idle { folder, .. }) if folder.get() == 1), "{jobs:?}");
}

#[test]
fn a_folder_list_without_an_inbox_watches_nothing() {
    let mut session = session();
    session.apply(&event(2, EventKind::Folders { listings: vec![listing("Notes", vec![], true)] })).unwrap();
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert!(matches!(&jobs[0].kind, JobKind::SyncFolder { folder, known: None, .. } if folder.get() == 6));
}

#[test]
fn an_event_for_something_not_held_is_dropped() {
    let mut session = session();
    assert_eq!(session.apply(&event(9, EventKind::Connection(ConnectionState::Online))).unwrap(), Applied::Dropped);
    // Folder 4 belongs to account 2.
    let added = EventKind::MessagesAdded { folder: FolderId::new(4), uid_validity: validity(1), messages: vec![summary(99, "Stray")] };
    assert_eq!(session.apply(&event(1, added)).unwrap(), Applied::Dropped);
    assert_eq!(uids(session.store(), 4), [(3, 7), (4, 8)]);
}

#[test]
fn added_messages_get_new_ids_and_known_ones_keep_theirs() {
    let mut session = session();
    let added = EventKind::MessagesAdded {
        folder: FolderId::new(1),
        uid_validity: validity(5),
        messages: vec![summary(10, "Budget, again"), summary(12, "New one"), summary(12, "New one, twice")],
    };
    session.apply(&event(1, added)).unwrap();
    assert_eq!(uids(session.store(), 1), [(1, 10), (2, 11), (6, 12)]);
    let first = session.store().message(MessageId::new(1)).unwrap().unwrap();
    assert_eq!(first.message().summary().envelope().subject(), "Budget, again");
}

#[test]
fn uids_of_an_older_uidvalidity_are_dropped_until_a_reset() {
    let mut session = session();
    session.apply(&event(1, EventKind::FolderSynced { folder: FolderId::new(1), state: FolderState::new(validity(5)) })).unwrap();
    let stale = || EventKind::MessagesAdded { folder: FolderId::new(1), uid_validity: validity(4), messages: vec![summary(20, "Old")] };
    assert_eq!(session.apply(&event(1, stale())).unwrap(), Applied::Dropped);
    let synced = EventKind::FolderSynced { folder: FolderId::new(1), state: FolderState::new(validity(4)) };
    assert_eq!(session.apply(&event(1, synced)).unwrap(), Applied::Dropped);
    let stale_flags = EventKind::MessagesChanged { folder: FolderId::new(1), uid_validity: validity(4), changes: vec![] };
    assert_eq!(session.apply(&event(1, stale_flags)).unwrap(), Applied::Dropped);
    let stale_gone = EventKind::MessagesExpunged { folder: FolderId::new(1), uid_validity: validity(4), uids: UidSet::default() };
    assert_eq!(session.apply(&event(1, stale_gone)).unwrap(), Applied::Dropped);
    let stale_body = EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity(4), uid: uid(10), raw: b"x".to_vec() };
    assert_eq!(session.apply(&event(1, stale_body)).unwrap(), Applied::Dropped);
    assert_eq!(uids(session.store(), 1), [(1, 10), (2, 11)]);

    let reset = EventKind::UidValidityReset { folder: FolderId::new(1), old: Some(validity(5)), new: validity(4) };
    session.apply(&event(1, reset)).unwrap();
    assert!(uids(session.store(), 1).is_empty(), "every UID was void");
    session.apply(&event(1, stale())).unwrap();
    assert_eq!(uids(session.store(), 1), [(6, 20)]);
}

#[test]
fn flags_and_removals_follow_the_server() {
    let mut session = session();
    let seen: Flags = [Flag::Seen].into_iter().collect();
    let changes =
        vec![FlagChange { uid: uid(10), flags: seen.clone(), modseq: None }, FlagChange { uid: uid(77), flags: seen.clone(), modseq: None }];
    session.apply(&event(1, EventKind::MessagesChanged { folder: FolderId::new(1), uid_validity: validity(1), changes })).unwrap();
    let first = session.store().message(MessageId::new(1)).unwrap().unwrap();
    assert!(first.message().summary().flags().is_seen());
    let gone: UidSet = [(uid(11), uid(11)), (uid(40), uid(50))].into_iter().collect();
    session.apply(&event(1, EventKind::MessagesExpunged { folder: FolderId::new(1), uid_validity: validity(1), uids: gone })).unwrap();
    assert_eq!(uids(session.store(), 1), [(1, 10)]);
}

#[test]
fn a_fetched_body_is_kept_with_its_text() {
    let mut session = session();
    let raw = b"Subject: Budget\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nThe marmot budget is ready.\r\n".to_vec();
    let fetched = EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity(1), uid: uid(10), raw: raw.clone() };
    assert_eq!(session.apply(&event(1, fetched)).unwrap(), Applied::Changed);
    let body = session.store().body(MessageId::new(1)).unwrap().unwrap();
    assert_eq!(body.source(), raw.as_slice());
    assert_eq!(body.text(), "The marmot budget is ready.\r\n");
    let found = session.store().search(&inboxcraft_store::Search::new().with_text("marmot budget")).unwrap();
    assert_eq!(found, [MessageId::new(1)]);
}

#[test]
fn a_body_for_no_message_or_too_large_is_dropped() {
    let mut session = session();
    let unknown = EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity(1), uid: uid(99), raw: b"x".to_vec() };
    assert_eq!(session.apply(&event(1, unknown)).unwrap(), Applied::Dropped);
    let huge = EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity(1), uid: uid(10), raw: vec![b'a'; MAX_SOURCE_BYTES + 1] };
    assert_eq!(session.apply(&event(1, huge)).unwrap(), Applied::Dropped);
    assert!(session.store().body(MessageId::new(1)).unwrap().is_none());
}

#[test]
fn plain_text_is_empty_for_unreadable_or_html_only_mail() {
    assert_eq!(plain_text(b"Content-Type: multipart/mixed\r\n\r\nx"), "");
    assert_eq!(plain_text(b"Content-Type: text/html\r\n\r\n<p>Hi</p>"), "");
    assert_eq!(plain_text(b"\r\nHello"), "Hello");
}

#[test]
fn a_synced_folder_records_where_the_next_sync_starts() {
    let mut session = session();
    let state = FolderState::new(validity(3)).with_uid_next(Uid::new(12));
    session.apply(&event(1, EventKind::FolderSynced { folder: FolderId::new(1), state })).unwrap();
    assert_eq!(session.store().folder_state(FolderId::new(1)).unwrap(), Some(state));
}

#[test]
fn connection_and_failures_show_in_account_list() {
    let mut session = session();
    let failed = Event::new(AccountId::new(1), Some(JobId::new(4)), EventKind::Failed { kind: ErrorKind::LoginRefused, folder: None });
    session.apply(&failed).unwrap();
    session.apply(&event(1, EventKind::Connection(ConnectionState::BackingOff { attempt: 1, retry_in_secs: 2 }))).unwrap();
    let accounts = session.run("account.list", &json!({})).unwrap();
    assert_eq!(accounts["accounts"][0]["failure"]["kind"], json!("login_refused"));
    assert_eq!(accounts["accounts"][0]["connection"]["state"], json!("backing_off"));
    session.apply(&event(1, EventKind::Connection(ConnectionState::Online))).unwrap();
    let accounts = session.run("account.list", &json!({})).unwrap();
    assert_eq!(accounts["accounts"][0]["failure"], json!(null), "online again clears the failure");
    assert_eq!(accounts["accounts"][1]["connection"], json!(null));
}

#[test]
fn a_new_session_takes_ids_and_uidvalidity_from_the_store() {
    let mut store = mailbox();
    inboxcraft_store::Storage::apply(
        &mut store,
        &inboxcraft_store::Batch::new().with(Change::SetFolderState(FolderId::new(1), FolderState::new(validity(8)))),
    )
    .unwrap();
    let mut session = session_with(store, crate::Registry::builtin().unwrap());
    let stale = EventKind::MessagesAdded { folder: FolderId::new(1), uid_validity: validity(1), messages: vec![summary(30, "Stale")] };
    assert_eq!(session.apply(&event(1, stale)).unwrap(), Applied::Dropped);
    let fresh = EventKind::MessagesAdded { folder: FolderId::new(1), uid_validity: validity(8), messages: vec![summary(30, "Fresh")] };
    session.apply(&event(1, fresh)).unwrap();
    assert_eq!(uids(session.store(), 1), [(1, 10), (2, 11), (6, 30)]);
    session.apply(&event(1, EventKind::Folders { listings: vec![listing("INBOX", vec![], true), listing("New", vec![], true)] })).unwrap();
    let ids: Vec<u64> = session.store().folders(AccountId::new(1)).unwrap().iter().map(|folder| folder.id().get()).collect();
    assert_eq!(ids, [1, 6]);
}

#[test]
fn a_message_for_a_removed_folder_is_dropped() {
    let mut session = session();
    // The folder list removes folder 2 in the store first; a message for it then fails as a whole batch.
    session.apply(&event(1, EventKind::Folders { listings: vec![listing("INBOX", vec![], true)] })).unwrap();
    session.take_jobs();
    let added = EventKind::MessagesAdded { folder: FolderId::new(2), uid_validity: validity(1), messages: vec![summary(1, "Late")] };
    assert_eq!(session.apply(&event(1, added)).unwrap(), Applied::Dropped, "the folder is gone");
}

#[test]
fn a_role_question_goes_once_a_folder_has_the_role() {
    let mut session = session();
    let asked = |session: &mut crate::Session| -> Vec<String> {
        let answer = session.run("folder.list", &json!({ "account": 1 })).unwrap();
        answer["roles_to_ask"].as_array().unwrap().iter().map(|ask| ask["role"].as_str().unwrap().to_owned()).collect()
    };
    assert_eq!(asked(&mut session), ["drafts", "trash", "junk"]);
    let listings = vec![listing("INBOX", vec![], true), listing("Drafts", vec![SpecialUse::Drafts], true), listing("Archive", vec![], true)];
    session.apply(&event(1, EventKind::Folders { listings })).unwrap();
    assert_eq!(asked(&mut session), ["trash", "junk", "sent"], "drafts found its folder; sent lost its folder and is asked about now");
}
