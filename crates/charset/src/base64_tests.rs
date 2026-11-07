// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{decode, encode, encoded_len};

#[test]
fn the_rfc_4648_vectors_encode_and_decode() {
    let vectors = [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")];
    for (plain, coded) in vectors {
        assert_eq!(encode(plain.as_bytes()), coded);
        assert_eq!(encoded_len(plain.len()), coded.len());
        assert_eq!(decode(coded.as_bytes()), Some(plain.as_bytes().to_vec()), "{coded}");
    }
}

#[test]
fn every_byte_goes_through() {
    let bytes: Vec<u8> = (0..=255).collect();
    let coded = encode(&bytes);
    assert!(coded.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)));
    assert_eq!(decode(coded.as_bytes()), Some(bytes));
}

#[test]
fn missing_padding_is_forgiven() {
    assert_eq!(decode(b"Zg"), Some(b"f".to_vec()));
    assert_eq!(decode(b"Zm8"), Some(b"fo".to_vec()));
    assert_eq!(decode(b"Zg="), Some(b"f".to_vec()));
}

#[test]
fn broken_base64_is_refused() {
    for coded in ["Z", "Zm9vY", "Zg===", "Z=g=", "Zm9v Yg==", "Zm9v\u{e9}", "Zm9v!", "===="] {
        assert_eq!(decode(coded.as_bytes()), None, "{coded:?}");
    }
}
