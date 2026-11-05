// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::borrow::Cow;

use super::{Body, MAX_DEPTH, MAX_MESSAGE_BYTES, MAX_PARTS, Part, parse};
use crate::{CharsetError, MimeError, TransferEncoding};

/// A message from lines, CRLF between them and after the last.
fn eml(lines: &[&str]) -> Vec<u8> {
    lines.iter().flat_map(|line| [line.as_bytes(), b"\r\n"].concat()).collect()
}

fn essences(part: &Part<'_>) -> Vec<String> {
    part.parts().iter().map(|p| p.content_type().essence()).collect()
}

fn children<'p, 'a>(part: &'p Part<'a>) -> &'p [Part<'a>] {
    match part.body() {
        Body::Multipart(multipart) => multipart.parts(),
        _ => panic!("not a multipart"),
    }
}

// ---- one part ----

#[test]
fn a_plain_message_is_one_text_part() {
    let bytes = eml(&["From: Anna <anna@example.org>", "Subject: Hello", "", "Hi there,", "Anna"]);
    let message = parse(&bytes).unwrap();
    assert_eq!(message.headers().text("subject").as_deref(), Some("Hello"));
    assert_eq!(message.content_type().essence(), "text/plain");
    assert_eq!(message.transfer_encoding(), &TransferEncoding::SevenBit);
    assert!(matches!(message.body(), Body::Leaf));
    assert_eq!(message.raw_body(), b"Hi there,\r\nAnna\r\n");
    assert_eq!(message.text().unwrap(), "Hi there,\r\nAnna\r\n");
    assert_eq!(message.text_body().map(|p| p.raw_body()), Some(&b"Hi there,\r\nAnna\r\n"[..]));
    assert!(message.html_body().is_none());
    assert!(message.attachments().is_empty());
    assert_eq!(message.disposition(), None);
    assert_eq!(message.file_name(), None);
    assert_eq!(message.content_id(), None);
}

#[test]
fn an_empty_message_is_an_empty_text_part() {
    let message = parse(b"").unwrap();
    assert!(message.headers().fields().is_empty());
    assert_eq!(message.text().unwrap(), "");
}

#[test]
fn a_message_longer_than_the_cap_is_an_error() {
    let bytes = vec![b'a'; MAX_MESSAGE_BYTES + 1];
    assert_eq!(parse(&bytes).unwrap_err(), MimeError::TooLarge { limit: MAX_MESSAGE_BYTES });
}

#[test]
fn a_broken_header_block_is_an_error() {
    let bytes = "X: y\r\n".repeat(crate::MAX_FIELDS + 1);
    assert_eq!(parse(bytes.as_bytes()).unwrap_err(), MimeError::TooManyFields { limit: crate::MAX_FIELDS });
}

#[test]
fn text_is_decoded_from_its_transfer_encoding_and_charset() {
    let qp = eml(&["Content-Type: text/plain; charset=iso-8859-2", "Content-Transfer-Encoding: quoted-printable", "", "=E1rv=EDzt=FBr=F5"]);
    assert_eq!(parse(&qp).unwrap().text().unwrap(), "árvíztűrő\r\n");
    let b64 = eml(&["Content-Type: text/plain; charset=utf-8", "Content-Transfer-Encoding: base64", "", "w6FydsOtenTFsXLFkQ=="]);
    let message = parse(&b64).unwrap();
    assert_eq!(message.decoded().unwrap(), Cow::<[u8]>::Owned("árvíztűrő".as_bytes().to_vec()));
    assert_eq!(message.text().unwrap(), "árvíztűrő");
}

#[test]
fn text_without_a_charset_is_utf8_or_windows_1252() {
    let utf8 = [&b"Content-Type: text/plain\r\n\r\n\xef\xbb\xbf"[..], "café".as_bytes()].concat();
    assert_eq!(parse(&utf8).unwrap().text().unwrap(), "café");
    assert_eq!(parse("\r\ncafé".as_bytes()).unwrap().text().unwrap(), "café");
    assert_eq!(parse(b"Content-Type: text/plain; charset=US-ASCII\r\n\r\ncaf\xe9").unwrap().text().unwrap(), "café");
    assert_eq!(parse(b"Content-Type: text/plain; charset=\" \"\r\n\r\ncaf\xe9").unwrap().text().unwrap(), "café");
}

#[test]
fn broken_encodings_are_errors() {
    let bad_base64 = eml(&["Content-Transfer-Encoding: base64", "", "w6Fyd*"]);
    assert_eq!(parse(&bad_base64).unwrap().text(), Err(MimeError::BadBase64));
    let unknown = eml(&["Content-Transfer-Encoding: x-uuencode", "", "begin 644 a"]);
    assert_eq!(parse(&unknown).unwrap().decoded(), Err(MimeError::UnknownEncoding("x-uuencode".to_owned())));
    let charset = eml(&["Content-Type: text/plain; charset=x-klingon", "", "x"]);
    assert_eq!(parse(&charset).unwrap().text(), Err(MimeError::Charset(CharsetError::UnknownLabel("x-klingon".to_owned()))));
}

