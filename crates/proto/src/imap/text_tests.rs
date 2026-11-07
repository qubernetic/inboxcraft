// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_TEXT_CHARS, Secrets, tidy};
use crate::ProtoError;

#[test]
fn server_text_is_tidied() {
    assert_eq!(tidy("  5.7.8\tBad\r\n credentials \u{7}"), "5.7.8 Bad credentials");
    let long = tidy(&"é".repeat(1000));
    assert_eq!(long.chars().count(), MAX_TEXT_CHARS + 3);
    assert!(long.ends_with("..."));
    assert_eq!(tidy(""), "");
}

#[test]
fn secrets_are_hidden_longest_first() {
    let mut secrets = Secrets::default();
    secrets.add("hunter2");
    secrets.add("hunter2-and-more");
    secrets.add("hunter2");
    secrets.add("");
    assert_eq!(secrets.tidy("no hunter2-and-more, no hunter2, nothing"), "no ***, no ***, nothing");
    assert_eq!(Secrets::default().tidy("plain\r\ntext"), "plain text");
    assert_eq!(format!("{secrets:?}"), "Secrets(..)");
}

#[test]
fn a_secret_at_the_cut_is_hidden_before_the_cut() {
    let mut secrets = Secrets::default();
    secrets.add("hunter2");
    let text = format!("{}hunter2", "x".repeat(MAX_TEXT_CHARS - 3));
    let shown = secrets.tidy(&text);
    assert!(!shown.contains("hun"), "{shown}");
}

#[test]
fn invisible_direction_and_zero_width_marks_are_dropped() {
    assert_eq!(tidy("abc\u{202e}gpj.exe\u{200b}x"), "abcgpj.exex");
    for mark in ['\u{061c}', '\u{200b}', '\u{200f}', '\u{202a}', '\u{202e}', '\u{2060}', '\u{2069}', '\u{feff}'] {
        assert_eq!(tidy(&format!("a{mark}b")), "ab", "{mark:?}");
        assert!(inboxcraft_model::is_invisible_mark(mark), "{mark:?}");
    }
    // A mark between two words leaves one space, not two.
    assert_eq!(tidy("a \u{200b} b"), "a b");
}

#[test]
fn a_secret_with_edge_white_space_is_hidden_at_the_edge() {
    for password in [" secret", "secret ", "\tsecret"] {
        let mut secrets = Secrets::default();
        secrets.add(password);
        // The server's line is trimmed before it is searched, so the password's own white space may be gone.
        assert_eq!(secrets.tidy("secret is wrong"), "*** is wrong", "{password:?}");
        assert_eq!(secrets.tidy("wrong password secret"), "wrong password ***", "{password:?}");
        assert_eq!(secrets.tidy(&format!("wrong {password} here")), "wrong *** here", "{password:?}");
    }
}

#[test]
fn a_secret_is_found_in_any_case() {
    let mut secrets = Secrets::default();
    secrets.add("hunter2");
    assert!(secrets.shows("XHUNTER2"));
    assert!(secrets.shows("hunter2"));
    assert!(!secrets.shows("HUNTER"));
    assert!(!Secrets::default().shows("HUNTER2"));
}

#[test]
fn credentials_are_hidden_in_every_form_as_sent_and_trimmed() {
    let mut secrets = Secrets::default();
    assert_eq!(secrets.add_credentials("tim", "hunter2\t"), Ok(()));
    // The password, its PLAIN token and the password alone in base64, with and without padding, as sent and trimmed.
    for form in [
        "hunter2\t",
        "hunter2",
        "AHRpbQBodW50ZXIyCQ==",
        "AHRpbQBodW50ZXIyCQ",
        "aHVudGVyMgk=",
        "aHVudGVyMgk",
        "AHRpbQBodW50ZXIy",
        "aHVudGVyMg==",
        "aHVudGVyMg",
    ] {
        assert_eq!(secrets.tidy(&format!("wrong: {form} here")), "wrong: *** here", "{form:?}");
    }
    assert_eq!(Secrets::default().add_credentials("tim", ""), Err(ProtoError::BadArgument("empty user name or password")));
    assert_eq!(Secrets::default().add_credentials("tim", "a\0b"), Err(ProtoError::BadArgument("user name or password with a NUL")));
}
