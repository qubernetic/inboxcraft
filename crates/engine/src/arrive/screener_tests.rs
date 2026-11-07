// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The screener stage: who is known, and where the first mail of an unknown sender goes in each mode.

use inboxcraft_model::{
    AccountId, Copied, EmailAddress, Envelope, Event, EventKind, FolderId, FolderListing, FolderPath, FolderState, JobKind, Message, Screening,
    Sender, SpecialUse, Summary,
};
use inboxcraft_store::Change;
use serde_json::{Value, json};

use crate::Session;
use crate::cmd::act_tests::{session, uid, validity};
use crate::test_support::{address, change, folder};

/// A message from `from` (name, address) with UID `n`.
pub(crate) fn from(n: u32, sender: &str) -> Summary {
    Summary::new(uid(n), Envelope::new("Hello").with_from(vec![address("Someone", sender)]).with_to(vec![address("Anna", "anna@example.org")]))
}

/// New mail in folder `folder` of account 1 at UIDVALIDITY 7.
pub(crate) fn arrive(session: &mut Session, folder: u64, messages: Vec<Summary>) {
    let event = Event::new(AccountId::new(1), None, EventKind::MessagesAdded { folder: FolderId::new(folder), uid_validity: validity(), messages });
    session.apply(&event).unwrap();
}

/// Answers the moves handed out as a server with `COPYUID` would, each message keeping its UID number.
pub(crate) fn settle(session: &mut Session) {
    for job in session.take_jobs() {
        if let JobKind::Move { folder, uid_validity, uids, to, .. } = job.kind {
            let pairs = uids.ranges().iter().flat_map(|(low, high)| (low.get()..=high.get()).map(|n| (uid(n), uid(n)))).collect();
            let copied = Some(Copied { uid_validity: validity(), uids: pairs });
            session.apply(&Event::new(job.account, Some(job.id), EventKind::Moved { folder, uid_validity, uids, to, copied })).unwrap();
        }
    }
}

pub(crate) fn mode(session: &mut Session, mode: &str) {
    session.run("screener.mode", &json!({ "mode": mode })).unwrap();
}

/// The message with UID `n` that arrived in account 1, wherever it is now.
pub(crate) fn arrived(session: &Session, n: u32) -> Message {
    let mut found = Vec::new();
    for folder in session.store().folders(AccountId::new(1)).unwrap() {
        for entry in session.store().messages(folder.id()).unwrap() {
            let moving = entry.message().moving().map(|place| place.uid);
            if entry.message().summary().uid() == uid(n) && moving.is_none() || moving == Some(uid(n)) {
                found.push(entry.message().clone());
            }
        }
    }
    assert_eq!(found.len(), 1, "{found:?}");
    found.remove(0)
}

/// The path of the folder `id` of account 1.
pub(crate) fn path_of(session: &Session, id: FolderId) -> String {
    let folders = session.store().folders(AccountId::new(1)).unwrap();
    folders.iter().find(|folder| folder.id() == id).map(|folder| folder.listing().path().name().to_owned()).unwrap()
}

pub(crate) fn sender(session: &Session, address: &str) -> Option<Sender> {
    session.store().sender(&EmailAddress::new(address).unwrap()).unwrap()
}

pub(crate) fn known(address: &str) -> Change {
    Change::PutSender(Sender { known: true, ..Sender::new(EmailAddress::new(address).unwrap()) })
}

pub(crate) fn decided(address: &str, screening: Screening) -> Change {
    Change::PutSender(Sender { screening: Some(screening), ..Sender::new(EmailAddress::new(address).unwrap()) })
}

fn queued(session: &Session) -> Vec<JobKind> {
    session.store().queued_jobs().unwrap().into_iter().map(|job| job.kind).collect()
}

fn listed(session: &mut Session, params: &Value) -> Vec<(u64, bool)> {
    let answer = session.run("mail.list", params).unwrap();
    answer["messages"].as_array().unwrap().iter().map(|message| (message["id"].as_u64().unwrap(), message["new_sender"] == json!(true))).collect()
}

