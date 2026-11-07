// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use proptest::prelude::*;

use super::{TOKEN_BYTES, Token};

fn token(fill: u8) -> Token {
    Token::from_bytes([fill; TOKEN_BYTES])
}

#[test]
fn a_token_is_written_as_lowercase_hex() {
    let mut bytes = [0_u8; TOKEN_BYTES];
    bytes[0] = 0xab;
    bytes[31] = 0x09;
    let text = Token::from_bytes(bytes).text();
    assert_eq!(text.len(), 64);
    assert!(text.starts_with("ab00"));
    assert!(text.ends_with("0009"));
}

#[test]
fn a_written_token_reads_back() {
    let written = token(0x5a).text();
    assert_eq!(Token::parse(&written), Some(token(0x5a)));
    assert_eq!(Token::parse(&written.to_uppercase()), Some(token(0x5a)));
}

#[test]
fn anything_but_64_hex_digits_is_no_token() {
    let good = token(1).text();
    for bad in [
        String::new(),
        good[..63].to_owned(),
        format!("{good}0"),
        format!(" {}", &good[1..]),
        format!("{}g", &good[..63]),
        format!("{}é", &good[..62]),
        format!("+{}", &good[1..]),
    ] {
        assert_eq!(Token::parse(&bad), None, "{bad:?}");
    }
}

#[test]
fn tokens_compare_by_every_byte() {
    let mut bytes = [7_u8; TOKEN_BYTES];
    assert_eq!(Token::from_bytes(bytes), token(7));
    bytes[31] = 8;
    assert_ne!(Token::from_bytes(bytes), token(7));
    bytes[31] = 7;
    bytes[0] = 0;
    assert_ne!(Token::from_bytes(bytes), token(7));
}

#[test]
fn a_token_never_shows_in_debug_output() {
    let shown = format!("{:?}", token(0xcd));
    assert!(!shown.contains("cd"), "{shown}");
    assert_eq!(shown, "Token(..)");
}

proptest! {
    #[test]
    fn every_token_reads_back(bytes in proptest::array::uniform32(any::<u8>())) {
        let token = Token::from_bytes(bytes);
        prop_assert_eq!(Token::parse(&token.text()), Some(token));
    }

    #[test]
    fn no_text_panics(text in "\\PC{0,80}") {
        let _ = Token::parse(&text);
    }
}
