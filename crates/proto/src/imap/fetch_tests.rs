// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{ModSeq, Uid};
use inboxcraft_time::{CivilDateTime, Offset};

use super::{FetchAttributes, FetchItem, MAX_PREVIEW_CHARS, PreviewFetch, Section, internal_date};
use crate::imap::value::{Piece, values};

fn item(pieces: Vec<Piece>) -> Option<FetchItem> {
    FetchItem::from_values(values(pieces).unwrap_or_default())
}

fn text(s: &str) -> Piece {
    Piece::Text(s.as_bytes().to_vec())
}

#[test]
fn the_items_asked_for() {
    assert_eq!(FetchAttributes::default().items(), "(UID)");
    let all =
        FetchAttributes { flags: true, internal_date: true, size: true, modseq: true, section: Some(Section::Full), ..FetchAttributes::default() };
    assert_eq!(all.items(), "(UID FLAGS INTERNALDATE RFC822.SIZE MODSEQ BODY.PEEK[])");
    assert_eq!(all.modifiers(), None);
    assert_eq!(FetchAttributes { section: Some(Section::Header), ..FetchAttributes::default() }.items(), "(UID BODY.PEEK[HEADER])");
    assert_eq!(FetchAttributes { section: Some(Section::Text), flags: true, ..FetchAttributes::default() }.items(), "(UID FLAGS BODY.PEEK[TEXT])");
}

#[test]
fn changed_since_asks_for_the_modseq_and_vanished_goes_with_it() {
    let since = FetchAttributes { flags: true, changed_since: ModSeq::new(12345), ..FetchAttributes::default() };
    assert_eq!(since.items(), "(UID FLAGS MODSEQ)");
    assert_eq!(since.modifiers().as_deref(), Some("(CHANGEDSINCE 12345)"));
    assert_eq!(FetchAttributes { vanished: true, ..since }.modifiers().as_deref(), Some("(CHANGEDSINCE 12345 VANISHED)"));
    assert_eq!(FetchAttributes { vanished: true, ..FetchAttributes::default() }.modifiers(), None);
}

#[test]
fn a_modseq_is_one_number_in_parentheses() {
    let modseq = |text: &str| item(vec![self::text(text)]).and_then(|item| item.modseq);
    assert_eq!(modseq("1 FETCH (MODSEQ (624140003))"), ModSeq::new(624_140_003));
    assert_eq!(modseq("1 FETCH (MODSEQ (18446744073709551615))"), ModSeq::new(u64::MAX));
    for bad in ["1 FETCH (MODSEQ 5)", "1 FETCH (MODSEQ ())", "1 FETCH (MODSEQ (0))", "1 FETCH (MODSEQ (\"5\"))", "1 FETCH (MODSEQ (x))"] {
        assert_eq!(modseq(bad), None, "{bad}");
    }
}

#[test]
fn a_preview_is_one_clean_line_of_at_most_256_characters() {
    let preview = |pieces: Vec<Piece>| item(pieces).and_then(|item| item.preview);
    let long = "é".repeat(MAX_PREVIEW_CHARS + 10);
    let cut = preview(vec![text(&format!("1 FETCH (PREVIEW \"{long}\")"))]);
    assert_eq!(cut, Some("é".repeat(MAX_PREVIEW_CHARS)));
    let raw = preview(vec![text("1 FETCH (PREVIEW {9}"), Piece::Literal(b"a\0b\x1b\xff\r\n c".to_vec()), text(")")]);
    assert_eq!(raw.as_deref(), Some("a b \u{fffd} c"));
    assert_eq!(preview(vec![text("1 FETCH (PREVIEW NIL)")]), None);
    assert_eq!(preview(vec![text("1 FETCH (PREVIEW (x))")]), None);
    let both = FetchAttributes { preview: Some(PreviewFetch::Lazy), section: Some(Section::Header), ..FetchAttributes::default() };
    assert_eq!(both.items(), "(UID PREVIEW (LAZY) BODY.PEEK[HEADER])");
}

