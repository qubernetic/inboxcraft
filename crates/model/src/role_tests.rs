// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Role, RoleError, RoleMap, RoleSource, detect_role};
use crate::{AccountId, Folder, FolderId, FolderListing, FolderPath, SpecialUse};

fn listing(name: &str, delimiter: Option<char>, special_use: &[SpecialUse]) -> FolderListing {
    FolderListing::new(FolderPath::new(name, delimiter).unwrap(), special_use.to_vec(), true)
}

fn folder(id: u64, name: &str, special_use: &[SpecialUse]) -> Folder {
    Folder::new(FolderId::new(id), AccountId::new(1), listing(name, Some('/'), special_use))
}

/// A row of the detection table: name, delimiter, SPECIAL-USE attributes, the role found.
type Row = (&'static str, Option<char>, &'static [SpecialUse], Option<(Role, RoleSource)>);

/// Role detection, one folder at a time: SPECIAL-USE first, then the English and Hungarian names.
#[test]
fn role_detection_table() {
    use RoleSource::{Name, SpecialUse as Attribute};
    use SpecialUse as S;
    #[rustfmt::skip]
    let table: &[Row] = &[
        // The inbox, by its protocol name.
        ("INBOX", Some('/'), &[], Some((Role::Inbox, Name))),
        ("inbox", Some('.'), &[], Some((Role::Inbox, Name))),
        ("INBOX", Some('/'), &[S::Sent], Some((Role::Inbox, Name))),
        // SPECIAL-USE wins over the name.
        ("Sent", Some('/'), &[S::Sent], Some((Role::Sent, Attribute))),
        ("Outbox copies", Some('/'), &[S::Sent], Some((Role::Sent, Attribute))),
        ("Kuka", Some('/'), &[S::Junk], Some((Role::Junk, Attribute))),
        ("Mappa", Some('/'), &[S::Drafts], Some((Role::Drafts, Attribute))),
        ("Bin", Some('/'), &[S::Trash], Some((Role::Trash, Attribute))),
        ("Old", Some('/'), &[S::Archive], Some((Role::Archive, Attribute))),
        ("All Mail", Some('/'), &[S::All], Some((Role::All, Attribute))),
        ("Starred", Some('/'), &[S::Flagged], Some((Role::Flagged, Attribute))),
        ("Both", Some('/'), &[S::All, S::Archive], Some((Role::All, Attribute))),
        // English names, any case, at any depth.
        ("Sent", Some('/'), &[], Some((Role::Sent, Name))),
        ("Sent Items", Some('/'), &[], Some((Role::Sent, Name))),
        ("SENT MAIL", Some('/'), &[], Some((Role::Sent, Name))),
        ("Sent Messages", None, &[], Some((Role::Sent, Name))),
        ("INBOX.Sent", Some('.'), &[], Some((Role::Sent, Name))),
        ("[Mail]/Sent Mail", Some('/'), &[], Some((Role::Sent, Name))),
        ("Drafts", Some('/'), &[], Some((Role::Drafts, Name))),
        ("draft", Some('/'), &[], Some((Role::Drafts, Name))),
        ("Trash", Some('/'), &[], Some((Role::Trash, Name))),
        ("Deleted Items", Some('/'), &[], Some((Role::Trash, Name))),
        ("Deleted Messages", Some('/'), &[], Some((Role::Trash, Name))),
        ("Deleted", Some('/'), &[], Some((Role::Trash, Name))),
        ("Bin", Some('/'), &[], Some((Role::Trash, Name))),
        ("Junk", Some('/'), &[], Some((Role::Junk, Name))),
        ("Spam", Some('/'), &[], Some((Role::Junk, Name))),
        ("Junk E-mail", Some('/'), &[], Some((Role::Junk, Name))),
        ("Junk Email", Some('/'), &[], Some((Role::Junk, Name))),
        ("Bulk Mail", Some('/'), &[], Some((Role::Junk, Name))),
        ("Archive", Some('/'), &[], Some((Role::Archive, Name))),
        ("Archives", Some('/'), &[], Some((Role::Archive, Name))),
        // Hungarian names.
        ("Elküldött", Some('/'), &[], Some((Role::Sent, Name))),
        ("Elküldött elemek", Some('/'), &[], Some((Role::Sent, Name))),
        ("ELKÜLDÖTT LEVELEK", Some('/'), &[], Some((Role::Sent, Name))),
        ("Küldött elemek", Some('/'), &[], Some((Role::Sent, Name))),
        ("Piszkozatok", Some('/'), &[], Some((Role::Drafts, Name))),
        ("Piszkozat", Some('/'), &[], Some((Role::Drafts, Name))),
        ("Kuka", Some('/'), &[], Some((Role::Trash, Name))),
        ("Törölt elemek", Some('/'), &[], Some((Role::Trash, Name))),
        ("Törölt levelek", Some('/'), &[], Some((Role::Trash, Name))),
        ("Lomtár", Some('/'), &[], Some((Role::Trash, Name))),
        ("Levélszemét", Some('/'), &[], Some((Role::Junk, Name))),
        ("Levélszemét e-mail", Some('/'), &[], Some((Role::Junk, Name))),
        ("Kéretlen levelek", Some('/'), &[], Some((Role::Junk, Name))),
        ("Archívum", Some('/'), &[], Some((Role::Archive, Name))),
        ("Archív", Some('/'), &[], Some((Role::Archive, Name))),
        // Our own hidden folders, at the top or under the inbox.
        ("InboxCraft/Screener", Some('/'), &[], Some((Role::Screener, Name))),
        ("InboxCraft/Snoozed", Some('/'), &[], Some((Role::Snoozed, Name))),
        ("INBOX.InboxCraft.Blocked", Some('.'), &[], Some((Role::Blocked, Name))),
        ("Work/InboxCraft/Screener", Some('/'), &[], None),
        ("inboxcraft/screener", Some('/'), &[], None),
        ("InboxCraft", Some('/'), &[], None),
        // No role: not a known name, a name inside a longer one, or only white space around it.
        ("Clients", Some('/'), &[], None),
        ("Sent to the printer", Some('/'), &[], None),
        ("INBOX/Sub", Some('/'), &[], None),
        (" Sent ", Some('/'), &[], Some((Role::Sent, Name))),
        ("Sent/2026", Some('/'), &[], None),
        // The virtual folders have no name rule.
        ("All Mail", Some('/'), &[], None),
        ("Flagged", Some('/'), &[], None),
    ];
    for (name, delimiter, special_use, expected) in table {
        let folder = listing(name, *delimiter, special_use);
        assert_eq!(detect_role(&folder), *expected, "{name:?} {special_use:?}");
    }
}

