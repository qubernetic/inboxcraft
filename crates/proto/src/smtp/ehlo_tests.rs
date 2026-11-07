// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Extensions, MAX_EXTENSIONS, MAX_KEYWORD_LEN};

fn read(lines: &[&str]) -> Extensions {
    Extensions::from_lines(lines.iter().copied())
}

#[test]
fn keywords_are_read_in_any_case_with_their_parameters() {
    let extensions = read(&["size 35882577", "8bitmime", "AUTH PLAIN LOGIN", "STARTTLS", "SMTPUTF8"]);
    assert!(extensions.has("SIZE") && extensions.has("8BITMIME") && extensions.has("starttls") && extensions.has("SmtpUtf8"));
    assert!(!extensions.has("PIPELINING"));
    assert_eq!(extensions.size_limit(), Some(35_882_577));
    assert!(extensions.offers_auth("PLAIN") && extensions.offers_auth("login"));
    assert!(!extensions.offers_auth("CRAM-MD5"));
    assert_eq!(extensions.keywords().collect::<Vec<_>>(), ["SIZE", "8BITMIME", "AUTH", "STARTTLS", "SMTPUTF8"]);
}

#[test]
fn a_size_of_zero_or_none_means_no_fixed_limit() {
    assert_eq!(read(&["SIZE"]).size_limit(), None);
    assert_eq!(read(&["SIZE 0"]).size_limit(), None);
    assert_eq!(read(&["SIZE lots"]).size_limit(), None);
    assert_eq!(read(&["SIZE 99999999999999999999999"]).size_limit(), None);
    assert_eq!(read(&[]).size_limit(), None);
}

#[test]
fn the_old_auth_equals_form_counts_too() {
    let extensions = read(&["AUTH=PLAIN LOGIN"]);
    assert!(extensions.offers_auth("PLAIN") && extensions.offers_auth("LOGIN"));
    assert!(!extensions.has("AUTH=PLAIN"));
}

#[test]
fn lines_that_are_not_keywords_are_skipped() {
    let long = "X".repeat(MAX_KEYWORD_LEN + 1);
    let extensions = read(&["", "   ", "-DASH", "ÁRVÍZ", "WITH_UNDERSCORE", &long, "X-OK yes"]);
    assert_eq!(extensions.keywords().collect::<Vec<_>>(), ["X-OK"]);
}

#[test]
fn a_keyword_is_kept_once_and_the_list_is_capped() {
    let lines: Vec<String> = (0..MAX_EXTENSIONS + 5).map(|n| format!("X-{n}")).chain(["X-0".to_owned()]).collect();
    let extensions = Extensions::from_lines(lines.iter().map(String::as_str));
    assert_eq!(extensions.keywords().count(), MAX_EXTENSIONS);
    assert_eq!(read(&["SIZE 10", "SIZE 20"]).size_limit(), Some(10));
}

#[test]
fn parameters_are_capped() {
    let many = format!("AUTH {}", "M ".repeat(MAX_EXTENSIONS * 2));
    let extensions = read(&[&many, "STARTTLS"]);
    assert!(extensions.has("STARTTLS"));
    assert!(extensions.offers_auth("M"));
    let long = format!("AUTH {} PLAIN", "Y".repeat(MAX_KEYWORD_LEN + 1));
    assert!(read(&[&long]).offers_auth("PLAIN"));
}
