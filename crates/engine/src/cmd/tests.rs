// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use crate::test_support::session;
use crate::{Caller, EngineError};

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

#[test]
fn folder_list_shows_roles_counts_and_unified_views() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "folder.list", &json!({})).unwrap();
    let folders = answer["folders"].as_array().unwrap();
    assert_eq!(folders.len(), 5);
    assert_eq!(
        folders[0],
        json!({
            "id": 1,
            "account": 1,
            "name": "INBOX",
            "leaf": "INBOX",
            "delimiter": "/",
            "role": "inbox",
            "special_use": [],
            "selectable": true,
            "synced": false,
            "messages": 2,
            "unread": 2,
        })
    );
    assert_eq!(folders[1]["role"], json!("sent"));
    assert_eq!(folders[1]["special_use"], json!(["sent"]));
    assert_eq!(folders[4]["role"], json!("sent"), "Elküldött is Sent by its Hungarian name");
    assert_eq!(
        answer["views"],
        json!([
            { "role": "inbox", "folders": [1, 4] },
            { "role": "sent", "folders": [2, 5] },
            { "role": "archive", "folders": [3] },
        ])
    );
    assert_eq!(
        answer["roles_to_ask"],
        json!([
            { "account": 1, "role": "drafts" },
            { "account": 1, "role": "trash" },
            { "account": 1, "role": "junk" },
            { "account": 2, "role": "drafts" },
            { "account": 2, "role": "trash" },
            { "account": 2, "role": "junk" },
            { "account": 2, "role": "archive" },
        ])
    );
}

#[test]
fn folder_list_narrows_to_one_account() {
    let mut session = session();
    let answer = session.run("folder.list", &json!({ "account": 2 })).unwrap();
    let ids: Vec<u64> = answer["folders"].as_array().unwrap().iter().map(|folder| folder["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, [4, 5]);
    assert_eq!(answer["views"][0], json!({ "role": "inbox", "folders": [4] }));
    assert_eq!(session.run("folder.list", &json!({ "account": 9 })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
}

fn ids(answer: &serde_json::Value) -> Vec<u64> {
    answer["messages"].as_array().unwrap().iter().map(|message| message["id"].as_u64().unwrap()).collect()
}

#[test]
fn the_unified_inbox_is_merged_by_date() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "mail.list", &json!({})).unwrap();
    // Lunch (60 s ago, account 1), Garden party (30 min, account 2), Budget (1 h, account 1), then the undated one.
    assert_eq!(ids(&answer), [2, 3, 1, 4]);
    assert_eq!(answer["view"], json!({ "role": "inbox" }));
    assert_eq!(answer["total"], json!(4));
    assert_eq!(answer["account"], json!(null));
    let lunch = &answer["messages"][0];
    assert_eq!(lunch["subject"], json!("Lunch on Friday"));
    assert_eq!(lunch["date"], json!("2026-11-05T08:59:00Z"));
    assert_eq!(lunch["sent"], json!(null));
    assert_eq!(lunch["from"], json!([{ "name": "Anna Example", "address": "anna@example.org" }]));
    assert_eq!(lunch["seen"], json!(false));
    assert_eq!(lunch["flags"], json!([]));
    assert_eq!(answer["messages"][3]["date"], json!(null));
}

#[test]
fn a_unified_view_narrows_to_one_account_and_pages() {
    let mut session = session();
    assert_eq!(ids(&session.run("mail.list", &json!({ "role": "inbox", "account": 2 })).unwrap()), [3, 4]);
    assert_eq!(ids(&session.run("mail.list", &json!({ "role": "sent" })).unwrap()), [5]);
    assert_eq!(ids(&session.run("mail.list", &json!({ "role": "drafts" })).unwrap()), Vec::<u64>::new());
    let page = session.run("mail.list", &json!({ "limit": 2, "offset": 1 })).unwrap();
    assert_eq!(ids(&page), [3, 1]);
    assert_eq!((page["offset"].clone(), page["limit"].clone(), page["total"].clone()), (json!(1), json!(2), json!(4)));
    assert_eq!(session.run("mail.list", &json!({ "limit": 0 })).unwrap()["limit"], json!(1));
    assert_eq!(session.run("mail.list", &json!({ "limit": u64::MAX })).unwrap()["limit"], json!(1000));
    assert_eq!(ids(&session.run("mail.list", &json!({ "offset": u64::MAX })).unwrap()), Vec::<u64>::new());
}

#[test]
fn one_folder_lists_alone() {
    let mut session = session();
    let answer = session.run("mail.list", &json!({ "folder": 4 })).unwrap();
    assert_eq!(ids(&answer), [3, 4]);
    assert_eq!(answer["view"], json!({ "folder": 4 }));
    assert_eq!(ids(&session.run("mail.list", &json!({ "folder": 4, "account": 2 })).unwrap()), [3, 4]);
    assert_eq!(session.run("mail.list", &json!({ "folder": 4, "account": 1 })).unwrap_err(), EngineError::NotFound { what: "folder", id: 4 });
    assert_eq!(session.run("mail.list", &json!({ "folder": 99 })).unwrap_err(), EngineError::NotFound { what: "folder", id: 99 });
}

#[test]
fn mail_list_refuses_what_does_not_fit() {
    let mut session = session();
    for params in [json!({ "folder": 1, "role": "inbox" }), json!({ "role": "spam" }), json!({ "limit": -1 }), json!({ "folders": 1 })] {
        assert_eq!(session.run("mail.list", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert_eq!(session.run("mail.list", &json!({ "account": 3 })).unwrap_err(), EngineError::NotFound { what: "account", id: 3 });
}
