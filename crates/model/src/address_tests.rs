// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Address, AddressError, EmailAddress, MAX_NAME_CHARS, labels};

fn email(text: &str) -> EmailAddress {
    EmailAddress::new(text).unwrap()
}

fn address(name: &str, text: &str) -> Address {
    Address::new(Some(name), email(text))
}

#[test]
fn an_address_keeps_its_text_and_splits_at_the_last_at_sign() {
    let address = email(" Anna.Kovacs@Example.org ");
    assert_eq!(address.as_str(), "Anna.Kovacs@Example.org");
    assert_eq!(address.local_part(), "Anna.Kovacs");
    assert_eq!(address.domain(), "Example.org");
    let quoted = email("\"a@b\"@example.org");
    assert_eq!(quoted.local_part(), "\"a@b\"");
    assert_eq!(quoted.domain(), "example.org");
    assert_eq!(email("info@[192.0.2.1]").domain(), "[192.0.2.1]");
    assert_eq!(email("józsef@példa.example").domain(), "példa.example");
}

#[test]
fn broken_addresses_are_errors() {
    let cases = [
        ("", AddressError::Empty),
        ("   ", AddressError::Empty),
        ("anna", AddressError::NoAtSign),
        ("@example.org", AddressError::EmptyLocalPart),
        ("anna@", AddressError::EmptyDomain),
        ("anna@exa mple.org", AddressError::BadDomain),
        ("anna@.example.org", AddressError::BadDomain),
        ("anna@example.org.", AddressError::BadDomain),
        ("anna@exa..mple.org", AddressError::BadDomain),
        ("anna@<example.org>", AddressError::BadDomain),
        ("anna@[192.0.2.1", AddressError::BadDomain),
        ("anna@[]", AddressError::BadDomain),
        ("anna@[192.0 .2.1]", AddressError::BadDomain),
        ("an\rna@example.org", AddressError::ControlCharacter),
        ("anna@example.org\n", AddressError::ControlCharacter),
        ("anna@exam\0ple.org", AddressError::ControlCharacter),
        ("anna@exam\u{7f}ple.org", AddressError::ControlCharacter),
        ("an\u{202e}na@example.org", AddressError::InvisibleMark),
        ("an\u{200b}na@example.org", AddressError::InvisibleMark),
        ("anna@exa\u{202e}gro.elpmaxe", AddressError::InvisibleMark),
        ("anna@exa\u{200b}mple.org", AddressError::InvisibleMark),
        ("\u{feff}anna@example.org", AddressError::InvisibleMark),
        ("an na@example.org", AddressError::BadLocalPart),
        ("<anna@example.org", AddressError::BadLocalPart),
    ];
    for (text, error) in cases {
        assert_eq!(EmailAddress::new(text), Err(error), "{text:?}");
    }
    let long = format!("{}@example.org", "a".repeat(EmailAddress::MAX_LEN));
    assert_eq!(EmailAddress::new(&long), Err(AddressError::TooLong));
    let fits = format!("{}@example.org", "a".repeat(EmailAddress::MAX_LEN - 12));
    assert!(EmailAddress::new(&fits).is_ok());
}

#[test]
fn errors_read_as_sentences() {
    for error in [
        AddressError::Empty,
        AddressError::TooLong,
        AddressError::ControlCharacter,
        AddressError::InvisibleMark,
        AddressError::NoAtSign,
        AddressError::EmptyLocalPart,
        AddressError::BadLocalPart,
        AddressError::EmptyDomain,
        AddressError::BadDomain,
    ] {
        let text = error.to_string();
        assert!(text.starts_with("the address"), "{text}");
    }
}

#[test]
fn addresses_compare_without_case() {
    assert_eq!(email("Anna@Example.ORG"), email("anna@example.org"));
    assert_ne!(email("anna@example.org"), email("anna@example.net"));
    let mut set = std::collections::BTreeSet::new();
    set.insert(email("Anna@example.org"));
    set.insert(email("anna@EXAMPLE.org"));
    assert_eq!(set.len(), 1);
    assert!(email("b@example.org") > email("A@example.org"));
    let hashed: std::collections::HashSet<_> = [email("Anna@example.org"), email("anna@EXAMPLE.org")].into();
    assert_eq!(hashed.len(), 1);
}

