// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use crate::Caller;
use crate::test_support::session;

#[test]
fn account_list_shows_every_account_without_a_secret() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "account.list", &json!({})).unwrap();
    let accounts = answer["accounts"].as_array().unwrap();
    assert_eq!(accounts.len(), 2);
    assert_eq!(
        accounts[0],
        json!({
            "id": 1,
            "label": "Account 1",
            "address": "anna@example.org",
            "display_name": null,
            "username": "anna@example.org",
            "imap": { "host": "imap.example.org", "port": 993, "security": "tls" },
            "submission": null,
            "connection": null,
            "failure": null,
        })
    );
    assert_eq!(accounts[1]["address"], json!("bob@example.net"));
    assert!(!answer.to_string().to_lowercase().contains("password"));
}
