// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_store::Storage;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{FOLDERS, MESSAGES, mailbox, session};

/// 2026-11-05T09:00:00Z.
fn now() -> Timestamp {
    Timestamp::from_unix(1_793_869_200, 0).unwrap()
}

fn run(command: &str, params: &Value) -> Value {
    session(Box::new(FakeClock::new(now()))).unwrap().run(command, params).unwrap()
}

#[test]
fn every_person_and_account_is_on_example_org() {
    let store = mailbox(now()).unwrap();
    let mut seen = 0;
    for account in store.accounts().unwrap() {
        assert!(account.address().as_str().ends_with("@example.org"), "{}", account.address());
        assert_eq!(account.imap().host().as_str(), "imap.example.org");
        for folder in store.folders(account.id()).unwrap() {
            for entry in store.messages(folder.id()).unwrap() {
                let envelope = entry.message().summary().envelope();
                for address in envelope.from().iter().chain(envelope.to()) {
                    assert!(address.email().as_str().ends_with("@example.org"), "{}", address.email());
                    seen += 1;
                }
            }
        }
    }
    assert_eq!(seen, MESSAGES.len() * 2);
}

#[test]
fn two_accounts_with_their_addresses_and_names() {
    let accounts = run("account.list", &json!({}));
    let shown: Vec<(&str, &str, &str)> = accounts["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|account| (account["label"].as_str().unwrap(), account["address"].as_str().unwrap(), account["display_name"].as_str().unwrap()))
        .collect();
    assert_eq!(shown, [("Personal", "mira@example.org", "Mira Holloway"), ("Studio", "holloway.studio@example.org", "Mira Holloway")]);
}

#[test]
fn every_folder_role_is_there_and_the_unified_views_span_both_accounts() {
    let folders = run("folder.list", &json!({}));
    assert_eq!(folders["folders"].as_array().unwrap().len(), FOLDERS.len());
    let roles: Vec<&str> = folders["folders"].as_array().unwrap().iter().filter(|f| f["account"] == 1).filter_map(|f| f["role"].as_str()).collect();
    for role in ["inbox", "drafts", "sent", "archive", "junk", "trash"] {
        assert!(roles.contains(&role), "{role} missing from {roles:?}");
    }
    let inbox = folders["views"].as_array().unwrap().iter().find(|view| view["role"] == "inbox").unwrap();
    assert_eq!(inbox["folders"], json!([1, 10]));
    assert!(folders["roles_to_ask"].as_array().unwrap().iter().all(|ask| ask["account"] == 2), "{}", folders["roles_to_ask"]);
    let nested = folders["folders"].as_array().unwrap().iter().find(|f| f["name"] == "Clients/Harbour Bakery").unwrap();
    assert_eq!(nested["leaf"], "Harbour Bakery");
}

#[test]
fn the_unified_inbox_is_recent_newest_first_and_partly_unread() {
    let list = run("mail.list", &json!({ "role": "inbox" }));
    let messages = list["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 11);
    let dates: Vec<&str> = messages.iter().map(|m| m["date"].as_str().unwrap()).collect();
    let mut sorted = dates.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(dates, sorted);
    assert_eq!(dates[0], "2026-11-05T08:35:00Z", "25 minutes before now");
    assert!(messages.iter().any(|m| m["account"] == 1) && messages.iter().any(|m| m["account"] == 2));
    assert_eq!(messages.iter().filter(|m| m["seen"] == false).count(), 4);
    assert!(messages.iter().any(|m| m["flagged"] == true));
}

#[test]
fn the_search_finds_sample_mail_by_subject_and_sender() {
    let found = run("mail.search", &json!({ "subject": "logo" }));
    assert_eq!(found["total"], 2);
    let found = run("mail.search", &json!({ "from": "odile" }));
    assert_eq!(found["total"], 2);
}

#[test]
fn the_dates_follow_now() {
    let later = Timestamp::from_unix(now().unix_seconds() + 86_400, 0).unwrap();
    let list = session(Box::new(FakeClock::new(later))).unwrap().run("mail.list", &json!({ "limit": 1 })).unwrap();
    assert_eq!(list["messages"][0]["date"], "2026-11-06T08:35:00Z");
}

#[test]
fn a_mailbox_at_the_start_of_time_still_builds() {
    // Dates before the earliest timestamp are left out rather than refused.
    let store = mailbox(Timestamp::MIN).unwrap();
    assert_eq!(store.accounts().unwrap().len(), 2);
}
