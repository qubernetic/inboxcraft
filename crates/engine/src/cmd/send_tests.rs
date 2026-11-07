// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `mail.send`: a draft goes out with its own `Message-ID` and `Date`, Bcc only in the envelope; the answer from the
//! worker removes the draft and appends the Sent copy, or keeps it with the server's reply.

use inboxcraft_model::{
    AccountId, DraftCopy, DraftId, EmailAddress, ErrorKind, Event, EventKind, Flag, FolderId, Host, JobKind, OutboxState, Security, Server, Uid,
    UidSet,
};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::{FakeClock, Offset};
use serde_json::{Value, json};

use super::compose_tests::{store as drafts_store, validity};
use crate::test_support::{NOW, account, at};
use crate::{AgentClass, Caller, EngineError, Session};

/// The compose test mailbox, account 1 with a submission server on 465.
fn store() -> Memory {
    let mut store = drafts_store();
    let submission = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let sending = account(1, "anna@example.org").with_display_name(Some("Anna Example")).with_submission(Some(submission));
    store.apply(&Batch::new().with(Change::PutAccount(sending))).unwrap();
    store
}

fn session() -> Session {
    Session::new(Box::new(store()), Box::new(FakeClock::new(at(NOW)))).unwrap()
}

/// A draft of account 1 to Béla, Cili on Cc and Dóra on Bcc; its id.
fn draft(session: &mut Session) -> u64 {
    let id = session.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
    let fields = json!({
        "id": id,
        "to": ["Béla <bela@example.org>"],
        "cc": ["cili@example.net"],
        "bcc": ["Dora <dora@example.org>", "BELA@example.org"],
        "subject": "Budget",
        "text": "The numbers are in.",
    });
    session.run("compose.update", &fields).unwrap();
    session.take_jobs();
    id
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

fn sent_job(session: &mut Session) -> (DraftId, EmailAddress, Vec<EmailAddress>, Vec<u8>) {
    let jobs = session.take_jobs();
    let [job] = jobs.as_slice() else { panic!("{jobs:?}") };
    assert_eq!(job.account, AccountId::new(1));
    let JobKind::Send { draft, from, recipients, raw } = &job.kind else { panic!("{job:?}") };
    (*draft, from.clone(), recipients.clone(), raw.clone())
}

fn event(kind: EventKind) -> Event {
    Event::new(AccountId::new(1), None, kind)
}

#[test]
fn sending_is_for_the_owner() {
    let mut session = session();
    assert_eq!(session.registry().get("mail.send").map(|spec| spec.class), Some(Some(AgentClass::Owner)));
    let id = draft(&mut session);
    assert!(matches!(session.run_as(Caller::Agent, "mail.send", &json!({ "id": id })), Err(EngineError::NotAllowed { .. })));
    assert!(session.take_jobs().is_empty());
}

#[test]
fn a_draft_goes_out_with_its_own_message_id_and_date_and_bcc_only_in_the_envelope() {
    let mut session = session();
    session.set_offsets(Box::new(Offset::from_minutes(60).unwrap()));
    let id = draft(&mut session);
    let answer = session.run("mail.send", &json!({ "id": id })).unwrap();
    assert_eq!(answer["id"], json!(id));
    assert_eq!(answer["state"], json!("sending"));
    assert_eq!(answer["recipients"], json!(3));
    let message_id = answer["message_id"].as_str().unwrap().to_owned();
    assert!(message_id.starts_with('<') && message_id.ends_with("@example.org>"), "{message_id}");
    let (draft, from, recipients, raw) = sent_job(&mut session);
    assert_eq!(draft, DraftId::new(id));
    assert_eq!(from.as_str(), "anna@example.org");
    // To, Cc and Bcc, each address once.
    let to: Vec<&str> = recipients.iter().map(EmailAddress::as_str).collect();
    assert_eq!(to, ["bela@example.org", "cili@example.net", "dora@example.org"]);
    let text = String::from_utf8_lossy(&raw);
    assert!(text.contains("Date: Thu, 5 Nov 2026 10:00:00 +0100\r\n"), "{text}");
    assert!(text.contains(&format!("Message-ID: {message_id}\r\n")), "{text}");
    assert!(text.contains("From: \"Anna Example\" <anna@example.org>\r\n"), "{text}");
    assert!(text.contains("Cc: cili@example.net\r\n"), "{text}");
    assert!(text.contains("Subject: Budget\r\n"), "{text}");
    assert!(!text.to_ascii_lowercase().contains("bcc") && !text.contains("dora"), "{text}");
    // The outbox holds it, with the message for the Sent copy.
    let outbox = session.store().outbox().unwrap();
    assert_eq!(outbox.len(), 1);
    assert_eq!((outbox[0].draft, &outbox[0].state, outbox[0].message.as_slice()), (DraftId::new(id), &OutboxState::Sending, raw.as_slice()));
    // A second send of the same draft while it is on its way starts nothing.
    assert!(bad(session.run("mail.send", &json!({ "id": id }))));
    assert!(session.take_jobs().is_empty());
}

#[test]
fn two_sends_get_two_message_ids() {
    let mut session = session();
    let first = draft(&mut session);
    let second = draft(&mut session);
    let a = session.run("mail.send", &json!({ "id": first })).unwrap()["message_id"].clone();
    let b = session.run("mail.send", &json!({ "id": second })).unwrap()["message_id"].clone();
    assert_ne!(a, b);
}

#[test]
fn a_draft_that_cannot_go_is_refused() {
    let mut session = session();
    // No recipient.
    let empty = session.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
    assert!(bad(session.run("mail.send", &json!({ "id": empty }))));
    // An impossible address.
    session.run("compose.update", &json!({ "id": empty, "to": ["not an address"] })).unwrap();
    assert!(bad(session.run("mail.send", &json!({ "id": empty }))));
    // An account without a submission server.
    let other = session.run("compose.new", &json!({ "account": 2 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": other, "to": ["anna@example.org"] })).unwrap();
    assert!(bad(session.run("mail.send", &json!({ "id": other }))));
    // No such draft, and junk.
    assert!(matches!(session.run("mail.send", &json!({ "id": 999 })), Err(EngineError::NotFound { .. })));
    assert!(bad(session.run("mail.send", &json!({ "id": "1" }))));
    assert!(bad(session.run("mail.send", &json!({ "id": 1, "to": "x" }))));
    session.take_jobs();
    assert!(session.store().outbox().unwrap().is_empty());
}

#[test]
fn a_sent_draft_goes_and_its_copy_lands_in_sent() {
    let mut session = session();
    let id = draft(&mut session);
    // The draft has a copy in the Drafts folder.
    let copy = DraftCopy { folder: FolderId::new(6), uid_validity: validity(), uid: Uid::new(40).unwrap() };
    let held = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    crate::test_support::change(&mut session, Change::PutDraft(inboxcraft_model::Draft { uploaded: Some(copy), ..held }));
    session.run("mail.send", &json!({ "id": id })).unwrap();
    let (_, _, _, raw) = sent_job(&mut session);
    session.apply(&event(EventKind::Sent { draft: DraftId::new(id) })).unwrap();
    assert_eq!(session.store().draft(DraftId::new(id)).unwrap(), None);
    assert!(session.store().outbox().unwrap().is_empty());
    let jobs = session.take_jobs();
    let kinds: Vec<&JobKind> = jobs.iter().map(|job| &job.kind).collect();
    let append = kinds.iter().find_map(|kind| match kind {
        JobKind::Append { folder, flags, raw, draft, .. } => Some((*folder, flags.contains(&Flag::Seen), raw.clone(), *draft)),
        _ => None,
    });
    assert_eq!(append, Some((FolderId::new(2), true, raw, None)));
    let expunge = kinds.iter().find_map(|kind| match kind {
        JobKind::Expunge { folder, uids, .. } => Some((*folder, uids.clone())),
        _ => None,
    });
    assert_eq!(expunge, Some((FolderId::new(6), std::iter::once(Uid::new(40).unwrap()).collect::<UidSet>())));
    // Both are kept until they end, so a restart runs them again.
    assert_eq!(session.store().queued_jobs().unwrap().len(), 2);
}

#[test]
fn a_failed_send_keeps_the_draft_with_the_reason() {
    let mut session = session();
    let id = draft(&mut session);
    session.run("mail.send", &json!({ "id": id })).unwrap();
    session.take_jobs();
    let refused =
        EventKind::SendFailed { draft: DraftId::new(id), kind: ErrorKind::MessageRefused, reply: Some("550 5.1.1 No such user".to_owned()) };
    session.apply(&event(refused)).unwrap();
    assert!(session.store().draft(DraftId::new(id)).unwrap().is_some());
    let outbox = session.store().outbox().unwrap();
    assert_eq!(outbox[0].state, OutboxState::Refused { reply: "550 5.1.1 No such user".to_owned() });
    assert!(session.take_jobs().is_empty());
    // It can be sent again.
    session.run("mail.send", &json!({ "id": id })).unwrap();
    assert_eq!(sent_job(&mut session).0, DraftId::new(id));
    // A failure without a reply says its cause.
    session.apply(&event(EventKind::SendFailed { draft: DraftId::new(id), kind: ErrorKind::TimedOut, reply: None })).unwrap();
    let outbox = session.store().outbox().unwrap();
    assert_eq!(outbox[0].state, OutboxState::Refused { reply: ErrorKind::TimedOut.to_string() });
}

#[test]
fn an_answer_for_a_draft_not_on_its_way_is_dropped() {
    let mut session = session();
    let id = draft(&mut session);
    for kind in [
        EventKind::Sent { draft: DraftId::new(id) },
        EventKind::SendFailed { draft: DraftId::new(id), kind: ErrorKind::TimedOut, reply: None },
        EventKind::Sent { draft: DraftId::new(999) },
    ] {
        assert_eq!(session.apply(&event(kind)).unwrap(), crate::Applied::Dropped);
    }
    assert!(session.store().draft(DraftId::new(id)).unwrap().is_some());
    // From another account's worker.
    session.run("mail.send", &json!({ "id": id })).unwrap();
    let other = Event::new(AccountId::new(2), None, EventKind::Sent { draft: DraftId::new(id) });
    assert_eq!(session.apply(&other).unwrap(), crate::Applied::Dropped);
    assert!(session.store().draft(DraftId::new(id)).unwrap().is_some());
}

#[test]
fn an_account_without_a_sent_folder_sends_without_a_copy() {
    let mut store = store();
    store.apply(&Batch::new().with(Change::RemoveFolder(FolderId::new(2)))).unwrap();
    let mut session = Session::new(Box::new(store), Box::new(FakeClock::new(at(NOW)))).unwrap();
    let id = draft(&mut session);
    session.run("mail.send", &json!({ "id": id })).unwrap();
    session.take_jobs();
    session.apply(&event(EventKind::Sent { draft: DraftId::new(id) })).unwrap();
    assert_eq!(session.store().draft(DraftId::new(id)).unwrap(), None);
    assert!(session.take_jobs().is_empty());
}

#[test]
fn a_send_cut_off_by_a_restart_can_be_sent_again() {
    let mut session = session();
    let id = draft(&mut session);
    session.run("mail.send", &json!({ "id": id })).unwrap();
    session.take_jobs();
    // The program stops before the worker answers; the store is all that is left.
    let outbox = session.store().outbox().unwrap();
    let mut store = store();
    let held = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    store.apply(&Batch::new().with(Change::PutDraft(held)).with(Change::PutOutgoing(outbox[0].clone()))).unwrap();
    let mut again = Session::new(Box::new(store), Box::new(FakeClock::new(at(NOW)))).unwrap();
    let outbox = again.store().outbox().unwrap();
    assert_eq!(outbox[0].state, OutboxState::Refused { reply: crate::send::CUT_OFF.to_owned() });
    assert!(!outbox[0].message.is_empty());
    again.run("mail.send", &json!({ "id": id })).unwrap();
    assert_eq!(sent_job(&mut again).0, DraftId::new(id));
}
