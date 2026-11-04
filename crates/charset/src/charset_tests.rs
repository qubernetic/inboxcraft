// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Charset, CharsetError, Decoded, MAX_LABEL_IN_ERROR, decode};

#[test]
fn utf_8_decodes_as_is() {
    assert_eq!(decode("utf-8", "Grüße".as_bytes()), Ok(Decoded { text: "Grüße".to_owned(), malformed: false }));
}

#[test]
fn labels_are_matched_without_case_and_surrounding_white_space() {
    for label in ["UTF-8", " utf8 ", "\tUnicode-1-1-UTF-8", "Utf-8"] {
        assert_eq!(Charset::for_label(label), Ok(Charset::UTF_8), "{label:?}");
    }
}

#[test]
fn latin_1_and_ascii_read_as_windows_1252_like_every_browser() {
    for label in ["iso-8859-1", "ISO_8859-1", "latin1", "us-ascii", "ascii", "cp1252"] {
        let charset = Charset::for_label(label);
        assert_eq!(charset.map(Charset::name), Ok("windows-1252"), "{label:?}");
    }
    // 0x80 is the euro sign in windows-1252, a C1 control in ISO 8859-1.
    assert_eq!(decode("iso-8859-1", b"\x80 5, caf\xe9").map(|d| d.text), Ok("€ 5, café".to_owned()));
}

#[test]
fn the_other_mail_charsets_decode() {
    let cases: [(&str, &[u8], &str); 8] = [
        ("iso-8859-2", b"\xe1rv\xedzt\xfbr\xf5", "árvíztűrő"),
        ("windows-1251", b"\xcf\xf0\xe8\xe2\xe5\xf2", "Привет"),
        ("koi8-r", b"\xf0\xd2\xc9\xd7\xc5\xd4", "Привет"),
        ("shift_jis", b"\x82\xb1\x82\xf1\x82\xc9\x82\xbf\x82\xcd", "こんにちは"),
        ("iso-2022-jp", b"\x1b$B$3$s$K$A$O\x1b(B", "こんにちは"),
        ("euc-kr", b"\xbe\xc8\xb3\xe7", "안녕"),
        ("gb2312", b"\xc4\xe3\xba\xc3", "你好"),
        ("big5", b"\xa7\x41\xa6\x6e", "你好"),
    ];
    for (label, bytes, text) in cases {
        assert_eq!(decode(label, bytes), Ok(Decoded { text: text.to_owned(), malformed: false }), "{label}");
    }
}

#[test]
fn utf_16_decodes_and_its_own_byte_order_mark_goes() {
    assert_eq!(decode("utf-16le", b"\xff\xfeh\x00i\x00").map(|d| d.text), Ok("hi".to_owned()));
    assert_eq!(decode("utf-16be", b"\x00h\x00i").map(|d| d.text), Ok("hi".to_owned()));
}

#[test]
fn a_utf_8_byte_order_mark_goes() {
    assert_eq!(decode("utf-8", b"\xef\xbb\xbfhi").map(|d| d.text), Ok("hi".to_owned()));
}

#[test]
fn bytes_the_charset_cannot_hold_become_replacement_characters_and_are_reported() {
    assert_eq!(decode("utf-8", b"ok \xff\xfe end"), Ok(Decoded { text: "ok \u{fffd}\u{fffd} end".to_owned(), malformed: true }));
    assert_eq!(decode("shift_jis", b"\x82").map(|d| d.malformed), Ok(true));
}

#[test]
fn empty_input_is_empty_text() {
    assert_eq!(decode("koi8-u", b""), Ok(Decoded { text: String::new(), malformed: false }));
}

#[test]
fn unknown_labels_are_reported() {
    for label in ["", " ", "utf-7", "x-unknown", "unknown-8bit", "utf\u{2010}8"] {
        assert_eq!(Charset::for_label(label), Err(CharsetError::UnknownLabel(label.trim().to_owned())), "{label:?}");
    }
}

#[test]
fn labels_whatwg_maps_to_the_replacement_encoding_are_unsupported() {
    for label in ["iso-2022-kr", "hz-gb-2312", "ISO-2022-CN"] {
        assert_eq!(Charset::for_label(label), Err(CharsetError::Unsupported(label.to_owned())), "{label:?}");
    }
}

#[test]
fn a_long_or_unprintable_label_is_shortened_and_cleaned_in_the_error() {
    let long = "x".repeat(10_000);
    let Err(CharsetError::UnknownLabel(kept)) = Charset::for_label(&long) else { panic!("not an unknown label") };
    assert_eq!(kept.chars().count(), MAX_LABEL_IN_ERROR);

    let Err(CharsetError::UnknownLabel(kept)) = Charset::for_label("bad\r\nBcc: x@example.org\u{0}") else { panic!("not an unknown label") };
    assert_eq!(kept, "bad\u{fffd}\u{fffd}Bcc: x@example.org\u{fffd}");

    let Err(CharsetError::UnknownLabel(kept)) = Charset::for_label(&"é".repeat(100)) else { panic!("not an unknown label") };
    assert_eq!(kept, "é".repeat(MAX_LABEL_IN_ERROR));
}

#[test]
fn errors_read_as_sentences() {
    assert_eq!(CharsetError::UnknownLabel("x-foo".to_owned()).to_string(), "unknown character set \"x-foo\"");
    assert_eq!(CharsetError::Unsupported("hz-gb-2312".to_owned()).to_string(), "character set \"hz-gb-2312\" is not supported");
    let error: &dyn std::error::Error = &CharsetError::UnknownLabel(String::new());
    assert!(error.source().is_none());
}

#[test]
fn a_charset_has_its_whatwg_name_and_shows_as_it() {
    let charset = Charset::for_label("sjis").unwrap();
    assert_eq!(charset.name(), "Shift_JIS");
    assert_eq!(charset.to_string(), "Shift_JIS");
    assert_eq!(format!("{:?}", Charset::UTF_8), "Charset(\"UTF-8\")");
}

#[test]
fn a_charset_decodes_on_its_own() {
    assert_eq!(Charset::UTF_8.decode(b"a"), Decoded { text: "a".to_owned(), malformed: false });
}
