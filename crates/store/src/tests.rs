// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{AccountId, MessageId};

use super::{Batch, Change, MAX_QUERY_CHARS, MAX_QUERY_WORDS, Search};

#[test]
fn a_batch_keeps_its_changes_in_order() {
    let mut batch = Batch::new();
    assert!(batch.is_empty());
    batch.push(Change::RemoveMessage(MessageId::new(2)));
    let batch = batch.with(Change::RemoveAccount(AccountId::new(1)));
    assert_eq!(batch.len(), 2);
    assert!(!batch.is_empty());
    assert_eq!(batch.changes(), [Change::RemoveMessage(MessageId::new(2)), Change::RemoveAccount(AccountId::new(1))]);
}

#[test]
fn a_default_search_matches_everything() {
    assert_eq!(Search::default(), Search::new());
}

#[test]
fn search_criteria_are_capped() {
    let long = "a".repeat(MAX_QUERY_CHARS + 10);
    assert_eq!(Search::new().with_subject(&long), Search::new().with_subject(&"A".repeat(MAX_QUERY_CHARS)));
    let many = (0..MAX_QUERY_WORDS + 5).map(|n| format!("w{n}")).collect::<Vec<_>>().join(" ");
    let first = (0..MAX_QUERY_WORDS).map(|n| format!("w{n}")).collect::<Vec<_>>().join(" ");
    assert_eq!(Search::new().with_text(&many), Search::new().with_text(&first));
}

#[test]
fn a_body_prints_its_lengths_and_no_mail() {
    let body = super::Body::new(b"Subject: marmotsource\r\n\r\nx\r\n".to_vec(), "the marmottext").unwrap();
    let printed = format!("{body:?}");
    assert_eq!(printed, "Body { source_bytes: 28, text_bytes: 14 }");
    let batch = Batch::new().with(Change::PutBody(MessageId::new(1), body));
    assert!(!format!("{batch:?}").contains("marmot"), "nor does a batch that holds it");
}
