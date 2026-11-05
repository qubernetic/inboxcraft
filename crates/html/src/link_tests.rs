// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{clean, looks_like_domain, percent_decode, same_site, scheme, target, web_host};
use crate::LinkTarget;

#[test]
fn percent_decoding_leaves_bad_escapes() {
    assert_eq!(percent_decode("a%20b%2"), "a b%2");
    assert_eq!(percent_decode("%zz%41"), "%zzA");
    assert_eq!(percent_decode("%ff"), "\u{fffd}");
    assert_eq!(percent_decode(""), "");
}

#[test]
fn hosts_lose_user_port_and_trailing_dot() {
    assert_eq!(web_host("https://u:p@Example.org.:80/x").as_deref(), Some("example.org"));
    assert_eq!(web_host("http://[2001:db8::1]:8080/").as_deref(), Some("2001:db8::1"));
    assert_eq!(web_host("https://a.example\\@b.example/").as_deref(), Some("a.example"));
    assert_eq!(web_host("https://a%2eexample/"), None);
    assert_eq!(web_host("https:nohost"), None);
    assert_eq!(web_host("nocolon"), None);
}

#[test]
fn schemes_must_be_well_formed() {
    assert_eq!(scheme("HTTPS://x").as_deref(), Some("https"));
    assert_eq!(scheme("web+x:y").as_deref(), Some("web+x"));
    assert_eq!(scheme("1http://x"), None);
    assert_eq!(scheme(":x"), None);
    assert_eq!(scheme("a b:x"), None);
}

#[test]
fn cleaning_drops_browser_white_space_and_refuses_controls() {
    assert_eq!(clean("  a\tb\r\nc d \u{0} ").as_deref(), Some("abc%20d"));
    assert_eq!(clean("a\u{1}b"), None);
    assert_eq!(clean(" \n "), None);
}

#[test]
fn domains_need_a_dot_and_a_word_at_the_end() {
    assert!(looks_like_domain("bank.example"));
    assert!(looks_like_domain("bánk.példa."));
    assert!(!looks_like_domain("localhost"));
    assert!(!looks_like_domain("v2.0"));
    assert!(!looks_like_domain("a..example"));
    assert!(!looks_like_domain("a_b.example"));
    assert!(!looks_like_domain(""));
}

#[test]
fn same_site_allows_subdomains_only() {
    assert!(same_site("bank.example", "login.bank.example"));
    assert!(same_site("www.bank.example", "bank.example"));
    assert!(!same_site("bank.example", "evilbank.example"));
    assert!(!same_site("login.bank.example", "bank.example"));
}

#[test]
fn mail_targets_need_an_address() {
    assert!(matches!(target("mailto:%0d%0aBcc:x@example.org"), LinkTarget::Refused));
    assert!(matches!(target("MAILTO:a@example.org"), LinkTarget::Mail { .. }));
}
