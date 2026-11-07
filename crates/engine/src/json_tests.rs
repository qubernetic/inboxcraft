// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{ConnectionState, ErrorKind, FolderId, Security};
use inboxcraft_time::Timestamp;
use serde_json::json;

use super::{account, base64, date, failure, name, snake};
use crate::session::AccountState;
use crate::test_support::{NOW, at};

#[test]
fn dates_are_iso_8601_in_utc() {
    assert_eq!(date(at(NOW)), json!("2026-11-05T09:00:00Z"));
    assert_eq!(date(Timestamp::UNIX_EPOCH), json!("1970-01-01T00:00:00Z"));
    assert_eq!(date(Timestamp::MIN), json!("0000-01-01T00:00:00Z"));
    assert_eq!(date(Timestamp::MAX), json!("10000-12-31T23:59:59Z"));
}

#[test]
fn names_are_snake_case() {
    assert_eq!(snake("StartTls"), "start_tls");
    assert_eq!(snake("Tls"), "tls");
    assert_eq!(snake("already_snake"), "already_snake");
    assert_eq!(name(Security::StartTls), json!("start_tls"));
    assert_eq!(name(ErrorKind::CertificateWrongHost), json!("certificate_wrong_host"));
}

#[test]
fn an_account_shows_its_servers_connection_and_failure() {
    let mut state = AccountState { connection: Some(ConnectionState::BackingOff { attempt: 2, retry_in_secs: 8 }), ..AccountState::default() };
    let shown = account(&crate::test_support::account(1, "anna@example.org"), Some(&state));
    assert_eq!(shown["imap"], json!({ "host": "imap.example.org", "port": 993, "security": "tls" }));
    assert_eq!(shown["submission"], json!(null));
    assert_eq!(shown["connection"], json!({ "state": "backing_off", "attempt": 2, "retry_in_secs": 8 }));
    assert_eq!(shown["failure"], json!(null));
    state.connection = Some(ConnectionState::Watching);
    state.failure = Some((ErrorKind::FolderGone, Some(FolderId::new(4))));
    let shown = account(&crate::test_support::account(1, "anna@example.org"), Some(&state));
    assert_eq!(shown["connection"], json!({ "state": "watching" }));
    assert_eq!(shown["failure"], failure(ErrorKind::FolderGone, Some(FolderId::new(4))));
    assert_eq!(shown["failure"]["message"], json!("the folder no longer exists on the server"));
    let unknown = account(&crate::test_support::account(1, "anna@example.org"), None);
    assert_eq!(unknown["connection"], json!(null));
}

#[test]
fn base64_follows_rfc_4648() {
    for (bytes, encoded) in
        [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")]
    {
        assert_eq!(base64(bytes.as_bytes()), encoded);
    }
    assert_eq!(base64(&[0xfb, 0xff, 0xfe]), "+//+");
}
