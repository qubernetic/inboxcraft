// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `compose.reply`, `compose.reply_all` and `compose.forward`: the draft goes to the right people, keeps the thread
//! (`In-Reply-To`, `References`), quotes the message under a header in our words, and a forward carries the original's
//! attachments.

use inboxcraft_model::{
    Account, AccountId, Address, EmailAddress, Envelope, FolderId, Host, JobKind, MAX_REFERENCES, Message, MessageId, MessageRef, Security, Server,
    Summary, Uid,
};
use inboxcraft_store::{Batch, Body, Change, Memory, Storage};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use super::compose_tests::{session_at, store as drafts_store};
use crate::test_support::{NOW, account, at};
use crate::{AgentClass, Caller, EngineError, Session};

/// The body of message 20: a text and two attachments, one of a media type the builder does not write.
pub(crate) const SOURCE: &str = "From: Odile Brannock <odile@example.org>\r\n\
Subject: Allotment rota\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"b\"\r\n\
\r\n\
--b\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Hi all,\r\n\
the rota is attached.\r\n\
\r\n\
> earlier\r\n\
\r\n\
--b\r\n\
Content-Type: application/pdf\r\n\
Content-Disposition: attachment; filename=\"rota.pdf\"\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
JVBERi0xLjQ=\r\n\
--b\r\n\
Content-Type: message/rfc822\r\n\
Content-Disposition: attachment; filename=\"odd.eml\"\r\n\
\r\n\
odd\r\n\
--b--\r\n";

pub(crate) fn person(name: Option<&str>, email: &str) -> Address {
    Address::new(name, EmailAddress::new(email).unwrap())
}

pub(crate) fn id(text: &str) -> MessageRef {
    MessageRef::new(text).unwrap()
}

fn kept(id: u64, folder: u64, uid: u32, envelope: Envelope) -> Message {
    let summary = Summary::new(Uid::new(uid).unwrap(), envelope).with_received(Some(at(NOW - 3000))).with_preview(Some("A short preview"));
    Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(folder), summary)
}

/// Message 20 in account 1's inbox: from Odile to Anna, Cili and Bob (the owner's other account), Dora and Anna again
/// on Cc, the third in its thread, its body kept. Message 21: from Odile with a Reply-To and a prefix already, no
/// body. Message 22: one the owner sent to Cili, with Hidden on Bcc.
pub(crate) fn store() -> Memory {
    let mut store = drafts_store();
    let submission = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let anna = account(1, "anna@example.org")
        .with_display_name(Some("Anna Example"))
        .with_signature(Some("Anna\nExample Ltd."))
        .with_submission(Some(submission));
    let rota = Envelope::new("Allotment rota")
        .with_date(Some(at(NOW - 3600)))
        .with_from(vec![person(Some("Odile Brannock"), "odile@example.org")])
        .with_to(vec![person(None, "anna@example.org"), person(Some("Cili"), "cili@example.net"), person(None, "bob@example.net")])
        .with_cc(vec![person(Some("Dora"), "dora@example.org"), person(None, "ANNA@example.org")])
        .with_message_id(Some(id("m20@example.org")))
        .with_in_reply_to(vec![id("m19@example.org")])
        .with_references(vec![id("m18@example.org"), id("m19@example.org")]);
    let again = Envelope::new("RE: Allotment rota")
        .with_from(vec![person(Some("Odile Brannock"), "odile@example.org")])
        .with_reply_to(vec![person(Some("Rota list"), "rota@example.org")])
        .with_to(vec![person(None, "anna@example.org")])
        .with_message_id(Some(id("m21@example.org")));
    let plans = Envelope::new("Plans")
        .with_date(Some(at(NOW - 600)))
        .with_from(vec![person(Some("Anna Example"), "anna@example.org")])
        .with_to(vec![person(Some("Cili"), "cili@example.net")])
        .with_bcc(vec![person(Some("Hidden"), "hidden@example.org")]);
    let batch = Batch::new()
        .with(Change::PutAccount(anna))
        .with(Change::PutMessage(kept(20, 1, 20, rota)))
        .with(Change::PutBody(MessageId::new(20), Body::new(SOURCE.as_bytes().to_vec(), "Hi all").unwrap()))
        .with(Change::PutMessage(kept(21, 1, 21, again)))
        .with(Change::PutMessage(kept(22, 2, 22, plans)));
    store.apply(&batch).unwrap();
    store
}