#[test]
fn a_folder_that_cannot_hold_mail_gets_no_role() {
    let folder = FolderListing::new(FolderPath::new("Sent", None).unwrap(), vec![SpecialUse::Sent], false);
    assert_eq!(detect_role(&folder), None);
}

#[test]
fn roles_know_which_are_fixed_and_which_are_ours() {
    for role in [Role::Inbox, Role::Sent, Role::Drafts] {
        assert!(role.is_fixed() && !role.is_hidden(), "{role:?}");
    }
    for role in [Role::Trash, Role::Junk, Role::Archive, Role::All, Role::Flagged] {
        assert!(!role.is_fixed() && !role.is_hidden(), "{role:?}");
    }
    for role in [Role::Screener, Role::Snoozed, Role::Blocked] {
        assert!(role.is_fixed() && role.is_hidden(), "{role:?}");
    }
}

#[test]
fn detection_fills_the_map_and_asks_once_for_what_is_missing() {
    let folders = [folder(1, "INBOX", &[]), folder(2, "Elküldött", &[]), folder(3, "Bin", &[SpecialUse::Trash]), folder(4, "Clients", &[])];
    let mut roles = RoleMap::default();
    assert_eq!(roles.detect(&folders), [Role::Drafts, Role::Junk, Role::Archive]);
    assert_eq!(roles.folder(Role::Inbox), Some(FolderId::new(1)));
    assert_eq!(roles.folder(Role::Sent), Some(FolderId::new(2)));
    assert_eq!(roles.source(Role::Sent), Some(RoleSource::Name));
    assert_eq!(roles.folder(Role::Trash), Some(FolderId::new(3)));
    assert_eq!(roles.source(Role::Trash), Some(RoleSource::SpecialUse));
    assert_eq!(roles.role_of(FolderId::new(4)), None);
    assert_eq!(roles.role_of(FolderId::new(3)), Some(Role::Trash));
    // Asked once: the next sync does not ask again.
    assert_eq!(roles.detect(&folders), []);
    assert!(roles.was_asked(Role::Drafts));
    assert!(!roles.was_asked(Role::Sent));
}

#[test]
fn special_use_on_one_folder_beats_a_name_on_another() {
    let folders = [folder(1, "Sent", &[]), folder(2, "Outgoing", &[SpecialUse::Sent])];
    let mut roles = RoleMap::default();
    roles.detect(&folders);
    assert_eq!(roles.folder(Role::Sent), Some(FolderId::new(2)));
    assert_eq!(roles.role_of(FolderId::new(1)), None);
}

#[test]
fn the_first_folder_wins_and_a_folder_takes_one_role() {
    let folders = [folder(1, "Spam", &[]), folder(2, "Junk", &[]), folder(3, "Trash", &[SpecialUse::Archive]), folder(4, "Deleted Items", &[])];
    let mut roles = RoleMap::default();
    roles.detect(&folders);
    assert_eq!(roles.folder(Role::Junk), Some(FolderId::new(1)));
    assert_eq!(roles.folder(Role::Archive), Some(FolderId::new(3)));
    assert_eq!(roles.folder(Role::Trash), Some(FolderId::new(4)));
}

