// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Framer, MAX_LINE, MAX_LITERAL, MAX_LITERALS};
use crate::ProtoError;
use crate::imap::value::Piece;

fn text(s: &str) -> Piece {
    Piece::Text(s.as_bytes().to_vec())
}

fn literal(s: &str) -> Piece {
    Piece::Literal(s.as_bytes().to_vec())
}

/// Every response in `input`, pushed one byte at a time.
fn byte_by_byte(input: &[u8]) -> Vec<Vec<Piece>> {
    let mut framer = Framer::default();
    let mut out = Vec::new();
    for byte in input {
        framer.push(std::slice::from_ref(byte));
        while let Ok(Some(response)) = framer.next() {
            out.push(response);
        }
    }
    out
}

#[test]
fn lines_end_in_crlf_or_a_bare_lf() {
    let mut framer = Framer::default();
    framer.push(b"* OK ready\r\nA1 OK done\n* 3 EXI");
    assert_eq!(framer.next(), Ok(Some(vec![text("* OK ready")])));
    assert_eq!(framer.next(), Ok(Some(vec![text("A1 OK done")])));
    assert_eq!(framer.next(), Ok(None));
    assert!(framer.has_pending());
    framer.push(b"STS\r\n");
    assert_eq!(framer.next(), Ok(Some(vec![text("* 3 EXISTS")])));
    assert_eq!(framer.next(), Ok(None));
    assert!(!framer.has_pending());
}

#[test]
fn a_literal_and_the_line_after_it_belong_to_the_response() {
    let input = b"* 1 FETCH (BODY[] {7}\r\nab\r\ncd) {0}\r\n UID 9)\r\n* 2 EXISTS\r\n";
    let expected =
        vec![vec![text("* 1 FETCH (BODY[] {7}"), literal("ab\r\ncd)"), text(" {0}"), literal(""), text(" UID 9)")], vec![text("* 2 EXISTS")]];
    assert_eq!(byte_by_byte(input), expected);
    let mut framer = Framer::default();
    framer.push(input);
    assert_eq!(framer.next(), Ok(Some(expected[0].clone())));
}

#[test]
fn status_responses_and_continuations_carry_no_literal() {
    for line in ["* OK see {5}", "* BYE {5}", "A1 NO quota {5}", "+ {5}", "* no {5}"] {
        let mut framer = Framer::default();
        framer.push(format!("{line}\r\nnext\r\n").as_bytes());
        assert_eq!(framer.next(), Ok(Some(vec![text(line)])), "{line}");
    }
    // An untagged word that is no status word is data.
    let mut framer = Framer::default();
    framer.push(b"* LIST () \"/\" {1}\r\nx\r\n");
    assert_eq!(framer.next(), Ok(Some(vec![text("* LIST () \"/\" {1}"), literal("x"), text("")])));
}

#[test]
fn a_line_over_the_cap_is_refused_whole_or_in_pieces() {
    let mut framer = Framer::default();
    let mut line = vec![b'x'; MAX_LINE];
    line.extend_from_slice(b"\r\n");
    framer.push(&line);
    assert_eq!(framer.next().map(|r| r.map(|pieces| pieces.len())), Ok(Some(1)));

    let mut framer = Framer::default();
    framer.push(&vec![b'x'; MAX_LINE + 2]);
    assert_eq!(framer.next(), Err(ProtoError::LineTooLong { limit: MAX_LINE }));

    let mut framer = Framer::default();
    framer.push(&vec![b'x'; MAX_LINE + 1]);
    framer.push(b"\n");
    assert_eq!(framer.next(), Err(ProtoError::LineTooLong { limit: MAX_LINE }));

    // The cap is on all the text of one response, not on each line.
    let mut framer = Framer::default();
    let half = "x".repeat(MAX_LINE / 2);
    framer.push(format!("* 1 FETCH (A {{1}}\r\n.{half}{half} B {{1}}\r\n.{half})").as_bytes());
    assert_eq!(framer.next(), Err(ProtoError::LineTooLong { limit: MAX_LINE }));
}

#[test]
fn a_literal_over_the_cap_is_refused_before_it_arrives() {
    let mut framer = Framer::default();
    framer.push(format!("* 1 FETCH (BODY[] {{{}}}\r\n", MAX_LITERAL + 1).as_bytes());
    assert_eq!(framer.next(), Err(ProtoError::LiteralTooLarge { size: (MAX_LITERAL + 1) as u64, limit: MAX_LITERAL }));

    let mut framer = Framer::default();
    framer.push(b"* 1 FETCH (BODY[] {99999999999999999999999}\r\n");
    assert_eq!(framer.next(), Err(ProtoError::LiteralTooLarge { size: u64::MAX, limit: MAX_LITERAL }));

    // Two literals that each fit but not together.
    let mut framer = Framer::default();
    let half = MAX_LITERAL / 2 + 1;
    framer.push(format!("* 1 FETCH (A {{{half}}}\r\n").as_bytes());
    framer.push(&vec![b'a'; half]);
    framer.push(format!(" B {{{half}}}\r\n").as_bytes());
    assert_eq!(framer.next(), Err(ProtoError::LiteralTooLarge { size: half as u64, limit: MAX_LITERAL }));
}

#[test]
fn a_response_with_too_many_literals_is_refused() {
    let mut input = b"* 1 FETCH (".to_vec();
    for _ in 0..MAX_LITERALS {
        input.extend_from_slice(b"X {1}\r\nx ");
    }
    let mut ok = input.clone();
    ok.extend_from_slice(b")\r\n");
    let mut framer = Framer::default();
    framer.push(&ok);
    assert_eq!(framer.next().map(|r| r.map(|pieces| pieces.len())), Ok(Some(2 * MAX_LITERALS + 1)));

    input.extend_from_slice(b"X {1}\r\nx)\r\n");
    let mut framer = Framer::default();
    framer.push(&input);
    assert_eq!(framer.next(), Err(ProtoError::TooManyLiterals { limit: MAX_LITERALS }));
}

#[test]
fn a_literal_waits_for_all_its_bytes() {
    let mut framer = Framer::default();
    framer.push(b"* 1 FETCH (BODY[] {4}\r\nab");
    assert_eq!(framer.next(), Ok(None));
    framer.push(b"cd)\r\n");
    assert_eq!(framer.next(), Ok(Some(vec![text("* 1 FETCH (BODY[] {4}"), literal("abcd"), text(")")])));
}

#[test]
fn a_literal_is_moved_out_not_kept_twice() {
    let size = 1 << 20;
    let mut input = format!("* 1 FETCH (BODY[] {{{size}}}\r\n").into_bytes();
    input.extend(std::iter::repeat_n(b'x', size));
    input.extend_from_slice(b")\r\n* 2 EXISTS");
    let mut framer = Framer::default();
    framer.push(&input);
    let response = framer.next().unwrap().unwrap();
    assert!(matches!(response.get(1), Some(Piece::Literal(bytes)) if bytes.len() == size));
    // What is left is the line after the literal and the bytes still pending, not the literal.
    assert!(framer.buf.len() <= b")\r\n* 2 EXISTS".len(), "{}", framer.buf.len());
    assert!(framer.has_pending());
}