fn session() -> Session {
    session_at(store(), &FakeClock::new(at(NOW)))
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

const SIGNED: &str = "\n\n-- \nAnna\nExample Ltd.";

#[test]
fn reply_reply_all_and_forward_are_safe() {
    let session = session();
    for id in ["compose.reply", "compose.reply_all", "compose.forward"] {
        assert_eq!(session.registry().get(id).map(|spec| spec.class), Some(Some(AgentClass::Safe)), "{id}");
    }
}

#[test]
fn a_reply_goes_to_the_sender_in_the_thread_with_the_message_quoted() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "compose.reply", &json!({ "id": 20 })).unwrap();
    assert_eq!(answer["account"], json!(1));
    assert_eq!(answer["to"], json!([{ "text": "Odile Brannock <odile@example.org>", "ok": true }]));
    assert_eq!((answer["cc"].clone(), answer["bcc"].clone()), (json!([]), json!([])));
    assert_eq!(answer["subject"], json!("Re: Allotment rota"));
    assert_eq!(answer["answers"], json!(20));
    assert_eq!(answer["in_reply_to"], json!("m20@example.org"));
    assert_eq!(answer["references"], json!(["m18@example.org", "m19@example.org", "m20@example.org"]));
    assert_eq!(answer["forwards"], Value::Null);
    let quote = "\n\nOdile Brannock <odile@example.org> wrote on 5 Nov 2026 at 08:00:\n> Hi all,\n> the rota is attached.\n>\n>> earlier";
    assert_eq!(answer["text"], json!(format!("{quote}{SIGNED}")));
    assert_eq!(answer["sendable"], json!(true));
    // It is kept at once, and shows in the thread of the message it answers.
    let draft = session.store().draft(inboxcraft_model::DraftId::new(answer["id"].as_u64().unwrap())).unwrap().unwrap();
    assert_eq!(draft.in_reply_to, Some(id("m20@example.org")));
    let thread = session.run("mail.thread", &json!({ "id": 20 })).unwrap();
    assert_eq!(thread["drafts"][0]["id"], answer["id"]);
}

#[test]
fn a_reply_takes_the_reply_to_keeps_a_prefix_and_quotes_the_preview_without_a_body() {
    let mut session = session();
    let answer = session.run("compose.reply", &json!({ "id": 21 })).unwrap();
    assert_eq!(answer["to"], json!([{ "text": "Rota list <rota@example.org>", "ok": true }]));
    assert_eq!(answer["subject"], json!("RE: Allotment rota"));
    // Without a date the header says who wrote; without a References field the thread is the message itself.
    assert_eq!(answer["text"], json!(format!("\n\nOdile Brannock <odile@example.org> wrote:\n> A short preview{SIGNED}")));
    assert_eq!(answer["references"], json!(["m21@example.org"]));
}

#[test]
fn a_reply_to_the_owners_own_message_goes_to_its_recipients() {
    let mut session = session();
    let answer = session.run("compose.reply", &json!({ "id": 22 })).unwrap();
    assert_eq!(answer["to"], json!([{ "text": "Cili <cili@example.net>", "ok": true }]));
    // A message without a Message-ID gives the reply no thread fields.
    assert_eq!((answer["in_reply_to"].clone(), answer["references"].clone()), (Value::Null, json!([])));
    let all = session.run("compose.reply_all", &json!({ "id": 22 })).unwrap();
    assert_eq!(all["to"], json!([{ "text": "Cili <cili@example.net>", "ok": true }]));
}

