// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `compose.*` commands: a draft is in the store after every change, from the account that last sent, with its
//! account's signature, and its recipients are checked.

use inboxcraft_model::{AccountId, DraftCopy, DraftId, FolderId, FolderState, JobKind, MessageId, SpecialUse, Uid, UidSet, UidValidity};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use crate::test_support::{NOW, account, at, folder, mailbox, message};
use crate::{AgentClass, Caller, EngineError, Session};

/// The test mailbox with a Drafts folder (6) for account 1, synced at UIDVALIDITY 7, and a signature on account 1.
pub(crate) fn store() -> Memory {
    let mut store = mailbox();
    let signed = account(1, "anna@example.org").with_signature(Some("Anna\nExample Ltd."));
    let batch = Batch::new()
        .with(Change::PutAccount(signed))
        .with(Change::PutFolder(folder(6, 1, "Drafts", vec![SpecialUse::Drafts])))
        .with(Change::SetFolderState(FolderId::new(6), FolderState::new(validity())));
    store.apply(&batch).unwrap();
    store
}

pub(crate) fn validity() -> UidValidity {
    UidValidity::new(7).unwrap()
}

pub(crate) fn session_at(store: Memory, clock: &FakeClock) -> Session {
    Session::new(Box::new(store), Box::new(clock.clone())).unwrap()
}

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (session_at(store(), &clock), clock)
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

fn draft_id(answer: &Value) -> u64 {
    answer["id"].as_u64().unwrap()
}

#[test]
fn the_classes_are_safe_for_drafts_and_owner_for_the_signature() {
    let (session, _) = session();
    for (id, class) in [
        ("compose.new", AgentClass::Safe),
        ("compose.update", AgentClass::Safe),
        ("compose.show", AgentClass::Safe),
        ("compose.list", AgentClass::Safe),
        ("compose.discard", AgentClass::Safe),
        ("compose.signature", AgentClass::Owner),
    ] {
        assert_eq!(session.registry().get(id).map(|spec| spec.class), Some(Some(class)), "{id}");
    }
}

#[test]
fn a_new_draft_is_from_the_account_that_last_sent_with_its_signature() {
    let (mut session, _) = session();
    // Account 1's Sent holds the newest sent message.
    let answer = session.run_as(Caller::Agent, "compose.new", &json!({})).unwrap();
    assert_eq!(answer["account"], json!(1));
    assert_eq!(answer["text"], json!("\n\n-- \nAnna\nExample Ltd."));
    assert_eq!(answer["saved"], json!("2026-11-05T09:00:00Z"));
    assert_eq!((answer["to"].clone(), answer["subject"].clone(), answer["answers"].clone()), (json!([]), json!(""), Value::Null));
    assert_eq!(answer["sendable"], json!(false));
    assert_eq!(answer["blocks"], json!(["no_recipient"]));
    let id = DraftId::new(draft_id(&answer));
    let kept = session.store().draft(id).unwrap().unwrap();
    assert_eq!((kept.account, kept.text.as_str()), (AccountId::new(1), "\n\n-- \nAnna\nExample Ltd."));
    // Now account 2's Sent (found by its name) holds the newest one; its account has no signature.
    let mut store = store();
    store.apply(&Batch::new().with(Change::PutMessage(message(9, 2, 5, 4, "Re: Garden party", Some(NOW - 10))))).unwrap();
    let mut session = session_at(store, &FakeClock::new(at(NOW)));
    let answer = session.run("compose.new", &json!({})).unwrap();
    assert_eq!((answer["account"].clone(), answer["text"].clone()), (json!(2), json!("")));
}

#[test]
fn without_sent_mail_the_first_account_writes_and_without_an_account_nothing_does() {
    let mut store = store();
    store.apply(&Batch::new().with(Change::RemoveMessage(MessageId::new(5)))).unwrap();
    let mut session = session_at(store, &FakeClock::new(at(NOW)));
    assert_eq!(session.run("compose.new", &json!({})).unwrap()["account"], json!(1));
    let mut empty = session_at(Memory::new(inboxcraft_store::StoreConfig::default()), &FakeClock::new(at(NOW)));
    assert!(bad(empty.run("compose.new", &json!({}))));
}

