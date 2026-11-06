// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Folder, FolderError, FolderListing, FolderPath, SpecialUse};
use crate::{AccountId, FolderId};

#[test]
fn a_path_splits_at_its_delimiter() {
    let path = FolderPath::new("INBOX.Archive.2026", Some('.')).unwrap();
    assert_eq!(path.name(), "INBOX.Archive.2026");
    assert_eq!(path.delimiter(), Some('.'));
    assert_eq!(path.components().collect::<Vec<_>>(), ["INBOX", "Archive", "2026"]);
    assert_eq!(path.leaf(), "2026");
    let flat = FolderPath::new("Sent Items", None).unwrap();
    assert_eq!(flat.components().collect::<Vec<_>>(), ["Sent Items"]);
    assert_eq!(flat.leaf(), "Sent Items");
    assert_eq!(flat.to_string(), "Sent Items");
}

#[test]
fn inbox_is_inbox_in_any_case_and_only_at_the_top() {
    for name in ["INBOX", "inbox", "Inbox"] {
        assert!(FolderPath::new(name, Some('/')).unwrap().is_inbox(), "{name}");
    }
    assert!(!FolderPath::new("INBOX/Sub", Some('/')).unwrap().is_inbox());
    assert!(!FolderPath::new("Work/INBOX", Some('/')).unwrap().is_inbox());
}

#[test]
fn broken_paths_are_errors() {
    assert_eq!(FolderPath::new("", Some('/')), Err(FolderError::Empty));
    assert_eq!(FolderPath::new("a\r\nb", Some('/')), Err(FolderError::ControlCharacter));
    assert_eq!(FolderPath::new("a\0", None), Err(FolderError::ControlCharacter));
    assert_eq!(FolderPath::new("a", Some('\n')), Err(FolderError::BadDelimiter));
    assert_eq!(FolderPath::new(&"a".repeat(FolderPath::MAX_LEN + 1), None), Err(FolderError::TooLong));
    assert!(FolderPath::new(&"a".repeat(FolderPath::MAX_LEN), None).is_ok());
    for error in [FolderError::Empty, FolderError::TooLong, FolderError::ControlCharacter, FolderError::BadDelimiter] {
        assert!(error.to_string().starts_with("the folder"), "{error}");
    }
}

#[test]
fn paths_are_checked_on_the_way_in() {
    let path = FolderPath::new("Work/Clients", Some('/')).unwrap();
    let json = serde_json::to_string(&path).unwrap();
    assert_eq!(serde_json::from_str::<FolderPath>(&json).unwrap(), path);
    assert!(serde_json::from_str::<FolderPath>(r#"{"name":"a\r","delimiter":"/"}"#).is_err());
}

#[test]
fn special_use_attributes_read_without_case() {
    let cases = [
        ("\\All", Some(SpecialUse::All)),
        ("\\archive", Some(SpecialUse::Archive)),
        ("\\Drafts", Some(SpecialUse::Drafts)),
        ("\\FLAGGED", Some(SpecialUse::Flagged)),
        ("\\Junk", Some(SpecialUse::Junk)),
        ("\\Sent", Some(SpecialUse::Sent)),
        ("\\Trash", Some(SpecialUse::Trash)),
        ("\\Noselect", None),
        ("\\HasChildren", None),
        ("Sent", None),
        ("", None),
    ];
    for (text, special_use) in cases {
        assert_eq!(SpecialUse::from_attribute(text), special_use, "{text:?}");
    }
}

#[test]
fn a_folder_is_a_listing_with_our_ids() {
    let listing = FolderListing::new(FolderPath::new("Sent", None).unwrap(), vec![SpecialUse::Sent], true);
    let folder = Folder::new(FolderId::new(3), AccountId::new(1), listing.clone());
    assert_eq!(folder.id(), FolderId::new(3));
    assert_eq!(folder.account(), AccountId::new(1));
    assert_eq!(folder.listing(), &listing);
    assert_eq!(listing.path().name(), "Sent");
    assert_eq!(listing.special_use(), [SpecialUse::Sent]);
    assert!(listing.is_selectable());
    let json = serde_json::to_string(&folder).unwrap();
    assert_eq!(serde_json::from_str::<Folder>(&json).unwrap(), folder);
}

#[test]
fn a_listing_keeps_a_few_special_uses_once_each() {
    let many = vec![SpecialUse::Sent; 1000];
    let listing = FolderListing::new(FolderPath::new("Sent", None).unwrap(), many, true);
    assert_eq!(listing.special_use(), [SpecialUse::Sent]);
}
