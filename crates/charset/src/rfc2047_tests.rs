// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_WORD_LEN, WordError, decode_text, decode_word, encode_text};
use crate::{CharsetError, Decoded};

fn clean(text: &str) -> Decoded {
    Decoded { text: text.to_owned(), malformed: false }
}

fn broken(text: &str) -> Decoded {
    Decoded { text: text.to_owned(), malformed: true }
}

// ---- one word ----

#[test]
fn the_rfc_2047_examples_decode() {
    assert_eq!(decode_word("=?US-ASCII?Q?Keith_Moore?="), Ok("Keith Moore".to_owned()));
    assert_eq!(decode_word("=?ISO-8859-1?Q?Keld_J=F8rn_Simonsen?="), Ok("Keld Jørn Simonsen".to_owned()));
    assert_eq!(decode_word("=?ISO-8859-1?Q?Andr=E9?="), Ok("André".to_owned()));
    assert_eq!(decode_word("=?ISO-8859-1?B?SWYgeW91IGNhbiByZWFkIHRoaXMgeW8=?="), Ok("If you can read this yo".to_owned()));
    assert_eq!(decode_word("=?ISO-8859-2?B?dSB1bmRlcnN0YW5kIHRoZSBleGFtcGxlLg==?="), Ok("u understand the example.".to_owned()));
}

#[test]
fn encoding_letters_and_hex_digits_are_read_without_case() {
    assert_eq!(decode_word("=?utf-8?q?caf=c3=a9?="), Ok("café".to_owned()));
    assert_eq!(decode_word("=?UTF-8?b?Y2Fmw6k=?="), Ok("café".to_owned()));
}

#[test]
fn a_leading_u_feff_in_a_word_is_text_not_a_byte_order_mark() {
    assert_eq!(decode_word("=?UTF-8?B?77u/?="), Ok("\u{feff}".to_owned()));
    assert_eq!(decode_text("a =?UTF-8?Q?=EF=BB=BFb?="), clean("a \u{feff}b"));
    assert_eq!(decode_text(&encode_text("\u{feff}")), clean("\u{feff}"));
}

#[test]
fn an_empty_word_is_empty_text() {
    assert_eq!(decode_word("=?utf-8?q??="), Ok(String::new()));
    assert_eq!(decode_word("=?utf-8?b??="), Ok(String::new()));
}

#[test]
fn the_rfc_2231_language_after_the_charset_is_dropped() {
    assert_eq!(decode_word("=?US-ASCII*EN?Q?Keith_Moore?="), Ok("Keith Moore".to_owned()));
}

#[test]
fn what_is_not_one_word_is_refused() {
    for text in [
        "",
        "plain",
        "=?utf-8?q?a?= ",
        " =?utf-8?q?a?=",
        "=?utf-8?q?a",
        "=?utf-8?q?a b?=",
        "=?utf-8?q?a\tb?=",
        "=??q?a?=",
        "=?a=b?q?c?=",
        "=?utf-8??a?=",
        "=?utf-8?qq?a?=",
        "=?utf 8?q?a?=",
        "=?utf-8?q?a?=b?=",
    ] {
        assert_eq!(decode_word(text), Err(WordError::NotAWord), "{text:?}");
    }
}

#[test]
fn an_unknown_charset_is_reported() {
    assert_eq!(decode_word("=?x-unknown?q?a?="), Err(WordError::Charset(CharsetError::UnknownLabel("x-unknown".to_owned()))));
    assert_eq!(decode_word("=?iso-2022-kr?q?a?="), Err(WordError::Charset(CharsetError::Unsupported("iso-2022-kr".to_owned()))));
}

#[test]
fn an_unknown_encoding_is_reported() {
    assert_eq!(decode_word("=?utf-8?x?a?="), Err(WordError::UnknownEncoding('x')));
}

#[test]
fn broken_base64_is_reported() {
    assert_eq!(decode_word("=?utf-8?b?Y2Fmw6k?="), Ok("café".to_owned()));
    assert_eq!(decode_word("=?utf-8?b?Y2Fmw6!=?="), Err(WordError::BadBase64));
    assert_eq!(decode_word("=?utf-8?b?Y?="), Err(WordError::BadBase64));
}