#[test]
fn new_takes_an_account_and_the_message_it_answers() {
    let (mut session, _) = session();
    let answer = session.run("compose.new", &json!({ "account": 2, "answers": 3 })).unwrap();
    assert_eq!((answer["account"].clone(), answer["answers"].clone()), (json!(2), json!(3)));
    assert_eq!(session.run("compose.new", &json!({ "account": 9 })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
    assert_eq!(session.run("compose.new", &json!({ "answers": 99 })).unwrap_err(), EngineError::NotFound { what: "message", id: 99 });
    for params in [json!({ "account": "1" }), json!({ "typo": 1 }), json!([1])] {
        assert!(bad(session.run("compose.new", &params)), "{params}");
    }
}

#[test]
fn drafts_get_ids_of_their_own_also_after_a_restart() {
    let (mut session, clock) = session();
    let first = draft_id(&session.run("compose.new", &json!({})).unwrap());
    let second = draft_id(&session.run("compose.new", &json!({})).unwrap());
    assert_ne!(first, second);
    let mut store = store();
    store.apply(&Batch::new().with(Change::PutDraft(inboxcraft_model::Draft::new(DraftId::new(41), AccountId::new(2), at(NOW))))).unwrap();
    let mut session = session_at(store, &clock);
    assert_eq!(draft_id(&session.run("compose.new", &json!({})).unwrap()), 42);
}

#[test]
fn update_keeps_every_change_at_once() {
    let (mut session, clock) = session();
    let id = draft_id(&session.run("compose.new", &json!({})).unwrap());
    clock.advance(std::time::Duration::from_secs(3));
    let answer = session
        .run_as(
            Caller::Agent,
            "compose.update",
            &json!({ "id": id, "to": ["Béla <bela@example.org>"], "cc": ["cili@example.net"], "subject": "Budget", "text": "Hello" }),
        )
        .unwrap();
    assert_eq!(answer["subject"], json!("Budget"));
    assert_eq!(answer["saved"], json!("2026-11-05T09:00:03Z"));
    assert_eq!(answer["to"], json!([{ "text": "Béla <bela@example.org>", "ok": true }]));
    assert_eq!(answer["sendable"], json!(true));
    assert_eq!(answer["blocks"], json!([]));
    let kept = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    assert_eq!(
        (kept.to, kept.cc, kept.subject.as_str(), kept.text.as_str()),
        (vec!["Béla <bela@example.org>".to_owned()], vec!["cili@example.net".to_owned()], "Budget", "Hello")
    );
    assert_eq!(kept.saved, at(NOW + 3));
    // A field left out stays as it is.
    session.run("compose.update", &json!({ "id": id, "text": "Hello again" })).unwrap();
    let kept = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    assert_eq!((kept.subject.as_str(), kept.text.as_str(), kept.cc.len()), ("Budget", "Hello again", 1));
    assert_eq!(session.run("compose.show", &json!({ "id": id })).unwrap()["text"], json!("Hello again"));
}

#[test]
fn an_impossible_address_is_marked_and_blocks_send() {
    let (mut session, _) = session();
    let id = draft_id(&session.run("compose.new", &json!({})).unwrap());
    let answer = session
        .run("compose.update", &json!({ "id": id, "to": ["bela@example.org", "not an address"], "bcc": ["\"Dóra\" <dora@@example.org>", " "] }))
        .unwrap();
    assert_eq!(
        answer["to"],
        json!([{ "text": "bela@example.org", "ok": true }, { "text": "not an address", "ok": false, "problem": "the address has no @" }])
    );
    assert_eq!(answer["bcc"][0]["ok"], json!(false));
    assert_eq!(answer["bcc"][1], json!({ "text": " ", "ok": false, "problem": "the address is empty" }));
    assert_eq!(answer["sendable"], json!(false));
    assert_eq!(answer["blocks"], json!(["bad_address"]));
    // As typed: the owner sees what they wrote.
    assert_eq!(session.store().draft(DraftId::new(id)).unwrap().unwrap().to, ["bela@example.org", "not an address"]);
    let answer = session.run("compose.update", &json!({ "id": id, "to": ["Anna <anna@example.org"], "bcc": [] })).unwrap();
    assert_eq!(answer["to"][0]["ok"], json!(false));
    let answer = session.run("compose.update", &json!({ "id": id, "to": [], "cc": ["Cili <cili@example.net>"] })).unwrap();
    assert_eq!(answer["sendable"], json!(true), "a Cc alone is a recipient");
}

#[test]
fn changing_the_account_changes_the_signature_and_takes_the_copy_back() {
    let (mut session, _) = session();
    let id = draft_id(&session.run("compose.new", &json!({})).unwrap());
    session.run("compose.update", &json!({ "id": id, "text": "Hi Bob,\n\n-- \nAnna\nExample Ltd." })).unwrap();
    let mut kept = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    kept.uploaded = Some(DraftCopy { folder: FolderId::new(6), uid_validity: validity(), uid: Uid::new(30).unwrap() });
    kept.synced = Some(kept.saved);
    kept.unnamed = true;
    session.store_mut().apply(&Batch::new().with(Change::PutDraft(kept))).unwrap();
    session.take_jobs();
    let answer = session.run("compose.update", &json!({ "id": id, "account": 2 })).unwrap();
    assert_eq!((answer["account"].clone(), answer["text"].clone()), (json!(2), json!("Hi Bob,")));
    let kept = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    assert_eq!((kept.uploaded, kept.synced, kept.unnamed), (None, None, false), "the new account has no copy yet");
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].account, AccountId::new(1));
    assert!(
        matches!(&jobs[0].kind, JobKind::Expunge { folder, uids, .. } if *folder == FolderId::new(6) && *uids == UidSet::from_iter([Uid::new(30).unwrap()]))
    );
    // Back to account 1: its signature comes back, once.
    let answer = session.run("compose.update", &json!({ "id": id, "account": 1 })).unwrap();
    assert_eq!(answer["text"], json!("Hi Bob,\n\n-- \nAnna\nExample Ltd."));
    let answer = session.run("compose.update", &json!({ "id": id, "account": 1 })).unwrap();
    assert_eq!(answer["text"], json!("Hi Bob,\n\n-- \nAnna\nExample Ltd."));
    assert_eq!(session.run("compose.update", &json!({ "id": id, "account": 9 })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
}

#[test]
fn update_refuses_what_does_not_fit() {
    let (mut session, _) = session();
    let id = draft_id(&session.run("compose.new", &json!({})).unwrap());
    assert_eq!(session.run("compose.update", &json!({ "id": 99, "text": "x" })).unwrap_err(), EngineError::NotFound { what: "draft", id: 99 });
    let many: Vec<String> = (0..=super::compose::MAX_RECIPIENTS).map(|n| format!("p{n}@example.org")).collect();
    let long_address = format!("{}@example.org", "a".repeat(super::compose::MAX_RECIPIENT_BYTES));
    for params in [
        json!({ "id": id, "to": many }),
        json!({ "id": id, "cc": [long_address] }),
        json!({ "id": id, "subject": "s".repeat(super::compose::MAX_SUBJECT_BYTES + 1) }),
        json!({ "id": id, "text": "t".repeat(super::compose::MAX_TEXT_BYTES + 1) }),
        json!({ "id": id, "to": "bela@example.org" }),
        json!({ "id": id, "typo": 1 }),
        json!({ "text": "no id" }),
    ] {
        assert!(bad(session.run("compose.update", &params)), "{}", &params.to_string().chars().take(80).collect::<String>());
    }
    assert_eq!(session.store().draft(DraftId::new(id)).unwrap().unwrap().text, "\n\n-- \nAnna\nExample Ltd.", "nothing changed");
}

#[test]
fn list_shows_the_drafts_newest_first_and_show_one() {
    let (mut session, clock) = session();
    let first = draft_id(&session.run("compose.new", &json!({ "account": 2 })).unwrap());
    clock.advance(std::time::Duration::from_secs(5));
    let second = draft_id(&session.run("compose.new", &json!({})).unwrap());
    let answer = session.run_as(Caller::Agent, "compose.list", &json!({})).unwrap();
    let ids: Vec<u64> = answer["drafts"].as_array().unwrap().iter().map(draft_id).collect();
    assert_eq!(ids, [second, first]);
    let answer = session.run("compose.list", &json!({ "account": 2 })).unwrap();
    assert_eq!(answer["drafts"].as_array().unwrap().len(), 1);
    assert_eq!(session.run("compose.list", &json!({ "account": 9 })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
    assert_eq!(draft_id(&session.run("compose.show", &json!({ "id": first })).unwrap()), first);
    assert_eq!(session.run("compose.show", &json!({ "id": 99 })).unwrap_err(), EngineError::NotFound { what: "draft", id: 99 });
}

#[test]
fn discard_removes_the_draft_and_its_copy_on_the_server() {
    let (mut session, _) = session();
    let id = draft_id(&session.run("compose.new", &json!({})).unwrap());
    let plain = draft_id(&session.run("compose.new", &json!({})).unwrap());
    let mut kept = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    kept.uploaded = Some(DraftCopy { folder: FolderId::new(6), uid_validity: validity(), uid: Uid::new(30).unwrap() });
    session.store_mut().apply(&Batch::new().with(Change::PutDraft(kept))).unwrap();
    session.take_jobs();
    assert_eq!(session.run_as(Caller::Agent, "compose.discard", &json!({ "id": id })).unwrap(), json!({ "discarded": id }));
    assert_eq!(session.store().draft(DraftId::new(id)).unwrap(), None);
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert!(matches!(&jobs[0].kind, JobKind::Expunge { folder, uid_validity, .. } if *folder == FolderId::new(6) && *uid_validity == validity()));
    assert_eq!(session.store().queued_jobs().unwrap().len(), 1, "kept until the server answers");
    session.run("compose.discard", &json!({ "id": plain })).unwrap();
    assert!(session.take_jobs().is_empty(), "a draft never uploaded leaves nothing to remove");
    assert_eq!(session.run("compose.discard", &json!({ "id": id })).unwrap_err(), EngineError::NotFound { what: "draft", id });
}

#[test]
fn a_copy_in_a_folder_that_is_gone_is_not_asked_for() {
    let (mut session, _) = session();
    let id = draft_id(&session.run("compose.new", &json!({})).unwrap());
    let mut kept = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    kept.uploaded = Some(DraftCopy { folder: FolderId::new(6), uid_validity: validity(), uid: Uid::new(30).unwrap() });
    session.store_mut().apply(&Batch::new().with(Change::PutDraft(kept)).with(Change::RemoveFolder(FolderId::new(6)))).unwrap();
    session.take_jobs();
    session.run("compose.discard", &json!({ "id": id })).unwrap();
    assert!(session.take_jobs().is_empty());
}

#[test]
fn signature_sets_and_clears_an_accounts_signature() {
    let (mut session, _) = session();
    assert!(matches!(
        session.run_as(Caller::Agent, "compose.signature", &json!({ "account": 2, "signature": "Bob" })),
        Err(EngineError::NotAllowed { .. })
    ));
    let answer = session.run("compose.signature", &json!({ "account": 2, "signature": "Bob\r\nGardener" })).unwrap();
    assert_eq!(answer, json!({ "account": 2, "signature": "Bob\nGardener" }));
    assert_eq!(session.run("compose.new", &json!({ "account": 2 })).unwrap()["text"], json!("\n\n-- \nBob\nGardener"));
    assert_eq!(session.run("compose.signature", &json!({ "account": 2 })).unwrap(), json!({ "account": 2, "signature": null }));
    assert_eq!(session.run("compose.new", &json!({ "account": 2 })).unwrap()["text"], json!(""));
    assert_eq!(session.run("compose.signature", &json!({ "account": 9 })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
    let long = "s".repeat(super::compose::MAX_SIGNATURE_BYTES + 1);
    assert!(bad(session.run("compose.signature", &json!({ "account": 2, "signature": long }))));
}

#[test]
fn a_draft_answering_a_message_shows_in_its_thread() {
    let (mut session, _) = session();
    let id = draft_id(&session.run("compose.new", &json!({ "answers": 1 })).unwrap());
    session.run("compose.new", &json!({ "answers": 3 })).unwrap();
    let answer = session.run("mail.thread", &json!({ "id": 1 })).unwrap();
    let drafts: Vec<u64> = answer["drafts"].as_array().unwrap().iter().map(draft_id).collect();
    assert_eq!(drafts, [id]);
    assert!(session.run("mail.thread", &json!({ "id": 2 })).unwrap()["drafts"].as_array().unwrap().is_empty());
}

#[test]
fn saved_times_in_answers_are_iso_8601() {
    let (mut session, clock) = session();
    clock.set(Timestamp::from_unix(NOW + 61, 500_000_000).unwrap());
    assert_eq!(session.run("compose.new", &json!({})).unwrap()["saved"], json!("2026-11-05T09:01:01Z"));
}

#[test]
fn a_signature_the_owner_changed_stays_when_the_account_changes() {
    let (mut session, _) = session();
    let id = draft_id(&session.run("compose.new", &json!({})).unwrap());
    session.run("compose.update", &json!({ "id": id, "text": "Hi,\n\n-- \nAnna, on holiday" })).unwrap();
    let answer = session.run("compose.update", &json!({ "id": id, "account": 2 })).unwrap();
    assert_eq!(answer["text"], json!("Hi,\n\n-- \nAnna, on holiday"));
}
