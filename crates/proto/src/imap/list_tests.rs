// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::SpecialUse;

use super::{ListEntry, MAX_MAILBOX_NAME};
use crate::imap::value::{Piece, values};

fn entry(line: &str) -> Option<ListEntry> {
    ListEntry::from_values(values(vec![Piece::Text(line.as_bytes().to_vec())]).unwrap_or_default())
}

#[test]
fn a_mailbox_with_its_special_use() {
    let sent = entry(r#"LIST (\HasNoChildren \Sent \sent) "/" "Elk&APw-ld&APY-tt""#);
    assert_eq!(
        sent,
        Some(ListEntry {
            raw: "Elk&APw-ld&APY-tt".into(),
            name: "Elküldött".into(),
            delimiter: Some('/'),
            attributes: vec!["\\HasNoChildren".into(), "\\Sent".into(), "\\sent".into()],
            special_use: vec![SpecialUse::Sent],
        })
    );
    let sent = sent.unwrap();
    assert!(sent.is_selectable());
    let listing = sent.listing();
    assert_eq!(listing.as_ref().map(|l| l.special_use().to_vec()), Ok(vec![SpecialUse::Sent]));
    assert_eq!(listing.as_ref().map(|l| l.path().name().to_owned()), Ok("Elküldött".to_owned()));
}

#[test]
fn inbox_names_and_delimiters() {
    let inbox = entry(r#"list () "." inbox"#);
    assert_eq!(inbox.as_ref().map(|e| (e.raw.as_str(), e.name.as_str(), e.delimiter)), Some(("inbox", "INBOX", Some('.'))));
    let flat = entry(r#"LIST (\Noselect) NIL Archive"#);
    assert_eq!(flat.as_ref().map(|e| (e.delimiter, e.is_selectable())), Some((None, false)));
    assert_eq!(entry(r#"LIST (\NonExistent) "/" Gone"#).map(|e| e.is_selectable()), Some(false));
    assert_eq!(entry(r#"LIST () "\\" Work"#).and_then(|e| e.delimiter), Some('\\'));
    let literal = ListEntry::from_values(
        values(vec![Piece::Text(b"LIST () \"/\" {5}".to_vec()), Piece::Literal("Kép1".as_bytes().to_vec()), Piece::Text(b" (CHILDINFO)".to_vec())])
            .unwrap_or_default(),
    );
    assert_eq!(literal.map(|e| e.raw), Some("Kép1".into()));
}

#[test]
fn what_is_no_mailbox_is_left_out() {
    for line in [
        "",
        "LSUB () \"/\" x",
        "LIST",
        "LIST x \"/\" a",
        "LIST ()",
        "LIST () \"//\" a",
        "LIST () \"\" a",
        "LIST () \"\u{7}\" a",
        "LIST () Slash a",
        "LIST () (x) a",
        "LIST () \"/\"",
        "LIST () \"/\" (a)",
        "LIST () \"/\" \"\"",
        "LIST () \"/\" \"a\tb\"",
    ] {
        assert_eq!(entry(line), None, "{line:?}");
    }
    let long = "x".repeat(MAX_MAILBOX_NAME + 1);
    assert_eq!(entry(&format!("LIST () \"/\" {long}")), None);
    let fits = "x".repeat(MAX_MAILBOX_NAME);
    assert!(entry(&format!("LIST () \"/\" {fits}")).is_some());
}

#[test]
fn a_name_the_model_refuses_is_an_error_there() {
    // A tab, encoded: the raw name is clean, the decoded one is not.
    let entry = entry("LIST () \"/\" a&AAk-b").map(|e| e.listing().is_err());
    assert_eq!(entry, Some(true));
}