#[test]
fn addresses_serialise_as_their_text_and_are_checked_on_the_way_in() {
    let text = serde_json::to_string(&email("Anna@example.org")).unwrap();
    assert_eq!(text, "\"Anna@example.org\"");
    assert_eq!(serde_json::from_str::<EmailAddress>(&text).unwrap().as_str(), "Anna@example.org");
    assert!(serde_json::from_str::<EmailAddress>("\"no at sign\"").is_err());
    let full = address("Anna", "anna@example.org");
    let json = serde_json::to_string(&full).unwrap();
    assert_eq!(serde_json::from_str::<Address>(&json).unwrap(), full);
}

#[test]
fn display_names_are_cleaned() {
    assert_eq!(address("  Anna   Kovács ", "anna@example.org").name(), Some("Anna Kovács"));
    assert_eq!(address("Evil\r\nBcc: x", "anna@example.org").name(), Some("Evil Bcc: x"));
    assert_eq!(address("\u{202e}gnp.exe", "anna@example.org").name(), Some("gnp.exe"));
    assert_eq!(address("a\u{2066}b\u{200f}c\0d", "anna@example.org").name(), Some("abcd"));
    assert_eq!(address(" \t ", "anna@example.org").name(), None);
    assert_eq!(Address::new(None, email("anna@example.org")).name(), None);
    let long = address(&"é".repeat(MAX_NAME_CHARS + 10), "anna@example.org");
    assert_eq!(long.name().map(|name| name.chars().count()), Some(MAX_NAME_CHARS));
    let space_at_the_cut = address(&format!("{} b", "a".repeat(MAX_NAME_CHARS - 1)), "anna@example.org");
    assert_eq!(space_at_the_cut.name().map(|name| name.chars().count()), Some(MAX_NAME_CHARS - 1));
}

#[test]
fn a_name_from_storage_is_cleaned_too() {
    let json = r#"{"name":"Ev\u0000il\r\n","email":"anna@example.org"}"#;
    assert_eq!(serde_json::from_str::<Address>(json).unwrap().name(), Some("Evil"));
}

#[test]
fn two_contacts_with_the_same_name_are_two_contacts() {
    let first = address("Anna Kovács", "anna@example.org");
    let second = address("Anna Kovács", "anna.kovacs@example.net");
    assert_ne!(first, second);
    assert_eq!(first, address("Someone Else", "ANNA@example.org").with_name(Some("Anna Kovács")));
}

#[test]
fn labels_show_the_address_when_names_collide() {
    let list = [
        address("Anna Kovács", "anna@example.org"),
        address("Béla", "bela@example.org"),
        address("anna kovács", "anna.kovacs@example.net"),
        Address::new(None, email("noname@example.org")),
    ];
    assert_eq!(labels(&list), ["Anna Kovács <anna@example.org>", "Béla", "anna kovács <anna.kovacs@example.net>", "noname@example.org"]);
}

#[test]
fn the_same_contact_twice_is_not_a_collision() {
    let list = [address("Anna", "anna@example.org"), address("Anna", "ANNA@example.org")];
    assert_eq!(labels(&list), ["Anna", "Anna"]);
}

#[test]
fn a_name_that_looks_like_another_address_shows_the_real_one() {
    let list = [address("bank@example.org", "someone@example.net"), address("anna@example.org", "anna@example.org")];
    assert_eq!(labels(&list), ["bank@example.org <someone@example.net>", "anna@example.org"]);
}

#[test]
fn the_full_form_names_both() {
    assert_eq!(address("Anna", "anna@example.org").to_string(), "Anna <anna@example.org>");
    assert_eq!(Address::new(None, email("anna@example.org")).to_string(), "anna@example.org");
    assert_eq!(email("anna@example.org").to_string(), "anna@example.org");
    assert_eq!(address("Anna", "anna@example.org").email().as_str(), "anna@example.org");
}