#[test]
fn the_screener_is_off_until_the_owner_turns_it_on() {
    let mut session = session();
    arrive(&mut session, 1, vec![from(20, "new@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(1));
    assert!(sender(&session, "new@example.com").unwrap().known, "mail that came in makes its sender known");
}

#[test]
fn before_the_inbox_the_first_mail_of_an_unknown_sender_is_held_in_a_folder_made_for_it() {
    let mut session = session();
    mode(&mut session, "before");
    session.take_jobs();
    arrive(&mut session, 1, vec![from(20, "new@example.com"), from(21, "New@Example.com"), from(22, "other@example.com")]);
    let held = arrived(&session, 20);
    assert_ne!(held.folder(), FolderId::new(1));
    assert_eq!(path_of(&session, held.folder()), "InboxCraft/Screener");
    assert_eq!(arrived(&session, 21).folder(), held.folder());
    assert_eq!(arrived(&session, 22).folder(), held.folder());
    let ours: Vec<u32> = (20..23).map(|n| arrived(&session, n).summary().uid().get()).collect();
    assert_eq!(ours, [u32::MAX, u32::MAX - 1, u32::MAX - 2], "each takes a UID of its own");
    assert_eq!(sender(&session, "new@example.com"), None, "nothing is known of a held sender");
    let jobs = queued(&session);
    assert!(matches!(&jobs[0], JobKind::CreateFolder { path } if path.name() == "InboxCraft/Screener"), "{jobs:?}");
    assert_eq!(jobs.iter().filter(|kind| matches!(kind, JobKind::Move { to, .. } if *to == held.folder())).count(), 3);
    assert_eq!(jobs.len(), 4, "the folder is made once");
    let list = session.run("mail.list", &json!({ "role": "screener" })).unwrap();
    assert_eq!(list["total"], json!(3), "the screener folder has its role at once: {list}");
}

#[test]
fn a_known_or_accepted_sender_comes_in() {
    let mut session = session();
    mode(&mut session, "before");
    change(&mut session, known("friend@example.com"));
    change(&mut session, decided("accepted@example.com", Screening::Accepted));
    arrive(&mut session, 1, vec![from(20, "Friend@example.com"), from(21, "accepted@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(1));
    assert_eq!(arrived(&session, 21).folder(), FolderId::new(1));
    assert!(queued(&session).is_empty());
}

#[test]
fn a_blocked_sender_goes_to_the_blocked_folder_in_every_mode() {
    for name in ["before", "inside", "off"] {
        let mut session = session();
        mode(&mut session, name);
        change(&mut session, decided("spam@example.com", Screening::Blocked));
        arrive(&mut session, 1, vec![from(20, "spam@example.com")]);
        let blocked = arrived(&session, 20);
        assert_eq!(path_of(&session, blocked.folder()), "InboxCraft/Blocked", "{name}");
        assert_eq!(sender(&session, "spam@example.com").unwrap().screening, Some(Screening::Blocked));
    }
}

#[test]
fn inside_the_inbox_new_mail_stays_and_is_marked() {
    let mut session = session();
    mode(&mut session, "inside");
    arrive(&mut session, 1, vec![from(20, "new@example.com")]);
    let message = arrived(&session, 20);
    assert_eq!(message.folder(), FolderId::new(1));
    assert_eq!(sender(&session, "new@example.com"), None);
    let list = listed(&mut session, &json!({}));
    assert!(list.contains(&(message.id().get(), true)), "{list:?}");
    assert!(list.contains(&(1, false)), "anna@example.org is the account's own address: {list:?}");
    mode(&mut session, "off");
    assert!(listed(&mut session, &json!({})).iter().all(|(_, new)| !new), "no mark with the screener off");
}

#[test]
fn delivery_reports_and_the_owners_own_mail_are_never_held() {
    let mut session = session();
    mode(&mut session, "before");
    arrive(&mut session, 1, vec![from(20, "MAILER-DAEMON@example.com"), from(21, "postmaster@example.net"), from(22, "bob@example.net")]);
    let none = Summary::new(uid(23), Envelope::new("No sender"));
    arrive(&mut session, 1, vec![none]);
    for n in 20..24 {
        assert_eq!(arrived(&session, n).folder(), FolderId::new(1), "{n}");
    }
    assert_eq!(sender(&session, "mailer-daemon@example.com"), None, "a report makes nobody known");
    mode(&mut session, "inside");
    assert!(listed(&mut session, &json!({})).iter().all(|(_, new)| !new));
}

#[test]
fn the_first_sync_of_a_folder_learns_its_senders_and_holds_nothing() {
    let mut session = session();
    mode(&mut session, "before");
    change(&mut session, Change::PutFolder(folder(8, 1, "Projects", vec![])));
    arrive(&mut session, 8, vec![from(20, "old@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(8));
    assert!(sender(&session, "old@example.com").unwrap().known);
    // An inbox never synced before: its mail is the owner's already.
    let mut session = crate::test_support::session();
    mode(&mut session, "before");
    arrive(&mut session, 1, vec![from(20, "older@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(1));
    assert!(sender(&session, "older@example.com").unwrap().known);
}

#[test]
fn later_mail_in_another_folder_changes_nothing() {
    let mut session = session();
    mode(&mut session, "before");
    arrive(&mut session, 3, vec![from(20, "list@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(3));
    assert_eq!(sender(&session, "list@example.com"), None);
    arrive(&mut session, 6, vec![from(21, "junk@example.com")]);
    assert_eq!(sender(&session, "junk@example.com"), None);
}

#[test]
fn writing_to_someone_makes_them_known() {
    let mut session = session();
    mode(&mut session, "before");
    change(&mut session, decided("blocked@example.com", Screening::Blocked));
    let envelope = Envelope::new("Hi")
        .with_from(vec![address("Anna", "anna@example.org")])
        .with_to(vec![address("Cecil", "cecil@example.com"), address("Bob", "bob@example.net")])
        .with_cc(vec![address("Dora", "dora@example.com")])
        .with_bcc(vec![address("Blocked", "blocked@example.com")]);
    arrive(&mut session, 2, vec![Summary::new(uid(30), envelope)]);
    for address in ["cecil@example.com", "dora@example.com"] {
        assert!(sender(&session, address).unwrap().known, "{address}");
    }
    assert_eq!(sender(&session, "anna@example.org"), None, "not the owner's own address");
    assert_eq!(sender(&session, "bob@example.net"), None, "nor another account's");
    let blocked = sender(&session, "blocked@example.com").unwrap();
    assert!(blocked.known && blocked.screening == Some(Screening::Blocked), "writing to a blocked sender does not take the block off");
    arrive(&mut session, 1, vec![from(20, "cecil@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(1));
}

#[test]
fn a_screener_folder_the_server_has_is_used() {
    let mut session = session();
    mode(&mut session, "before");
    let listings = vec![
        FolderListing::new(FolderPath::new("INBOX", Some('/')).unwrap(), vec![], true),
        FolderListing::new(FolderPath::new("InboxCraft/Screener", Some('/')).unwrap(), vec![], true),
    ];
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Folders { listings })).unwrap();
    let screener = session.store().folders(AccountId::new(1)).unwrap().into_iter().find(|folder| folder.listing().path().name() != "INBOX").unwrap();
    session.store_mut().apply(&inboxcraft_store::Batch::new().with(Change::SetFolderState(screener.id(), FolderState::new(validity())))).unwrap();
    arrive(&mut session, 1, vec![from(20, "new@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), screener.id());
    assert!(!queued(&session).iter().any(|kind| matches!(kind, JobKind::CreateFolder { .. })));
}

#[test]
fn a_folder_list_before_the_folder_is_made_keeps_it_and_does_not_sync_it() {
    let mut session = session();
    mode(&mut session, "before");
    arrive(&mut session, 1, vec![from(20, "new@example.com")]);
    let held = arrived(&session, 20);
    session.take_jobs();
    let listings = vec![
        FolderListing::new(FolderPath::new("INBOX", Some('/')).unwrap(), vec![], true),
        FolderListing::new(FolderPath::new("Sent", Some('/')).unwrap(), vec![SpecialUse::Sent], true),
    ];
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Folders { listings })).unwrap();
    assert_eq!(arrived(&session, 20), held, "the folder and its message stay");
    let syncs: Vec<FolderId> = session
        .take_jobs()
        .into_iter()
        .filter_map(|job| match job.kind {
            JobKind::SyncFolder { folder, .. } => Some(folder),
            _ => None,
        })
        .collect();
    assert!(!syncs.contains(&held.folder()), "{syncs:?}");
    let list = session.run("mail.list", &json!({ "role": "screener" })).unwrap();
    assert_eq!(list["total"], json!(1));
}

#[test]
fn the_folder_goes_under_the_inbox_where_every_folder_is() {
    let mut session = crate::test_support::session();
    let account = AccountId::new(1);
    let inbox = folder(1, 1, "INBOX", vec![]);
    let dotted = |id: u64, name: &str| {
        inboxcraft_model::Folder::new(FolderId::new(id), account, FolderListing::new(FolderPath::new(name, Some('.')).unwrap(), vec![], true))
    };
    let batch = inboxcraft_store::Batch::new()
        .with(Change::PutFolder(inboxcraft_model::Folder::new(inbox.id(), account, FolderListing::new(FolderPath::new("INBOX", Some('.')).unwrap(), vec![], true))))
        .with(Change::PutFolder(dotted(2, "INBOX.Sent")))
        .with(Change::RemoveFolder(FolderId::new(3)))
        .with(Change::SetFolderState(FolderId::new(1), FolderState::new(validity())));
    session.store_mut().apply(&batch).unwrap();
    mode(&mut session, "before");
    arrive(&mut session, 1, vec![from(20, "new@example.com")]);
    assert_eq!(path_of(&session, arrived(&session, 20).folder()), "INBOX.InboxCraft.Screener");
}

#[test]
fn without_a_hierarchy_nothing_is_held() {
    let mut session = crate::test_support::session();
    let flat = inboxcraft_model::Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", None).unwrap(), vec![], true));
    let batch = inboxcraft_store::Batch::new().with(Change::PutFolder(flat)).with(Change::SetFolderState(FolderId::new(1), FolderState::new(validity())));
    session.store_mut().apply(&batch).unwrap();
    mode(&mut session, "before");
    arrive(&mut session, 1, vec![from(20, "new@example.com")]);
    assert_eq!(arrived(&session, 20).folder(), FolderId::new(1));
    assert_eq!(sender(&session, "new@example.com"), None, "still unknown, so the list marks it");
}
