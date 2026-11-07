// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::num::NonZeroU16;

use super::{Account, AccountError, Host, Security, Server};
use crate::{AccountId, EmailAddress};

fn host(text: &str) -> Host {
    Host::new(text).unwrap()
}

fn port(number: u16) -> NonZeroU16 {
    NonZeroU16::new(number).unwrap()
}

#[test]
fn host_names_and_address_literals_are_hosts() {
    for text in
        ["imap.example.org", "IMAP.Example.org", "mail-1.example.org", "localhost", "192.0.2.10", "[2001:db8::1]", "[::1]", "posta.példa.example"]
    {
        assert_eq!(host(text).as_str(), text, "{text}");
    }
    assert_eq!(host(" imap.example.org ").as_str(), "imap.example.org");
    assert_eq!(host("imap.example.org.").as_str(), "imap.example.org");
}

#[test]
fn broken_hosts_are_errors() {
    let too_long_label = format!("{}.example.org", "a".repeat(64));
    let too_long = ["a"; 128].join(".");
    for text in [
        "",
        " ",
        "imap example.org",
        "imap..example.org",
        ".example.org",
        "-imap.example.org",
        "imap-.example.org",
        "imap.example.org/x",
        "user@imap.example.org",
        "imap.example.org:993",
        "imap\r\n.example.org",
        "[2001:db8::1",
        "[]",
        "[2001:db8::zz]",
        too_long_label.as_str(),
        too_long.as_str(),
    ] {
        assert_eq!(Host::new(text), Err(AccountError::BadHost), "{text:?}");
    }
}

#[test]
fn hosts_compare_without_case_and_are_checked_on_the_way_in() {
    assert_eq!(host("IMAP.example.org"), host("imap.EXAMPLE.org"));
    let json = serde_json::to_string(&host("imap.example.org")).unwrap();
    assert_eq!(json, "\"imap.example.org\"");
    assert_eq!(serde_json::from_str::<Host>(&json).unwrap(), host("imap.example.org"));
    assert!(serde_json::from_str::<Host>("\"bad host\"").is_err());
    assert_eq!(host("imap.example.org").to_string(), "imap.example.org");
}

#[test]
fn every_server_is_encrypted() {
    assert_eq!(Security::Tls.imap_port(), port(993));
    assert_eq!(Security::StartTls.imap_port(), port(143));
    assert_eq!(Security::Tls.submission_port(), port(465));
    assert_eq!(Security::StartTls.submission_port(), port(587));
    // There is no plain text setting to deserialise into.
    assert!(serde_json::from_str::<Security>("\"None\"").is_err());
    assert_eq!(serde_json::from_str::<Security>("\"StartTls\"").unwrap(), Security::StartTls);
}

#[test]
fn a_server_names_host_port_and_security() {
    let server = Server::new(host("imap.example.org"), port(993), Security::Tls);
    assert_eq!(server.host(), &host("imap.example.org"));
    assert_eq!(server.port(), port(993));
    assert_eq!(server.security(), Security::Tls);
    assert_eq!(server.to_string(), "imap.example.org:993 (TLS)");
    let starttls = Server::new(host("[2001:db8::1]"), port(143), Security::StartTls);
    assert_eq!(starttls.to_string(), "[2001:db8::1]:143 (STARTTLS)");
    assert!(serde_json::from_str::<Server>(r#"{"host":"imap.example.org","port":0,"security":"Tls"}"#).is_err());
}

#[test]
fn an_account_holds_no_secret() {
    let imap = Server::new(host("imap.example.org"), port(993), Security::Tls);
    let smtp = Server::new(host("smtp.example.org"), port(465), Security::Tls);
    let address = EmailAddress::new("anna@example.org").unwrap();
    let account = Account::new(AccountId::new(1), " Work\r\n mail ", address.clone(), "anna", imap.clone())
        .unwrap()
        .with_display_name(Some("Anna Kovács"))
        .with_submission(Some(smtp.clone()));
    assert_eq!(account.id(), AccountId::new(1));
    assert_eq!(account.label(), "Work mail");
    assert_eq!(account.address(), &address);
    assert_eq!(account.display_name(), Some("Anna Kovács"));
    assert_eq!(account.username(), "anna");
    assert_eq!(account.imap(), &imap);
    assert_eq!(account.submission(), Some(&smtp));
    let json = serde_json::to_string(&account).unwrap();
    assert!(!json.contains("password") && !json.contains("token"), "{json}");
    assert_eq!(serde_json::from_str::<Account>(&json).unwrap(), account);
}

#[test]
fn an_account_label_defaults_to_the_address() {
    let imap = Server::new(host("imap.example.org"), port(993), Security::Tls);
    let address = EmailAddress::new("anna@example.org").unwrap();
    let account = Account::new(AccountId::new(1), "  ", address, "anna", imap).unwrap();
    assert_eq!(account.label(), "anna@example.org");
    assert_eq!(account.display_name(), None);
    assert_eq!(account.submission(), None);
}

#[test]
fn a_broken_username_is_an_error() {
    let imap = Server::new(host("imap.example.org"), port(993), Security::Tls);
    let address = EmailAddress::new("anna@example.org").unwrap();
    let long = "u".repeat(Account::MAX_USERNAME_LEN + 1);
    for username in ["", "an\rna", "anna\n", "an\0na", long.as_str()] {
        assert_eq!(Account::new(AccountId::new(1), "Work", address.clone(), username, imap.clone()), Err(AccountError::BadUsername), "{username:?}");
    }
    let json = r#"{"id":1,"label":"Work","address":"anna@example.org","display_name":null,"username":"a\r\nb","imap":{"host":"imap.example.org","port":993,"security":"Tls"},"submission":null}"#;
    assert!(serde_json::from_str::<Account>(json).is_err());
}

#[test]
fn account_errors_read_as_sentences() {
    assert_eq!(AccountError::BadHost.to_string(), "the server name is not a host name or an IP address");
    assert_eq!(AccountError::BadUsername.to_string(), "the user name is empty, too long or holds a control character");
}

#[test]
fn a_signature_keeps_its_lines_and_loses_other_control_characters() {
    let imap = Server::new(host("imap.example.org"), port(993), Security::Tls);
    let address = EmailAddress::new("anna@example.org").unwrap();
    let account = Account::new(AccountId::new(1), "Work", address, "anna", imap).unwrap();
    assert_eq!(account.signature(), None);
    let signed = account.clone().with_signature(Some("Anna\r\nExample Ltd.\t\u{1b}[2J\0\n"));
    assert_eq!(signed.signature(), Some("Anna\nExample Ltd.\t[2J"));
    for blank in ["", " \n\r\n\t", "\0"] {
        assert_eq!(account.clone().with_signature(Some(blank)).signature(), None, "{blank:?}");
    }
    let long = "é".repeat(Account::MAX_SIGNATURE_CHARS + 5);
    assert_eq!(account.clone().with_signature(Some(&long)).signature().map(|text| text.chars().count()), Some(Account::MAX_SIGNATURE_CHARS));
    let json = serde_json::to_string(&signed).unwrap();
    assert_eq!(serde_json::from_str::<Account>(&json).unwrap(), signed);
    // A record written before signatures reads back without one.
    let old = r#"{"id":1,"label":"Work","address":"anna@example.org","display_name":null,"username":"anna","imap":{"host":"imap.example.org","port":993,"security":"Tls"},"submission":null}"#;
    assert_eq!(serde_json::from_str::<Account>(old).unwrap(), account);
}
