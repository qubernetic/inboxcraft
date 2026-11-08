// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Credentials, base64};
use crate::DavError;

#[test]
fn base64_pads_as_rfc_4648_writes() {
    let cases: [(&[u8], &str); 7] =
        [(b"", ""), (b"f", "Zg=="), (b"fo", "Zm8="), (b"foo", "Zm9v"), (b"foob", "Zm9vYg=="), (b"fooba", "Zm9vYmE="), (b"\xff\xfe\xfd", "//79")];
    for (bytes, encoded) in cases {
        assert_eq!(base64(bytes), encoded);
    }
}

#[test]
fn the_header_is_basic_with_user_and_password() {
    let credentials = Credentials::new("ada@example.org", "not-a-real-password").unwrap();
    assert_eq!(credentials.user(), "ada@example.org");
    assert_eq!(credentials.authorization(), "Basic YWRhQGV4YW1wbGUub3JnOm5vdC1hLXJlYWwtcGFzc3dvcmQ=");
}

#[test]
fn debug_hides_the_password() {
    let credentials = Credentials::new("ada@example.org", "hunter2").unwrap();
    let shown = format!("{credentials:?}");
    assert!(shown.contains("ada@example.org") && shown.contains("***") && !shown.contains("hunter2"), "{shown}");
}

#[test]
fn what_basic_auth_cannot_carry_is_refused() {
    assert_eq!(Credentials::new("", "x"), Err(DavError::BadArgument("an empty user name")));
    assert_eq!(Credentials::new("ada:x", "x"), Err(DavError::BadArgument("a colon in the user name")));
    assert_eq!(Credentials::new("ada", "a\r\nb"), Err(DavError::BadArgument("a control character")));
    assert_eq!(Credentials::new("a\0da", "x"), Err(DavError::BadArgument("a control character")));
    assert!(Credentials::new("ada", "pass:word é").is_ok());
}
