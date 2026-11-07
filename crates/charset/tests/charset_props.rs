// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property tests of charset decoding and encoded words: no label, bytes or header text makes them panic, UTF-8
//! decodes as itself, and text written as encoded words reads back as the same text.

use inboxcraft_charset::{Charset, Decoded, MAX_WORD_LEN, decode, decode_text, decode_word, encode_text};
use proptest::prelude::*;

const LABELS: [&str; 12] =
    ["utf-8", "iso-8859-1", "iso-8859-2", "windows-1251", "koi8-r", "shift_jis", "euc-jp", "iso-2022-jp", "euc-kr", "gbk", "big5", "utf-16le"];

const SINGLE_BYTE: [&str; 5] = ["windows-1252", "iso-8859-2", "iso-8859-15", "koi8-u", "windows-1251"];

/// Header-like text: ASCII, letters with accents, CJK, emoji, white space, controls and the characters encoded
/// words are made of.
fn header_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            4 => proptest::char::range('a', 'z'),
            1 => prop::sample::select(vec![' ', ' ', '\t', '=', '?', '_', '\u{7}', '\r', '\n', '\0']),
            2 => prop::sample::select(vec!['é', 'ő', 'ß', 'Ж', '日', '本', '\u{1f600}', '\u{feff}', '\u{fffd}']),
            1 => any::<char>(),
        ],
        0..120,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #[test]
    fn no_label_and_no_bytes_make_decoding_panic(label in "\\PC{0,24}", bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let _ = decode(&label, &bytes);
    }

    #[test]
    fn every_known_charset_decodes_any_bytes(index in 0..LABELS.len(), bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        prop_assert!(decode(LABELS[index], &bytes).is_ok());
    }

    #[test]
    fn utf_8_text_decodes_as_itself(text in "\\PC{0,64}") {
        let text = text.trim_start_matches('\u{feff}');
        prop_assert_eq!(Charset::UTF_8.decode(text.as_bytes()), Decoded { text: text.to_owned(), malformed: false });
    }

    #[test]
    fn a_single_byte_charset_gives_one_character_per_byte(index in 0..SINGLE_BYTE.len(), bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let decoded = decode(SINGLE_BYTE[index], &bytes).unwrap();
        prop_assert!(!decoded.malformed);
        prop_assert_eq!(decoded.text.chars().count(), bytes.len());
    }

    #[test]
    fn written_text_reads_back_as_itself(text in header_text()) {
        let written = encode_text(&text);
        let expected: String = text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect();
        prop_assert_eq!(decode_text(&written), Decoded { text: expected, malformed: false }, "{:?}", written);
    }

    #[test]
    fn written_text_is_one_header_line_of_short_words(text in header_text()) {
        let written = encode_text(&text);
        prop_assert!(!written.contains(['\r', '\n', '\0']), "{:?}", written);
        for part in written.split([' ', '\t']).filter(|part| part.starts_with("=?UTF-8?")) {
            prop_assert!(part.len() <= MAX_WORD_LEN, "{:?}", part);
            prop_assert!(part.is_ascii(), "{:?}", part);
            prop_assert!(decode_word(part).is_ok(), "{:?}", part);
        }
    }

    #[test]
    fn no_text_makes_reading_words_panic(text in "\\PC{0,128}") {
        let _ = decode_text(&text);
        let _ = decode_word(&text);
    }

    #[test]
    fn word_shaped_noise_reads_or_errs(
        text in "(=\\?[-a-zA-Z0-9_*=]{0,12}\\?[bBqQx?]{0,2}\\??[-A-Za-z0-9+/=_?]{0,24}\\??=?[ \t]?){1,4}"
    ) {
        let decoded = decode_text(&text);
        if !decoded.malformed {
            // What reads cleanly reads the same after a round through the writer.
            prop_assert_eq!(decode_text(&encode_text(&decoded.text)).text, decoded.text.replace(['\r', '\n', '\0'], ""));
        }
        let _ = decode_word(&text);
    }
}