#[test]
fn a_content_type_that_cannot_be_read_is_plain_text() {
    let bytes = eml(&["Content-Type: garbage", "", "x"]);
    let message = parse(&bytes).unwrap();
    assert_eq!(message.content_type().essence(), "text/plain");
}

// ---- multipart ----

#[test]
fn an_alternative_with_plain_text_and_html() {
    let bytes = eml(&[
        "Content-Type: multipart/alternative; boundary=\"b1\"",
        "",
        "preamble",
        "--b1",
        "Content-Type: text/plain; charset=utf-8",
        "",
        "plain",
        "--b1",
        "Content-Type: text/html; charset=utf-8",
        "",
        "<p>html</p>",
        "--b1--",
        "epilogue",
    ]);
    let message = parse(&bytes).unwrap();
    assert_eq!(essences(&message), ["multipart/alternative", "text/plain", "text/html"]);
    let Body::Multipart(multipart) = message.body() else { panic!("not a multipart") };
    assert!(multipart.closed());
    assert_eq!(multipart.parts().len(), 2);
    assert_eq!(message.text_body().unwrap().text().unwrap(), "plain");
    assert_eq!(message.html_body().unwrap().text().unwrap(), "<p>html</p>");
    assert!(message.attachments().is_empty());
}

#[test]
fn a_part_without_headers_is_plain_text() {
    let bytes = eml(&["Content-Type: multipart/mixed; boundary=b", "", "--b", "", "no headers", "--b--"]);
    let message = parse(&bytes).unwrap();
    let parts = children(&message);
    assert!(parts[0].headers().fields().is_empty());
    assert_eq!(parts[0].text().unwrap(), "no headers");
}

#[test]
fn bare_lf_line_ends_work_in_multiparts() {
    let message = parse(b"Content-Type: multipart/mixed; boundary=b\n\n--b\nContent-Type: text/plain\n\nlf only\n--b--\n").unwrap();
    assert_eq!(children(&message)[0].text().unwrap(), "lf only");
}

#[test]
fn a_delimiter_may_carry_trailing_white_space_but_nothing_else() {
    let bytes = eml(&["Content-Type: multipart/mixed; boundary=b", "", "--b \t", "", "one", "--bb", "still one", "--b-- ", "--b", "after"]);
    let message = parse(&bytes).unwrap();
    let parts = children(&message);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].text().unwrap(), "one\r\n--bb\r\nstill one");
}

#[test]
fn a_multipart_cut_short_keeps_what_arrived() {
    let bytes = eml(&["Content-Type: multipart/mixed; boundary=b", "", "--b", "", "first", "--b", "", "cut"]);
    let message = parse(&bytes).unwrap();
    let Body::Multipart(multipart) = message.body() else { panic!("not a multipart") };
    assert!(!multipart.closed());
    assert_eq!(multipart.parts().len(), 2);
    assert_eq!(multipart.parts()[1].text().unwrap(), "cut\r\n");
}

#[test]
fn bad_boundaries_are_errors() {
    let missing = eml(&["Content-Type: multipart/mixed", "", "--b", "", "x", "--b--"]);
    assert_eq!(parse(&missing).unwrap_err(), MimeError::MissingBoundary);
    for boundary in ["\"\"", &format!("\"{}\"", "a".repeat(71)), "\"ends with space \"", "\"caf\u{e9}\"", "\"a\u{1}b\""] {
        let bytes = eml(&[&format!("Content-Type: multipart/mixed; boundary={boundary}"), "", "--x"]);
        assert_eq!(parse(&bytes).unwrap_err(), MimeError::BadBoundary, "{boundary}");
    }
    let not_found = eml(&["Content-Type: multipart/mixed; boundary=b", "", "--other", "", "x", "--other--"]);
    assert_eq!(parse(&not_found).unwrap_err(), MimeError::BoundaryNotFound);
    let a_70 = "a".repeat(70);
    let longest = eml(&[&format!("Content-Type: multipart/mixed; boundary=\"{a_70}\""), "", &format!("--{a_70}"), "", "x"]);
    assert!(parse(&longest).is_ok());
}

