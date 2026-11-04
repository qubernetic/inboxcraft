// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_DEPTH, Piece, Status, StatusLine, Value, literal_marker, number, values};
use crate::ProtoError;

fn text(s: &str) -> Piece {
    Piece::Text(s.as_bytes().to_vec())
}

fn atom(s: &str) -> Value {
    Value::Atom(s.into())
}

fn string(s: &str) -> Value {
    Value::Str(s.as_bytes().to_vec())
}

fn parse(s: &str) -> Vec<Value> {
    values(vec![text(s)]).unwrap_or_default()
}

#[test]
fn atoms_strings_and_lists() {
    assert_eq!(
        parse(r#"LIST (\HasNoChildren \Sent) "/" "Sent \"old\" \\ mail""#),
        vec![atom("LIST"), Value::List(vec![atom("\\HasNoChildren"), atom("\\Sent")]), string("/"), string("Sent \"old\" \\ mail")]
    );
    assert_eq!(
        parse("12 FETCH (UID 7 FLAGS ())"),
        vec![atom("12"), atom("FETCH"), Value::List(vec![atom("UID"), atom("7"), atom("FLAGS"), Value::List(vec![])])]
    );
    assert_eq!(
        parse("a ((b) (c (d))) NIL"),
        vec![atom("a"), Value::List(vec![Value::List(vec![atom("b")]), Value::List(vec![atom("c"), Value::List(vec![atom("d")])])]), atom("NIL")]
    );
    assert_eq!(parse("a\tb"), vec![atom("a"), atom("b")]);
    assert_eq!(parse(""), vec![]);
    assert_eq!(parse("   "), vec![]);
}

#[test]
fn sections_are_part_of_the_word() {
    assert_eq!(
        parse("(BODY[HEADER.FIELDS (FROM TO)] x BODY[]<0> y)"),
        vec![Value::List(vec![atom("BODY[HEADER.FIELDS (FROM TO)]"), atom("x"), atom("BODY[]<0>"), atom("y")])]
    );
    assert_eq!(parse(r#"LIST () "/" [Gmail]/Trash"#), vec![atom("LIST"), Value::List(vec![]), string("/"), atom("[Gmail]/Trash")]);
    // A bracket that never closes is an ordinary character.
    assert_eq!(parse("(a[b c)"), vec![Value::List(vec![atom("a[b"), atom("c")])]);
    assert_eq!(parse("[[[[ x"), vec![atom("[[[["), atom("x")]);
}

#[test]
fn broken_text_is_read_as_far_as_it_goes() {
    assert_eq!(parse("(a (b"), vec![Value::List(vec![atom("a"), Value::List(vec![atom("b")])])]);
    assert_eq!(parse("a) b))"), vec![atom("a"), atom("b")]);
    assert_eq!(parse(r#""never closed"#), vec![string("never closed")]);
    assert_eq!(parse(r#""ends in a backslash\"#), vec![string("ends in a backslash")]);
    assert_eq!(parse("a\"b c"), vec![atom("a\"b"), atom("c")]);
    let deep = "(".repeat(MAX_DEPTH + 5);
    assert_eq!(values(vec![text(&deep)]), Err(ProtoError::TooDeep { limit: MAX_DEPTH }));
    assert!(values(vec![text(&"(".repeat(MAX_DEPTH))]).is_ok());
}

#[test]
fn literals_take_the_place_of_their_marker() {
    let pieces = vec![text("1 FETCH (BODY[] {5}"), Piece::Literal(b"a (\"b".to_vec()), text(" UID 3 X {0}"), Piece::Literal(Vec::new()), text(")")];
    assert_eq!(
        values(pieces),
        Ok(vec![atom("1"), atom("FETCH"), Value::List(vec![atom("BODY[]"), string("a (\"b"), atom("UID"), atom("3"), atom("X"), string("")])])
    );
    // Without a literal after it, a marker is only a word.
    assert_eq!(parse("a {5}"), vec![atom("a"), atom("{5}")]);
}

#[test]
fn markers() {
    assert_eq!(literal_marker(b"* 1 FETCH (BODY[] {123}"), Some((18, 123)));
    assert_eq!(literal_marker(b"{0}"), Some((0, 0)));
    assert_eq!(literal_marker(b"x {7+}"), Some((2, 7)));
    assert_eq!(literal_marker(b"\"quoted\" {7}"), Some((9, 7)));
    assert_eq!(literal_marker(b"\"a \\\" b\" {7}"), Some((9, 7)));
    assert_eq!(literal_marker(b"x {99999999999999999999999}"), Some((2, u64::MAX)));
    for not in ["", "}", "{}", "{+}", "{5} ", "{-5}", "{5 }", "\"in a string {5}", "\"a \\\" {5}", "{5}x", "{ 5}", "5}"] {
        assert_eq!(literal_marker(not.as_bytes()), None, "{not:?}");
    }
}

#[test]
fn status_lines() {
    let s = StatusLine::new(Status::Ok, " [UIDVALIDITY 3857529045] UIDs valid ");
    assert_eq!((s.code.as_str(), s.arg.as_str(), s.text.as_str()), ("UIDVALIDITY", "3857529045", "UIDs valid"));
    let s = StatusLine::new(Status::No, "[alert] disk full");
    assert_eq!((s.code.as_str(), s.arg.as_str(), s.text.as_str()), ("ALERT", "", "disk full"));
    let s = StatusLine::new(Status::Ok, "done [not a code]");
    assert_eq!((s.code.as_str(), s.text.as_str()), ("", "done [not a code]"));
    assert_eq!(StatusLine::new(Status::Bad, "[unclosed").code, "");
    assert_eq!(StatusLine::new(Status::Bad, "[unclosed").text, "[unclosed");
    assert_eq!(StatusLine::new(Status::Bad, "").text, "");
    assert_eq!(Status::parse(b"preauth"), Some(Status::Preauth));
    assert_eq!(Status::parse(b"bye"), Some(Status::Bye));
    assert_eq!(Status::parse(b"FETCH"), None);
}

#[test]
fn values_as_numbers_and_text() {
    assert_eq!(atom("42").number(), Some(42));
    assert_eq!(atom("4294967296").number(), None);
    assert_eq!(atom("-1").number(), None);
    assert_eq!(atom("+1").number(), None);
    assert_eq!(string("42").number(), None);
    assert_eq!(number(""), None);
    assert_eq!(string("Entw&APw-rfe").into_text().as_deref(), Some("Entw&APw-rfe"));
    assert_eq!(atom("INBOX").into_text().as_deref(), Some("INBOX"));
    assert_eq!(Value::Str(vec![0xFF, b'a']).into_text().as_deref(), Some("\u{FFFD}a"));
    assert_eq!(Value::List(vec![]).into_text(), None);
    assert_eq!(Value::List(vec![]).atom(), None);
}
