// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_TEXT_CHARS, Secrets, tidy};

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