#[test]
fn a_fetch_response_in_any_order() {
    let pieces = vec![
        text(r#"12 FETCH (FLAGS (\Seen $Work) UID 40 INTERNALDATE "17-Jul-1996 02:44:25 -0700" RFC822.SIZE 4286 BODY[HEADER] {9}"#),
        Piece::Literal(b"Subject: ".to_vec()),
        text(")"),
    ];
    let date = CivilDateTime::new(1996, 7, 17, 9, 44, 25).and_then(|civil| civil.to_timestamp(Offset::UTC));
    assert_eq!(
        item(pieces),
        Some(FetchItem {
            seq: 12,
            uid: Uid::new(40),
            flags: Some(vec!["\\Seen".into(), "$Work".into()]),
            internal_date: date,
            size: Some(4286),
            modseq: None,
            preview: None,
            section: Some((Section::Header, b"Subject: ".to_vec())),
        })
    );
}

#[test]
fn sections_by_every_name() {
    for (name, section) in [
        ("BODY[]", Section::Full),
        ("RFC822", Section::Full),
        ("body[header]", Section::Header),
        ("RFC822.HEADER", Section::Header),
        ("BODY[TEXT]", Section::Text),
        ("RFC822.TEXT", Section::Text),
    ] {
        let found = item(vec![text(&format!("1 FETCH ({name} \"x\")"))]).and_then(|item| item.section);
        assert_eq!(found, Some((section, b"x".to_vec())), "{name}");
    }
    // A partial body, an unknown section and NIL are not taken.
    for line in ["1 FETCH (BODY[]<0> \"x\")", "1 FETCH (BODY[1] \"x\")", "1 FETCH (BODY[] NIL)", "1 FETCH (ENVELOPE (NIL))"] {
        assert_eq!(item(vec![text(line)]).map(|item| item.section), Some(None), "{line}");
    }
}

#[test]
fn values_that_cannot_be_read_are_left_out() {
    let found = item(vec![text("3 FETCH (UID 0 FLAGS \\Seen INTERNALDATE \"yesterday\" RFC822.SIZE x (junk) MODSEQ (0) UID)")]);
    assert_eq!(found, Some(FetchItem { seq: 3, ..FetchItem::default() }));
    for line in ["", "0 FETCH (UID 1)", "x FETCH (UID 1)", "1 STORE (UID 1)", "1 FETCH UID 1", "1 FETCH", "1"] {
        assert_eq!(item(vec![text(line)]), None, "{line:?}");
    }
}

#[test]
fn internal_dates() {
    let at = |y, mo, d, h, mi, s| CivilDateTime::new(y, mo, d, h, mi, s).and_then(|civil| civil.to_timestamp(Offset::UTC));
    assert_eq!(internal_date(b"17-Jul-1996 02:44:25 -0700"), at(1996, 7, 17, 9, 44, 25));
    assert_eq!(internal_date(b" 1-jan-2026 00:00:00 +0130"), at(2025, 12, 31, 22, 30, 0));
    assert_eq!(internal_date(b"01-Dec-2026 23:59:59 +0000"), at(2026, 12, 1, 23, 59, 59));
    for junk in [
        "",
        "17-Jul-1996",
        "17-Jul-1996 02:44:25",
        "17-Jul-1996 02:44:25 0700",
        "17-Jul-1996 02:44:25 -070",
        "17-Jul-1996 02:44:25 -0760",
        "17-Jul-1996 02:44:25 -07000",
        "17-Jul-1996 2:44:25 -0700",
        "17-Jul-1996 02:44 -0700",
        "117-Jul-1996 02:44:25 -0700",
        "17-Jux-1996 02:44:25 -0700",
        "17-Jul-96 02:44:25 -0700",
        "17-Jul-1996-1 02:44:25 -0700",
        "31-Feb-1996 02:44:25 -0700",
        "17-Jul-1996 25:44:25 -0700",
        "17-Jul",
        "17-Jul-1996 02:44:25 +",
    ] {
        assert_eq!(internal_date(junk.as_bytes()), None, "{junk:?}");
    }
    assert_eq!(internal_date(&[0xFF, b' ']), None);
}