#[test]
fn reply_all_leaves_out_every_address_of_the_owner() {
    let mut session = session();
    let answer = session.run("compose.reply_all", &json!({ "id": 20 })).unwrap();
    let texts = |list: &Value| list.as_array().unwrap().iter().map(|entry| entry["text"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(texts(&answer["to"]), ["Odile Brannock <odile@example.org>", "Cili <cili@example.net>"]);
    assert_eq!(texts(&answer["cc"]), ["Dora <dora@example.org>"]);
    assert_eq!(answer["subject"], json!("Re: Allotment rota"));
    assert_eq!(answer["in_reply_to"], json!("m20@example.org"));
}

#[test]
fn reply_all_leaves_out_the_login_of_an_account_too() {
    let mut session = session();
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    let login = Account::new(AccountId::new(3), "Club", EmailAddress::new("club@example.org").unwrap(), "c.ward@example.org", imap).unwrap();
    // A login that is no address is no address of the owner.
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    let plain = Account::new(AccountId::new(4), "Shop", EmailAddress::new("shop@example.org").unwrap(), "shopkeeper", imap).unwrap();
    crate::test_support::change(&mut session, Change::PutAccount(login));
    crate::test_support::change(&mut session, Change::PutAccount(plain));
    let envelope = Envelope::new("Club night")
        .with_from(vec![person(Some("Odile Brannock"), "odile@example.org")])
        .with_to(vec![person(None, "C.Ward@example.org"), person(None, "shopkeeper@example.org")])
        .with_cc(vec![person(Some("Charlie"), "c.ward@example.org"), person(Some("Dora"), "dora@example.org")]);
    crate::test_support::change(&mut session, Change::PutMessage(kept(24, 1, 24, envelope)));
    let answer = session.run("compose.reply_all", &json!({ "id": 24 })).unwrap();
    let texts = |list: &Value| list.as_array().unwrap().iter().map(|entry| entry["text"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(texts(&answer["to"]), ["Odile Brannock <odile@example.org>", "shopkeeper@example.org"]);
    assert_eq!(texts(&answer["cc"]), ["Dora <dora@example.org>"]);
}

#[test]
fn bcc_never_leaks_into_a_reply_or_a_forward() {
    let mut session = session();
    let source = "From: Anna Example <anna@example.org>\r\nTo: Cili <cili@example.net>\r\nBcc: Hidden <hidden@example.org>\r\n\
                  Subject: Plans\r\n\r\nSee you.\r\n";
    crate::test_support::change(&mut session, Change::PutBody(MessageId::new(22), Body::new(source.as_bytes().to_vec(), "See you.").unwrap()));
    for command in ["compose.reply", "compose.reply_all", "compose.forward"] {
        let answer = session.run(command, &json!({ "id": 22 })).unwrap();
        assert!(!answer.to_string().to_lowercase().contains("hidden"), "{command}: {answer}");
    }
}

#[test]
fn a_forward_quotes_the_message_inline_and_carries_its_attachments() {
    let mut session = session();
    let answer = session.run("compose.forward", &json!({ "id": 20 })).unwrap();
    assert_eq!(answer["to"], json!([]));
    assert_eq!(answer["subject"], json!("Fwd: Allotment rota"));
    assert_eq!((answer["in_reply_to"].clone(), answer["references"].clone()), (Value::Null, json!([])));
    assert_eq!(answer["forwards"], json!(20));
    assert_eq!(answer["answers"], json!(20));
    let header = "\n\nForwarded message\nFrom: Odile Brannock <odile@example.org>\nDate: 5 Nov 2026 at 08:00\nSubject: Allotment rota\n\
                  To: anna@example.org, Cili <cili@example.net>, bob@example.net\nCc: Dora <dora@example.org>, ANNA@example.org\n\n";
    assert_eq!(answer["text"], json!(format!("{header}Hi all,\nthe rota is attached.\n\n> earlier{SIGNED}")));
    assert_eq!(
        answer["attachments"],
        json!([{ "name": "rota.pdf", "type": "application/pdf", "size": 8 }, { "name": "odd.eml", "type": "application/octet-stream", "size": 3 }])
    );
    // Sent, the message carries both attachments.
    let draft = answer["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": draft, "to": ["Erik <erik@example.org>"] })).unwrap();
    session.run("mail.send", &json!({ "id": draft })).unwrap();
    let jobs = session.take_jobs();
    let raw = jobs.iter().find_map(|job| if let JobKind::Send { raw, .. } = &job.kind { Some(raw.clone()) } else { None }).unwrap();
    let message = inboxcraft_mime::parse(&raw).unwrap();
    let attached: Vec<(Option<String>, String, Vec<u8>)> = message
        .attachments()
        .iter()
        .map(|part| (part.file_name(), part.content_type().essence().to_owned(), part.decoded().unwrap().into_owned()))
        .collect();
    assert_eq!(
        attached,
        [
            (Some("rota.pdf".to_owned()), "application/pdf".to_owned(), b"%PDF-1.4".to_vec()),
            (Some("odd.eml".to_owned()), "application/octet-stream".to_owned(), b"odd".to_vec()),
        ]
    );
    assert_eq!(message.headers().get("in-reply-to"), None);
    // A message without Cc gets no Cc line.
    let plain = "From: Anna Example <anna@example.org>\r\nSubject: Plans\r\n\r\nSee you.\r\n";
    crate::test_support::change(&mut session, Change::PutBody(MessageId::new(22), Body::new(plain.as_bytes().to_vec(), "See you.").unwrap()));
    let answer = session.run("compose.forward", &json!({ "id": 22 })).unwrap();
    let header = "\n\nForwarded message\nFrom: Anna Example <anna@example.org>\nDate: 5 Nov 2026 at 08:50\nSubject: Plans\nTo: Cili <cili@example.net>\n\nSee you.";
    assert_eq!(answer["text"], json!(format!("{header}{SIGNED}")));
    assert_eq!(answer["attachments"], json!([]));
}

#[test]
fn a_forward_needs_the_body_and_its_send_needs_it_still() {
    let mut session = session();
    // Message 21's body is not kept, so its attachments are not known.
    assert!(matches!(session.run("compose.forward", &json!({ "id": 21 })), Err(EngineError::NotFound { what: "body", id: 21 })));
    let draft = session.run("compose.forward", &json!({ "id": 20 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": draft, "to": ["erik@example.org"] })).unwrap();
    crate::test_support::change(&mut session, Change::RemoveBody(MessageId::new(20)));
    // The draft tells the owner before Send.
    let shown = session.run("compose.show", &json!({ "id": draft })).unwrap();
    assert_eq!((shown["sendable"].clone(), shown["blocks"].clone()), (json!(false), json!(["forward_body"])));
    assert!(bad(session.run("mail.send", &json!({ "id": draft }))));
    assert!(session.store().outbox().unwrap().is_empty());
}

#[test]
fn a_reply_lands_in_the_original_thread() {
    let mut session = session();
    let draft = session.run("compose.reply", &json!({ "id": 20 })).unwrap()["id"].as_u64().unwrap();
    session.run("mail.send", &json!({ "id": draft })).unwrap();
    let jobs = session.take_jobs();
    let raw = jobs.iter().find_map(|job| if let JobKind::Send { raw, .. } = &job.kind { Some(raw.clone()) } else { None }).unwrap();
    let message = inboxcraft_mime::parse(&raw).unwrap();
    let headers = message.headers();
    assert_eq!(headers.get("in-reply-to"), Some("<m20@example.org>"));
    assert_eq!(headers.get("references"), Some("<m18@example.org> <m19@example.org> <m20@example.org>"));
    // The sent copy, as the next sync of the Sent folder brings it in, joins the thread of the message it answers.
    let ids = |name: &str| headers.get(name).unwrap_or_default().split_whitespace().filter_map(MessageRef::new).collect::<Vec<_>>();
    let envelope = Envelope::new("Re: Allotment rota")
        .with_date(Some(at(NOW)))
        .with_from(vec![person(Some("Anna Example"), "anna@example.org")])
        .with_message_id(ids("message-id").into_iter().next())
        .with_in_reply_to(ids("in-reply-to"))
        .with_references(ids("references"));
    crate::test_support::change(&mut session, Change::PutMessage(kept(30, 2, 30, envelope)));
    let thread = session.run("mail.thread", &json!({ "id": 20 })).unwrap();
    let members: Vec<u64> = thread["messages"].as_array().unwrap().iter().map(|message| message["id"].as_u64().unwrap()).collect();
    assert_eq!(members, [20, 30]);
}

#[test]
fn ids_the_builder_cannot_write_are_left_out_and_a_long_thread_is_cut() {
    let mut session = session();
    let mut references: Vec<MessageRef> = (0..MAX_REFERENCES).map(|n| id(&format!("r{n}@example.org"))).collect();
    references.insert(3, id("é@example.org"));
    // "Re: x" and then two bytes a character: the longest subject ends inside one.
    let envelope = Envelope::new(&format!("x{}", "é".repeat(600)))
        .with_from(vec![person(None, "odile@example.org")])
        .with_message_id(Some(id("not-an-id")))
        .with_references(references);
    crate::test_support::change(&mut session, Change::PutMessage(kept(23, 1, 23, envelope)));
    let answer = session.run("compose.reply", &json!({ "id": 23 })).unwrap();
    assert_eq!(answer["in_reply_to"], Value::Null);
    let kept: Vec<&str> = answer["references"].as_array().unwrap().iter().map(|id| id.as_str().unwrap()).collect();
    // The first id of the thread and the newest ones.
    assert_eq!(kept.len(), 20);
    assert_eq!((kept[0], kept[1], kept[19]), ("r1@example.org", "r45@example.org", "r63@example.org"));
    assert!(answer["subject"].as_str().unwrap().len() <= super::compose::MAX_SUBJECT_BYTES);
    let draft = answer["id"].as_u64().unwrap();
    assert!(session.run("mail.send", &json!({ "id": draft })).is_ok());
}

#[test]
fn reply_and_forward_refuse_what_is_not_there() {
    let mut session = session();
    for command in ["compose.reply", "compose.reply_all", "compose.forward"] {
        assert!(matches!(session.run(command, &json!({ "id": 999 })), Err(EngineError::NotFound { what: "message", .. })), "{command}");
        assert!(bad(session.run(command, &json!({}))), "{command}");
        assert!(bad(session.run(command, &json!({ "id": 20, "all": true }))), "{command}");
    }
}