#[test]
fn a_boundary_inside_a_nested_multipart_does_not_end_the_outer_one() {
    let bytes = eml(&[
        "Content-Type: multipart/mixed; boundary=outer",
        "",
        "--outer",
        "Content-Type: multipart/alternative; boundary=inner",
        "",
        "--inner",
        "",
        "plain",
        "--inner",
        "Content-Type: text/html",
        "",
        "<b>html</b>",
        "--inner--",
        "--outer",
        "Content-Type: application/pdf; name=report.pdf",
        "Content-Transfer-Encoding: base64",
        "",
        "JVBERg==",
        "--outer--",
    ]);
    let message = parse(&bytes).unwrap();
    assert_eq!(essences(&message), ["multipart/mixed", "multipart/alternative", "text/plain", "text/html", "application/pdf"]);
    assert_eq!(message.text_body().unwrap().text().unwrap(), "plain");
    assert_eq!(message.html_body().unwrap().text().unwrap(), "<b>html</b>");
    let attachments = message.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].file_name().as_deref(), Some("report.pdf"));
    assert_eq!(attachments[0].decoded().unwrap(), Cow::<[u8]>::Owned(b"%PDF".to_vec()));
}

#[test]
fn nesting_deeper_than_the_cap_is_an_error() {
    let nested = |levels: usize| {
        let mut bytes = Vec::new();
        for level in 0..levels {
            bytes.extend(format!("Content-Type: multipart/mixed; boundary=b{level}\r\n\r\n--b{level}\r\n").bytes());
        }
        bytes.extend(b"\r\ninnermost\r\n");
        bytes
    };
    let bytes = nested(MAX_DEPTH);
    let message = parse(&bytes).unwrap();
    assert_eq!(message.parts().len(), MAX_DEPTH + 1);
    assert_eq!(message.text_body().unwrap().text().unwrap(), "innermost\r\n");
    assert_eq!(parse(&nested(MAX_DEPTH + 1)).unwrap_err(), MimeError::TooDeep { limit: MAX_DEPTH });
    // Endless nesting is no stack overflow.
    assert_eq!(parse(&nested(10_000)).unwrap_err(), MimeError::TooDeep { limit: MAX_DEPTH });
}

#[test]
fn more_parts_than_the_cap_is_an_error() {
    let many = |parts: usize| {
        let mut bytes = b"Content-Type: multipart/mixed; boundary=b\r\n\r\n".to_vec();
        bytes.extend(b"--b\r\n\r\nx\r\n".repeat(parts));
        bytes
    };
    assert_eq!(parse(&many(MAX_PARTS - 1)).unwrap().parts().len(), MAX_PARTS);
    assert_eq!(parse(&many(MAX_PARTS)).unwrap_err(), MimeError::TooManyParts { limit: MAX_PARTS });
    assert_eq!(parse(&many(1_000_000)).unwrap_err(), MimeError::TooManyParts { limit: MAX_PARTS });
}

#[test]
fn parts_in_nested_multiparts_count_toward_the_cap() {
    let mut bytes = b"Content-Type: multipart/mixed; boundary=outer\r\n\r\n--outer\r\nContent-Type: multipart/mixed; boundary=inner\r\n\r\n".to_vec();
    bytes.extend(b"--inner\r\n\r\nx\r\n".repeat(MAX_PARTS - 10));
    bytes.extend(b"--inner--\r\n");
    bytes.extend(b"--outer\r\n\r\ny\r\n".repeat(10));
    assert_eq!(parse(&bytes).unwrap_err(), MimeError::TooManyParts { limit: MAX_PARTS });
}

// ---- attached messages ----

#[test]
fn an_attached_message_is_parsed_and_is_an_attachment() {
    let bytes = eml(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "",
        "see below",
        "--b",
        "Content-Type: message/rfc822",
        "",
        "Subject: inner",
        "Content-Type: text/plain",
        "",
        "inner text",
        "--b--",
    ]);
    let message = parse(&bytes).unwrap();
    assert_eq!(essences(&message), ["multipart/mixed", "text/plain", "message/rfc822", "text/plain"]);
    assert_eq!(message.text_body().unwrap().text().unwrap(), "see below");
    let attachments = message.attachments();
    assert_eq!(attachments.len(), 1);
    let Body::Message(inner) = attachments[0].body() else { panic!("not a message") };
    assert_eq!(inner.headers().text("subject").as_deref(), Some("inner"));
    assert_eq!(inner.text().unwrap(), "inner text");
}

#[test]
fn an_attached_message_in_base64_stays_a_leaf() {
    let bytes = eml(&["Content-Type: message/rfc822", "Content-Transfer-Encoding: base64", "", "U3ViamVjdDogeA0KDQp5"]);
    let message = parse(&bytes).unwrap();
    assert!(matches!(message.body(), Body::Leaf));
    assert_eq!(message.decoded().unwrap(), Cow::<[u8]>::Owned(b"Subject: x\r\n\r\ny".to_vec()));
}