#[test]
fn broken_q_is_reported() {
    for word in ["=?utf-8?q?a=?=", "=?utf-8?q?a=4?=", "=?utf-8?q?a=4g?=", "=?utf-8?q?a=\u{e9}9?=", "=?utf-8?q?\u{e9}?=", "=?utf-8?q?a\u{7}?="] {
        assert_eq!(decode_word(word), Err(WordError::BadQ), "{word:?}");
    }
}

#[test]
fn bytes_the_charset_cannot_hold_are_reported() {
    assert_eq!(decode_word("=?utf-8?q?=FF?="), Err(WordError::Malformed));
}

#[test]
fn errors_read_as_sentences() {
    let cases = [
        (WordError::NotAWord, "not an encoded word"),
        (WordError::Charset(CharsetError::UnknownLabel("x".to_owned())), "unknown character set \"x\""),
        (WordError::UnknownEncoding('x'), "unknown encoding 'x', expected B or Q"),
        (WordError::BadBase64, "broken base64 in an encoded word"),
        (WordError::BadQ, "broken Q encoding in an encoded word"),
        (WordError::Malformed, "an encoded word holds bytes its character set cannot"),
    ];
    for (error, text) in cases {
        assert_eq!(error.to_string(), text);
    }
    let error = WordError::Charset(CharsetError::UnknownLabel("x".to_owned()));
    assert!(std::error::Error::source(&error).is_some());
    assert!(std::error::Error::source(&WordError::BadQ).is_none());
}

// ---- header text ----

#[test]
fn text_without_words_is_kept_as_it_is() {
    for text in ["", "Hello, world", "  two  spaces ", "a =? b ?= c", "=?", "?=", "=?=", "=?a?", "price=?"] {
        assert_eq!(decode_text(text), clean(text), "{text:?}");
    }
}

#[test]
fn the_rfc_2047_section_8_examples_decode() {
    let cases = [
        ("(=?ISO-8859-1?Q?a?=)", "(a)"),
        ("(=?ISO-8859-1?Q?a?= b)", "(a b)"),
        ("(=?ISO-8859-1?Q?a?= =?ISO-8859-1?Q?b?=)", "(ab)"),
        ("(=?ISO-8859-1?Q?a?=  =?ISO-8859-1?Q?b?=)", "(ab)"),
        ("(=?ISO-8859-1?Q?a?=\r\n    =?ISO-8859-1?Q?b?=)", "(ab)"),
        ("(=?ISO-8859-1?Q?a_b?=)", "(a b)"),
        ("(=?ISO-8859-1?Q?a?= =?ISO-8859-2?Q?_b?=)", "(a b)"),
    ];
    for (text, decoded) in cases {
        assert_eq!(decode_text(text), clean(decoded), "{text:?}");
    }
}

#[test]
fn white_space_beside_plain_text_stays() {
    assert_eq!(decode_text("Re: =?utf-8?q?caf=C3=A9?= au lait"), clean("Re: café au lait"));
    assert_eq!(decode_text("\t=?utf-8?q?a?=\t"), clean("\ta\t"));
}

#[test]
fn a_character_split_across_two_words_of_one_charset_is_joined() {
    // "é" is C3 A9 in UTF-8; a sender split it across the words.
    assert_eq!(decode_text("=?utf-8?q?caf=C3?= =?UTF-8?B?qQ==?="), clean("café"));
    // Different charsets are decoded on their own.
    assert_eq!(decode_text("=?utf-8?q?caf=C3?= =?iso-8859-1?q?=A9?="), broken("caf\u{fffd}©"));
}

#[test]
fn a_word_without_white_space_around_it_is_decoded_too() {
    assert_eq!(decode_text("Re:=?utf-8?q?caf=C3=A9?=!"), clean("Re:café!"));
    assert_eq!(decode_text("=?utf-8?q?a?==?utf-8?q?b?="), clean("ab"));
}

