// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::EnhancedCode;

#[test]
fn a_code_is_read_off_the_front_of_the_text() {
    let (code, rest) = EnhancedCode::split("5.1.1 <bela@example.net>: no such user", 5);
    assert_eq!(code.map(|c| (c.class(), c.subject(), c.detail())), Some((5, 1, 1)));
    assert_eq!(rest, "<bela@example.net>: no such user");
    assert_eq!(code.map(|c| c.to_string()).as_deref(), Some("5.1.1"));
}

#[test]
fn subject_and_detail_have_one_to_three_digits() {
    let (code, rest) = EnhancedCode::split("4.123.999", 4);
    assert_eq!(code.map(|c| (c.class(), c.subject(), c.detail())), Some((4, 123, 999)));
    assert_eq!(rest, "");
    assert_eq!(EnhancedCode::split("2.0.0  queued", 2).1, "queued");
}

#[test]
fn text_that_does_not_start_with_a_code_is_left_alone() {
    for text in ["", "OK", "5.1 short", "5.1.1x", "5.1.1.1", "5..1", "5.1.", "5.1234.1 long", "5.1.1234", "3.1.1 class", "1.0.0", ".1.1"] {
        assert_eq!(EnhancedCode::split(text, text.as_bytes().first().map_or(0, |b| b.wrapping_sub(b'0'))), (None, text), "{text}");
    }
}

#[test]
fn the_class_must_agree_with_the_reply_code() {
    assert_eq!(EnhancedCode::split("5.1.1 refused", 2), (None, "5.1.1 refused"));
}