#[test]
fn attached_messages_count_toward_the_depth_cap() {
    let bytes = "Content-Type: message/rfc822\r\n\r\n".repeat(MAX_DEPTH + 1);
    assert_eq!(parse(bytes.as_bytes()).unwrap_err(), MimeError::TooDeep { limit: MAX_DEPTH });
    let bytes = "Content-Type: message/global\r\n\r\n".repeat(MAX_DEPTH);
    assert_eq!(parse(bytes.as_bytes()).unwrap().parts().len(), MAX_DEPTH + 1);
}

#[test]
fn a_digest_holds_messages_by_default() {
    let bytes = eml(&["Content-Type: multipart/digest; boundary=d", "", "--d", "", "Subject: one", "", "first", "--d--"]);
    let message = parse(&bytes).unwrap();
    assert_eq!(essences(&message), ["multipart/digest", "message/rfc822", "text/plain"]);
}

// ---- attachments ----

#[test]
fn what_is_an_attachment_and_what_is_not() {
    let bytes = eml(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: text/plain",
        "",
        "body",
        "--b",
        "Content-Type: text/plain",
        "Content-Disposition: attachment",
        "",
        "attached text",
        "--b",
        "Content-Type: text/plain; name=\"notes.txt\"",
        "",
        "named text",
        "--b",
        "Content-Type: image/png",
        "Content-Disposition: inline",
        "",
        "inline picture",
        "--b",
        "Content-Type: image/png",
        "Content-ID: <logo@example.org>",
        "",
        "related picture",
        "--b",
        "Content-Type: application/zip",
        "",
        "unnamed file",
        "--b",
        "Content-Type: image/png",
        "Content-Disposition: inline; filename=\"=?UTF-8?Q?k=C3=A9p?=.png\"",
        "",
        "named inline picture",
        "--b--",
    ]);
    let message = parse(&bytes).unwrap();
    let attachments: Vec<String> = message.attachments().iter().map(|p| p.text().unwrap()).collect();
    assert_eq!(attachments, ["attached text", "named text", "unnamed file", "named inline picture"]);
    let parts = children(&message);
    assert_eq!(parts[2].file_name().as_deref(), Some("notes.txt"));
    assert_eq!(parts[4].content_id().as_deref(), Some("logo@example.org"));
    assert!(parts[3].disposition().unwrap().is_inline());
    assert_eq!(parts[6].file_name().as_deref(), Some("kép.png"));
    assert!(!message.is_attachment());
    assert!(parts[6].is_attachment());
}

#[test]
fn a_file_name_loses_its_invisible_direction_marks() {
    let bytes = "Content-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"invoice\u{202e}fdp.exe\"\r\n\r\nx";
    assert_eq!(parse(bytes.as_bytes()).unwrap().file_name().as_deref(), Some("invoicefdp.exe"));
    let bytes = b"Content-Type: application/octet-stream; name=\"=?UTF-8?Q?a=E2=80=8Fb=E2=81=A7.txt?=\"\r\n\r\nx";
    assert_eq!(parse(bytes).unwrap().file_name().as_deref(), Some("ab.txt"));
}

#[test]
fn an_inline_text_part_with_a_file_name_is_the_body() {
    let message = parse(b"Content-Type: text/plain\r\nContent-Disposition: inline; filename=body.txt\r\n\r\nhello").unwrap();
    assert!(!message.is_attachment());
    assert!(message.attachments().is_empty());
    assert_eq!(message.text_body().map(|part| part.text().unwrap()).as_deref(), Some("hello"));
    let html = parse(b"Content-Type: text/html; name=page.html\r\nContent-Disposition: inline\r\n\r\n<p>hi</p>").unwrap();
    assert!(html.html_body().is_some());
    let named = parse(b"Content-Type: text/plain; name=notes.txt\r\n\r\nnotes").unwrap();
    assert!(named.is_attachment());
}

#[test]
fn a_content_id_is_cleaned() {
    let message = parse(b"Content-ID:  < a\x01b@example.org > \r\n\r\n").unwrap();
    assert_eq!(message.content_id().as_deref(), Some("a b@example.org"));
    assert_eq!(parse(b"Content-ID: <>\r\n\r\n").unwrap().content_id(), None);
}

#[test]
fn the_text_body_is_not_taken_from_an_attachment_or_an_attached_message() {
    let bytes = eml(&[
        "Content-Type: multipart/mixed; boundary=b",
        "",
        "--b",
        "Content-Type: message/rfc822",
        "",
        "Content-Type: text/html",
        "",
        "<p>inner</p>",
        "--b",
        "Content-Type: text/plain",
        "Content-Disposition: attachment; filename=a.txt",
        "",
        "attached",
        "--b--",
    ]);
    let message = parse(&bytes).unwrap();
    assert!(message.text_body().is_none());
    assert!(message.html_body().is_none());
}
