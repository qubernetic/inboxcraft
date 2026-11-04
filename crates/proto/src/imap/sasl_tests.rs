// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_CREDENTIALS, base64, check_credentials, plain_response};
use crate::ProtoError;

#[test]
fn base64_of_the_rfc_4648_vectors() {
    for (bytes, text) in
        [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")]
    {
        assert_eq!(base64(bytes.as_bytes()), text);
    }
    assert_eq!(base64(&[0xFB, 0xFF]), "+/8=");
}

#[test]
fn the_plain_response() {
    // RFC 4616 section 4, without an authorisation identity.
    assert_eq!(plain_response("tim", "tanstaaftanstaaf"), Ok("AHRpbQB0YW5zdGFhZnRhbnN0YWFm".to_owned()));
    assert_eq!(plain_response("ádá", "jó"), Ok(base64("\0ádá\0jó".as_bytes())));
}

#[test]
fn credentials_that_cannot_be_sent() {
    for (user, password) in [("", "x"), ("x", ""), ("a\0", "x"), ("x", "p\0ss")] {
        assert!(matches!(plain_response(user, password), Err(ProtoError::BadArgument(_))), "{user:?}");
    }
    let long = "x".repeat(MAX_CREDENTIALS);
    assert!(check_credentials("u", &long).is_err());
    assert!(check_credentials("u", &long[1..]).is_ok());
}
