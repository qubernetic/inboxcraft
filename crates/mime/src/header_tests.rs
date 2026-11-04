// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Headers, MAX_FIELD_BYTES, MAX_FIELDS, line_at, split};
use crate::MimeError;

fn names_and_values(headers: &Headers) -> Vec<(&str, &str)> {
    headers.fields().iter().map(|field| (field.name(), field.value())).collect()
}

#[test]
fn a_header_block_ends_at_the_empty_line() {
    let (headers, body) = split(b"Subject: Hi\r\nTo: anna@example.org\r\n\r\nBody\r\nmore", true).unwrap();
    assert_eq!(names_and_values(&headers), [("Subject", "Hi"), ("To", "anna@example.org")]);
    assert_eq!(body, b"Body\r\nmore");
}

#[test]
fn names_match_without_case_and_the_first_field_wins() {
    let headers = Headers::parse(b"Received: one\nRECEIVED: two\nX-Empty:\n").unwrap();
    assert_eq!(headers.get("received"), Some("one"));
    assert_eq!(headers.get_all("Received").collect::<Vec<_>>(), ["one", "two"]);
    assert_eq!(headers.get("x-empty"), Some(""));
    assert_eq!(headers.get("cc"), None);
    assert_eq!(headers.text("cc"), None);
}

#[test]
fn bare_lf_line_ends_and_a_message_without_a_body() {
    let (headers, body) = split(b"Subject: Hi\nFrom: anna@example.org\n", true).unwrap();
    assert_eq!(headers.get("from"), Some("anna@example.org"));
    assert_eq!(body, b"");
    let (headers, body) = split(b"", true).unwrap();
    assert!(headers.fields().is_empty());
    assert_eq!(body, b"");
}

#[test]
fn folded_lines_are_unfolded_and_keep_their_white_space() {
    let headers = Headers::parse(b"Subject: a long\r\n  subject\r\n\tline\r\nX: y\r\n\r\n").unwrap();
    assert_eq!(headers.get("subject"), Some("a long  subject\tline"));
    assert_eq!(headers.get("x"), Some("y"));
}

#[test]
fn a_bare_cr_in_a_field_is_a_space() {
    // Found by the `mime_headers` fuzz target: a CR that ends no line stayed in the value.
    let headers = Headers::parse(b"X: -\r\"\r\nY: a\r\r\n b\r\n").unwrap();
    assert_eq!(headers.get("x"), Some("- \""));
    assert_eq!(headers.get("y"), Some("a  b"));
}

#[test]
fn white_space_before_the_colon_and_around_the_value_goes() {
    let headers = Headers::parse(b"Subject  :   spaced out   \r\n").unwrap();
    assert_eq!(names_and_values(&headers), [("Subject", "spaced out")]);
}

#[test]
fn a_line_that_is_no_field_starts_the_body() {
    let (headers, body) = split(b"Subject: Hi\r\nthis is the body\r\n", true).unwrap();
    assert_eq!(headers.get("subject"), Some("Hi"));
    assert_eq!(body, b"this is the body\r\n");
    let (headers, body) = split(b"Bad Name: x\r\n", true).unwrap();
    assert!(headers.fields().is_empty());
    assert_eq!(body, b"Bad Name: x\r\n");
    let (headers, body) = split(b": no name\r\n", false).unwrap();
    assert!(headers.fields().is_empty());
    assert_eq!(body, b": no name\r\n");
}

#[test]
fn a_continuation_of_nothing_starts_the_body() {
    let (headers, body) = split(b" indented\r\nSubject: x\r\n", false).unwrap();
    assert!(headers.fields().is_empty());
    assert_eq!(body, b" indented\r\nSubject: x\r\n");
}

#[test]
fn an_mbox_from_line_is_skipped_at_the_top_of_a_message_only() {
    let (headers, _) = split(b"From anna@example.org Mon Jan  1 00:00:00 2026\nSubject: Hi\n\n", true).unwrap();
    assert_eq!(headers.get("subject"), Some("Hi"));
    let (headers, _) = split(b">From anna@example.org\nSubject: Hi\n\n", true).unwrap();
    assert_eq!(headers.get("subject"), Some("Hi"));
    let (headers, body) = split(b"From anna@example.org\nSubject: Hi\n\n", false).unwrap();
    assert!(headers.fields().is_empty());
    assert_eq!(body, b"From anna@example.org\nSubject: Hi\n\n");
    let (headers, _) = split(b"Subject: Hi\nFrom anna\n\n", true).unwrap();
    assert_eq!(headers.fields().len(), 1);
}

#[test]
fn raw_8bit_values_are_read_as_utf8_or_windows_1252() {
    let headers = Headers::parse("Subject: árvíztűrő\r\n".as_bytes()).unwrap();
    assert_eq!(headers.get("subject"), Some("árvíztűrő"));
    let headers = Headers::parse(b"Subject: caf\xe9\r\n").unwrap();
    assert_eq!(headers.get("subject"), Some("café"));
}

#[test]
fn text_decodes_encoded_words_and_drops_control_characters() {
    let headers = Headers::parse(b"Subject: Re: =?ISO-8859-1?Q?Andr=E9?=\r\nX-Evil: a=?UTF-8?Q?=0D=0ABcc:_x?= b\x00c\x7fd\tE\r\n").unwrap();
    assert_eq!(headers.text("subject").as_deref(), Some("Re: André"));
    assert_eq!(headers.text("x-evil").as_deref(), Some("a  Bcc: x b c d E"));
}

#[test]
fn text_drops_invisible_direction_marks() {
    let headers = Headers::parse("Subject: =?UTF-8?Q?pay_=E2=80=AEfdp.exe?= \u{200b}now\u{feff}\u{2066}!\u{061c}\r\n".as_bytes()).unwrap();
    assert_eq!(headers.text("subject").as_deref(), Some("pay fdp.exe now!"));
}

#[test]
fn too_many_fields_are_an_error() {
    let block = "X: y\r\n".repeat(MAX_FIELDS);
    assert_eq!(Headers::parse(block.as_bytes()).unwrap().fields().len(), MAX_FIELDS);
    let block = "X: y\r\n".repeat(MAX_FIELDS + 1);
    assert_eq!(Headers::parse(block.as_bytes()), Err(MimeError::TooManyFields { limit: MAX_FIELDS }));
}

#[test]
fn a_field_longer_than_the_cap_is_an_error() {
    // The name, the space after the colon and the value: exactly the cap, then one byte over it.
    let value = "a".repeat(MAX_FIELD_BYTES - 2);
    assert!(Headers::parse(format!("X: {value}\r\n").as_bytes()).is_ok());
    let long = format!("X: {value}b\r\n");
    assert_eq!(Headers::parse(long.as_bytes()), Err(MimeError::FieldTooLong { limit: MAX_FIELD_BYTES }));
    let folded = format!("X: {}\r\n {}\r\n", "a".repeat(MAX_FIELD_BYTES / 2), "b".repeat(MAX_FIELD_BYTES / 2));
    assert_eq!(Headers::parse(folded.as_bytes()), Err(MimeError::FieldTooLong { limit: MAX_FIELD_BYTES }));
}

#[test]
fn lines_end_at_lf_with_or_without_cr() {
    assert_eq!(line_at(b"ab\r\ncd", 0), (&b"ab"[..], 4));
    assert_eq!(line_at(b"ab\r\ncd", 4), (&b"cd"[..], 6));
    assert_eq!(line_at(b"ab\ncd", 0), (&b"ab"[..], 3));
    assert_eq!(line_at(b"ab", 9), (&b""[..], 2));
}