#[test]
fn detection_never_moves_a_role_but_frees_one_whose_folder_is_gone() {
    let mut roles = RoleMap::default();
    roles.detect(&[folder(1, "Trash", &[]), folder(2, "Archive", &[])]);
    assert_eq!(roles.detect(&[folder(1, "Trash", &[]), folder(2, "Archive", &[]), folder(5, "Bin", &[SpecialUse::Trash])]), []);
    assert_eq!(roles.folder(Role::Trash), Some(FolderId::new(1)));
    // Folder 1 is gone: the role goes to the next candidate.
    roles.detect(&[folder(2, "Archive", &[]), folder(5, "Bin", &[SpecialUse::Trash])]);
    assert_eq!(roles.folder(Role::Trash), Some(FolderId::new(5)));
    // Folder 2 can no longer hold mail and folder 5 is gone: both roles are free, never asked about, so asked now,
    // once.
    let gone = Folder::new(FolderId::new(2), AccountId::new(1), FolderListing::new(FolderPath::new("Archive", None).unwrap(), vec![], false));
    assert_eq!(roles.detect(std::slice::from_ref(&gone)), [Role::Trash, Role::Archive]);
    assert_eq!(roles.folder(Role::Archive), None);
    assert_eq!(roles.detect(&[gone]), []);
}

#[test]
fn the_owner_answers_a_question() {
    let folders = [folder(1, "INBOX", &[]), folder(2, "Papers", &[]), folder(3, "Old mail", &[])];
    let mut roles = RoleMap::default();
    assert!(roles.detect(&folders).contains(&Role::Archive));
    assert_eq!(roles.choose(Role::Archive, FolderId::new(3)), Ok(()));
    assert_eq!(roles.source(Role::Archive), Some(RoleSource::Owner));
    // Archive can be changed; the owner's choice survives detection.
    assert_eq!(roles.choose(Role::Archive, FolderId::new(2)), Ok(()));
    roles.detect(&[folder(1, "INBOX", &[]), folder(2, "Papers", &[]), folder(3, "Archive", &[])]);
    assert_eq!(roles.folder(Role::Archive), Some(FolderId::new(2)));
    // Clearing a changeable role.
    assert_eq!(roles.clear(Role::Archive), Ok(()));
    assert_eq!(roles.folder(Role::Archive), None);
}

#[test]
fn fixed_roles_and_our_own_roles_cannot_be_changed() {
    let folders = [folder(1, "INBOX", &[]), folder(2, "Sent", &[]), folder(3, "Other", &[]), folder(4, "More", &[])];
    let mut roles = RoleMap::default();
    roles.detect(&folders);
    assert_eq!(roles.choose(Role::Sent, FolderId::new(3)), Err(RoleError::Fixed(Role::Sent)));
    assert_eq!(roles.choose(Role::Inbox, FolderId::new(3)), Err(RoleError::Fixed(Role::Inbox)));
    assert_eq!(roles.clear(Role::Sent), Err(RoleError::Fixed(Role::Sent)));
    assert_eq!(roles.choose(Role::Screener, FolderId::new(3)), Err(RoleError::OurOwn(Role::Screener)));
    assert_eq!(roles.clear(Role::Blocked), Err(RoleError::OurOwn(Role::Blocked)));
    // Drafts was not found: the owner sets it once, then it is fixed.
    assert_eq!(roles.choose(Role::Drafts, FolderId::new(3)), Ok(()));
    assert_eq!(roles.choose(Role::Drafts, FolderId::new(4)), Err(RoleError::Fixed(Role::Drafts)));
    // A folder holds one role.
    assert_eq!(roles.choose(Role::Trash, FolderId::new(2)), Err(RoleError::FolderTaken(Role::Sent)));
    assert_eq!(roles.choose(Role::Trash, FolderId::new(4)), Ok(()));
    assert_eq!(roles.choose(Role::Trash, FolderId::new(4)), Ok(()));
}

#[test]
fn roles_have_english_names_for_logs() {
    let every = [
        Role::Inbox,
        Role::Sent,
        Role::Drafts,
        Role::Trash,
        Role::Junk,
        Role::Archive,
        Role::All,
        Role::Flagged,
        Role::Screener,
        Role::Snoozed,
        Role::Blocked,
    ];
    let names: Vec<String> = every.iter().map(ToString::to_string).collect();
    assert_eq!(names, ["Inbox", "Sent", "Drafts", "Trash", "Junk", "Archive", "All", "Flagged", "Screener", "Snoozed", "Blocked"]);
}

#[test]
fn role_errors_read_as_sentences() {
    assert_eq!(RoleError::Fixed(Role::Sent).to_string(), "the Sent folder is fixed once set");
    assert_eq!(RoleError::OurOwn(Role::Screener).to_string(), "the Screener folder is InboxCraft's own");
    assert_eq!(RoleError::FolderTaken(Role::Trash).to_string(), "the folder is the Trash folder already");
}

#[test]
fn the_role_map_serialises() {
    let mut roles = RoleMap::default();
    roles.detect(&[folder(1, "INBOX", &[]), folder(2, "Sent", &[SpecialUse::Sent])]);
    let json = serde_json::to_string(&roles).unwrap();
    assert_eq!(serde_json::from_str::<RoleMap>(&json).unwrap(), roles);
}
