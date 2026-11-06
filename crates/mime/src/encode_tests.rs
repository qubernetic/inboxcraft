// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{b_words, base64_encode, crlf, fold, percent_pieces, qp_encode, strip_breaks};
use crate::MimeError;
use crate::transfer::{base64_decode, qp_decode};

#[test]
fn base64_pads_and_breaks_lines_at_76() {
    assert_eq!(base64_encode(b""), b"");
    assert_eq!(base64_encode(b"f"), b"Zg==");
    assert_eq!(base64_encode(b"fo"), b"Zm8=");
    assert_eq!(base64_encode(b"foo"), b"Zm9v");
    let long = base64_encode(&[0xff; 60]);
    let lines: Vec<&[u8]> = long.split(|b| *b == b'\n').collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].len(), 77);
    assert!(lines[0].ends_with(b"\r"));
    assert_eq!(base64_decode(&long).unwrap(), vec![0xff; 60]);
}

#[test]
fn line_ends_become_crlf() {
    assert_eq!(crlf("a\nb\rc\r\nd\n"), "a\r\nb\r\nc\r\nd\r\n");
    assert_eq!(crlf(""), "");
}

#[test]
fn quoted_printable_escapes_what_is_not_plain_and_keeps_lines_short() {
    assert_eq!(qp_encode(b"a=b\xc3\xa9"), b"a=3Db=C3=A9");
    assert_eq!(qp_encode(b"end \r\ntab\t"), b"end=20\r\ntab=09");
    assert_eq!(qp_encode(b"nul\0"), b"nul=00");
    let long = qp_encode(&[b'x'; 200]);
    assert!(long.split(|b| *b == b'\n').all(|line| line.len() <= 77));
    assert_eq!(qp_decode(&long), vec![b'x'; 200]);
    let escapes = qp_encode(&[0xe9; 100]);
    assert!(escapes.split(|b| *b == b'\n').all(|line| line.len() <= 77));
    assert_eq!(qp_decode(&escapes), vec![0xe9; 100]);
}

#[test]
fn quoted_printable_keeps_a_space_before_a_soft_break() {
    let text = [b"a".repeat(74), b" b".to_vec()].concat();
    let encoded = qp_encode(&text);
    assert_eq!(qp_decode(&encoded), text);
}

#[test]
fn breaks_and_nul_are_stripped_from_text() {
    assert_eq!(strip_breaks(" a\r\nb\0c\nd\r "), "abcd");
}

#[test]
fn a_field_folds_at_spaces_before_78_characters() {
    let value = ["word"; 30].join(" ");
    let field = fold("Subject", &value).unwrap();
    let text = String::from_utf8(field).unwrap();
    assert!(text.ends_with("\r\n"));
    assert!(text.lines().all(|line| line.len() <= 78), "{text}");
    assert_eq!(text.replace("\r\n", ""), format!("Subject: {value}"));
}

#[test]
fn folding_never_leaves_a_line_of_white_space() {
    let value = format!("{}   {}", "a".repeat(70), "b".repeat(10));
    let text = String::from_utf8(fold("X", &value).unwrap()).unwrap();
    assert!(text.lines().all(|line| !line.trim().is_empty()), "{text:?}");
    assert_eq!(text.replace("\r\n", ""), format!("X: {value}"));
}

#[test]
fn an_empty_value_is_a_bare_field() {
    assert_eq!(fold("Subject", "").unwrap(), b"Subject:\r\n");
}

#[test]
fn a_field_the_reader_could_not_read_back_is_refused() {
    let value = ["word"; 60_000].join(" ");
    assert_eq!(fold("X", &value), Err(MimeError::FieldTooLong { limit: crate::MAX_FIELD_BYTES }));
}

#[test]
fn b_words_split_between_characters_and_stay_within_75() {
    let words = b_words(&"é".repeat(100));
    assert!(words.len() > 1);
    assert!(words.iter().all(|word| word.len() <= 75 && word.starts_with("=?UTF-8?B?") && word.ends_with("?=")));
    assert_eq!(inboxcraft_charset::decode_text(&words.join(" ")).text, "é".repeat(100));
    assert!(b_words("").is_empty());
}

#[test]
fn percent_pieces_keep_escapes_whole() {
    let pieces = percent_pieces("ááááááááááááááááááááááááááá.txt");
    assert!(pieces.len() > 1);
    assert!(pieces.iter().all(|piece| piece.len() <= 60));
    assert!(pieces.iter().all(|piece| !piece.ends_with('%') && piece.rfind('%').is_none_or(|i| i + 3 <= piece.len())));
    assert_eq!(percent_pieces("a b'*%.txt"), ["a%20b%27%2A%25.txt"]);
}