#[test]
fn a_broken_word_stays_as_it_is_and_is_reported() {
    for word in ["=?x-unknown?q?a?=", "=?utf-8?x?a?=", "=?utf-8?b?!!!!?=", "=?utf-8?q?=ZZ?="] {
        let text = format!("a {word} =?utf-8?q?b?=");
        assert_eq!(decode_text(&text), broken(&text.replace("=?utf-8?q?b?=", "b")), "{text:?}");
    }
}

#[test]
fn white_space_between_a_broken_word_and_a_good_one_stays() {
    assert_eq!(decode_text("=?utf-8?q?a?= =?x-y?q?b?= =?utf-8?q?c?="), broken("a =?x-y?q?b?= c"));
}

#[test]
fn bytes_the_charset_cannot_hold_become_replacement_characters_and_are_reported() {
    assert_eq!(decode_text("x =?utf-8?q?=FF?="), broken("x \u{fffd}"));
}

#[test]
fn a_word_cut_short_is_plain_text() {
    assert_eq!(decode_text("=?utf-8?q?abc"), clean("=?utf-8?q?abc"));
    assert_eq!(decode_text("=?utf-8?q?a b?="), clean("=?utf-8?q?a b?="));
    assert_eq!(decode_text("=?=?utf-8?q?a?="), clean("=?a"));
}

#[test]
fn a_hostile_charset_label_is_never_longer_than_its_cap() {
    let label = "x".repeat(10_000);
    let text = format!("=?{label}?q?a?=");
    assert_eq!(decode_text(&text), clean(&text));
}

// ---- writing ----

#[test]
fn plain_ascii_is_written_as_it_is() {
    for text in ["", "Hello, world", "  spaced\tout  ", "a = b ? c", "100% = (x)"] {
        assert_eq!(encode_text(text), text, "{text:?}");
    }
}

#[test]
fn only_the_words_that_need_it_are_encoded() {
    assert_eq!(encode_text("Re: café au lait"), "Re: =?UTF-8?Q?caf=C3=A9?= au lait");
    assert_eq!(encode_text("Grüße aus Köln"), "=?UTF-8?Q?Gr=C3=BC=C3=9Fe?= aus =?UTF-8?Q?K=C3=B6ln?=");
    assert_eq!(encode_text("Grüße  Köln"), "=?UTF-8?Q?Gr=C3=BC=C3=9Fe__K=C3=B6ln?=");
    assert_eq!(encode_text("\tKöln "), "\t=?UTF-8?Q?K=C3=B6ln?= ");
}

#[test]
fn text_that_looks_like_an_encoded_word_is_encoded() {
    let text = "see =?utf-8?q?x?= and a?=b";
    let written = encode_text(text);
    assert_eq!(written, "see =?UTF-8?Q?=3D=3Futf-8=3Fq=3Fx=3F=3D?= and =?UTF-8?Q?a=3F=3Db?=");
    assert_eq!(decode_text(&written), clean(text));
}

#[test]
fn control_characters_are_encoded_and_cr_lf_and_nul_go() {
    assert_eq!(encode_text("a\u{7}b"), "=?UTF-8?Q?a=07b?=");
    assert_eq!(encode_text("Subject\r\nBcc: x@example.org\0"), "SubjectBcc: x@example.org");
}

#[test]
fn base64_is_used_when_it_is_shorter() {
    let text = "Привет мир";
    let written = encode_text(text);
    assert_eq!(written, "=?UTF-8?B?0J/RgNC40LLQtdGCINC80LjRgA==?=");
    assert_eq!(decode_text(&written), clean(text));
}

#[test]
fn long_text_is_split_into_words_of_at_most_75_characters_at_character_boundaries() {
    for text in ["é".repeat(200), "日本語のテキスト".repeat(20), "a\u{1f600}".repeat(50), format!("{}é", "x".repeat(300))] {
        let written = encode_text(&text);
        for word in written.split(' ') {
            assert!(word.len() <= MAX_WORD_LEN, "{word:?}");
            assert!(decode_word(word).is_ok(), "{word:?}");
        }
        assert_eq!(decode_text(&written), clean(&text));
    }
}
