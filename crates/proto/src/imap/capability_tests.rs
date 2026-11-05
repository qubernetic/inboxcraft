// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Capabilities, MAX_CAPABILITIES, MAX_CAPABILITY_LEN};

#[test]
fn capabilities_are_upper_cased_and_kept_once() {
    let caps = Capabilities::from_words(["IMAP4rev1", "starttls", "AUTH=PLAIN", "STARTTLS", "Login\u{7}", "x\"y", ""]);
    assert_eq!(caps.iter().collect::<Vec<_>>(), ["IMAP4REV1", "STARTTLS", "AUTH=PLAIN"]);
    assert!(caps.has("starttls"));
    assert!(caps.has("auth=plain"));
    assert!(!caps.has("LOGINDISABLED"));
    assert!(!Capabilities::default().has("IMAP4REV1"));
}

#[test]
fn long_words_and_long_lists_are_cut() {
    let long = "X".repeat(MAX_CAPABILITY_LEN + 1);
    let fits = "Y".repeat(MAX_CAPABILITY_LEN);
    assert_eq!(Capabilities::from_words([long.as_str(), fits.as_str()]).iter().collect::<Vec<_>>(), [fits.as_str()]);
    let many: Vec<String> = (0..MAX_CAPABILITIES + 10).map(|n| format!("X-{n}")).collect();
    let caps = Capabilities::from_words(many.iter().map(String::as_str));
    assert_eq!(caps.iter().count(), MAX_CAPABILITIES);
    assert!(!caps.has(&format!("X-{MAX_CAPABILITIES}")));
}
