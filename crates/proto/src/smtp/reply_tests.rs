// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_REPLY_LINE, MAX_REPLY_LINES, Reader, Reply};
use crate::ProtoError;

fn read_all(bytes: &[u8]) -> Result<Vec<Reply>, ProtoError> {
    let mut reader = Reader::default();
    reader.push(bytes);
    let mut replies = Vec::new();
    while let Some(reply) = reader.next()? {
        replies.push(reply);
    }
    Ok(replies)
}

fn reply(code: u16, lines: &[&str]) -> Reply {
    Reply { code, lines: lines.iter().map(|line| (*line).to_owned()).collect() }
}

#[test]
fn a_one_line_reply_is_its_code_and_text() {
    assert_eq!(read_all(b"250 OK\r\n"), Ok(vec![reply(250, &["OK"])]));
}

#[test]
fn a_reply_may_have_no_text() {
    assert_eq!(read_all(b"250\r\n354 \r\n"), Ok(vec![reply(250, &[""]), reply(354, &[""])]));
}

#[test]
fn a_multiline_reply_ends_at_the_line_with_a_space() {
    let bytes = b"250-mail.example.org greets you\r\n250-SIZE 1000\r\n250 8BITMIME\r\n";
    assert_eq!(read_all(bytes), Ok(vec![reply(250, &["mail.example.org greets you", "SIZE 1000", "8BITMIME"])]));
}

#[test]
fn a_reply_may_arrive_in_pieces_of_any_size() {
    let bytes = b"250-first\r\n250 second\r\n221 bye\r\n";
    let mut reader = Reader::default();
    let mut replies = Vec::new();
    for byte in bytes {
        reader.push(&[*byte]);
        while let Ok(Some(reply)) = reader.next() {
            replies.push(reply);
        }
    }
    assert_eq!(replies, [reply(250, &["first", "second"]), reply(221, &["bye"])]);
    assert!(!reader.has_pending());
}

#[test]
fn a_bare_line_feed_ends_a_line_too() {
    assert_eq!(read_all(b"220 hi\n"), Ok(vec![reply(220, &["hi"])]));
}

#[test]
fn bytes_after_a_reply_are_pending() {
    let mut reader = Reader::default();
    reader.push(b"220 go ahead\r\n250 early");
    assert_eq!(reader.next(), Ok(Some(reply(220, &["go ahead"]))));
    assert!(reader.has_pending());
    assert_eq!(reader.next(), Ok(None));
}

#[test]
fn text_that_is_not_utf8_is_kept_with_replacement_characters() {
    assert_eq!(read_all(b"250 caf\xe9\r\n"), Ok(vec![reply(250, &["caf\u{fffd}"])]));
}

#[test]
fn a_code_outside_the_grammar_is_malformed() {
    for line in [&b"25\r\n"[..], b"abc\r\n", b"150 one\r\n", b"600 six\r\n", b"260 six\r\n", b"25x\r\n", b"250+plus\r\n", b"\r\n"] {
        assert_eq!(read_all(line), Err(ProtoError::Malformed("reply code")), "{}", line.escape_ascii());
    }
}

#[test]
fn every_line_of_a_reply_has_the_same_code() {
    assert_eq!(read_all(b"250-one\r\n251 two\r\n"), Err(ProtoError::Malformed("reply code")));
}

#[test]
fn a_line_longer_than_the_cap_is_refused_before_it_ends() {
    let mut long = b"250 ".to_vec();
    long.resize(MAX_REPLY_LINE + 1, b'a');
    assert_eq!(read_all(&long), Err(ProtoError::LineTooLong { limit: MAX_REPLY_LINE }));
    long.extend_from_slice(b"\r\n");
    assert_eq!(read_all(&long), Err(ProtoError::LineTooLong { limit: MAX_REPLY_LINE }));
    let mut fits = b"250 ".to_vec();
    fits.resize(MAX_REPLY_LINE, b'a');
    fits.extend_from_slice(b"\r\n");
    assert_eq!(read_all(&fits).map(|replies| replies.len()), Ok(1));
}

#[test]
fn a_reply_with_more_lines_than_the_cap_is_refused() {
    let many = b"250-x\r\n".repeat(MAX_REPLY_LINES);
    assert_eq!(read_all(&many), Err(ProtoError::TooManyLines { limit: MAX_REPLY_LINES }));
    let mut fits = b"250-x\r\n".repeat(MAX_REPLY_LINES - 1);
    fits.extend_from_slice(b"250 x\r\n");
    assert_eq!(read_all(&fits).map(|replies| replies[0].lines.len()), Ok(MAX_REPLY_LINES));
}

#[test]
fn the_class_is_the_first_digit() {
    assert!(reply(250, &[]).is_positive());
    assert!(reply(354, &[]).is_intermediate());
    assert!(!reply(354, &[]).is_positive());
    assert!(reply(451, &[]).is_transient());
    assert!(reply(550, &[]).is_permanent());
    assert!(!reply(550, &[]).is_transient());
}
