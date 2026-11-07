// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use proptest::prelude::*;

use super::{decode_utf7, encode_utf7};

#[test]
fn the_examples_of_the_rfc() {
    // RFC 3501 section 5.1.3.
    assert_eq!(decode_utf7("~peter/mail/&U,BTFw-/&ZeVnLIqe-"), "~peter/mail/台北/日本語");
    assert_eq!(encode_utf7("~peter/mail/台北/日本語"), "~peter/mail/&U,BTFw-/&ZeVnLIqe-");
    assert_eq!(decode_utf7("&Jjo-!"), "☺!");
    assert_eq!(encode_utf7("☺!"), "&Jjo-!");
}

#[test]
fn ordinary_names() {
    assert_eq!(encode_utf7("INBOX"), "INBOX");
    assert_eq!(encode_utf7("Tom & Jerry"), "Tom &- Jerry");
    assert_eq!(decode_utf7("Tom &- Jerry"), "Tom & Jerry");
    assert_eq!(encode_utf7("Entwürfe"), "Entw&APw-rfe");
    assert_eq!(decode_utf7("Entw&APw-rfe"), "Entwürfe");
    assert_eq!(encode_utf7("Elküldött"), "Elk&APw-ld&APY-tt");
    // Outside the basic plane: a surrogate pair.
    assert_eq!(decode_utf7(&encode_utf7("📬 Post")), "📬 Post");
    assert_eq!(encode_utf7("📬"), "&2D3c7A-");
    // Control characters are encoded too.
    assert_eq!(encode_utf7("a\tb"), "a&AAk-b");
    assert_eq!(encode_utf7(""), "");
}

#[test]
fn junk_is_kept_as_it_is() {
    for junk in ["&", "a&b", "&&&", "&!!!-", "&A-", "&AAAAA-x", "R&D", "&2D0-", "&-&", "é&é-"] {
        assert!(!decode_utf7(junk).is_empty(), "{junk}");
    }
    assert_eq!(decode_utf7("R&D"), "R&D");
    assert_eq!(decode_utf7("&!!!-"), "&!!!-");
    // A lone surrogate is not text.
    assert_eq!(decode_utf7("&2D0-"), "&2D0-");
    assert_eq!(decode_utf7("AT&T-Mobile"), "AT&T-Mobile");
}

proptest! {
    #[test]
    fn any_name_round_trips(name in "\\PC{0,40}") {
        let encoded = encode_utf7(&name);
        prop_assert!(encoded.bytes().all(|b| (0x20..=0x7E).contains(&b)), "{encoded}");
        prop_assert_eq!(decode_utf7(&encoded), name);
    }

    #[test]
    fn any_text_round_trips_with_control_characters(name in proptest::collection::vec(any::<char>(), 0..40)) {
        let name: String = name.into_iter().collect();
        prop_assert_eq!(decode_utf7(&encode_utf7(&name)), name);
    }

    #[test]
    fn decoding_never_panics(name in "[&A-Za-z0-9+,\\-é ]{0,60}") {
        let _ = decode_utf7(&name);
    }
}
