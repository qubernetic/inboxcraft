// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the reader asks for: the HTML part of a body, an attachment's content, a thread, and the senders whose
//! remote pictures load without asking.

use serde_json::{Value, json};

use crate::test_support::{NOW, at, mailbox, session, session_with};
use crate::{Caller, EngineError, Registry, Session};

fn put_body(session: &mut Session, id: u64, raw: &[u8]) {
    let uid = session.store().message(inboxcraft_model::MessageId::new(id)).unwrap().unwrap().message().summary().uid();
    let fetched = inboxcraft_model::EventKind::BodyFetched {
        folder: inboxcraft_model::FolderId::new(1),
        uid_validity: inboxcraft_model::UidValidity::new(1).unwrap(),
        uid,
        raw: raw.to_vec(),
    };
    session.apply(&inboxcraft_model::Event::new(inboxcraft_model::AccountId::new(1), None, fetched)).unwrap();
}

/// A newsletter: text and HTML, an inline picture by Content-ID and an attached file.
const NEWSLETTER: &[u8] = b"Subject: News\r\n\
Content-Type: multipart/mixed; boundary=outer\r\n\
\r\n\
--outer\r\n\
Content-Type: multipart/related; boundary=inner\r\n\
\r\n\
--inner\r\n\
Content-Type: multipart/alternative; boundary=alt\r\n\
\r\n\
--alt\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Plain words.\r\n\
--alt\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p>Rich <b>words</b><img src=\"cid:logo@example.org\"><img src=\"https://pictures.example.org/p.png\"></p>\r\n\
--alt--\r\n\
--inner\r\n\
Content-Type: image/png\r\n\
Content-ID: <logo@example.org>\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
iVBORw==\r\n\
--inner--\r\n\
--outer\r\n\
Content-Type: text/csv; name=\"rota.csv\"\r\n\
Content-Disposition: attachment; filename=\"rota.csv\"\r\n\
\r\n\
week,name\r\n\
--outer--\r\n";

#[test]
fn mail_read_gives_the_html_part_as_text_and_numbers_the_attachments() {
    let mut session = session();
    put_body(&mut session, 1, NEWSLETTER);
    let answer = session.run_as(Caller::Agent, "mail.read", &json!({ "id": 1 })).unwrap();
    let body = &answer["message"]["body"];
    assert_eq!(body["text"], json!("Plain words."));
    assert_eq!(body["has_html"], json!(true));
    assert_eq!(body["html"], json!("<p>Rich <b>words</b><img src=\"cid:logo@example.org\"><img src=\"https://pictures.example.org/p.png\"></p>"));
    assert_eq!(body["attachments"], json!([{ "index": 0, "name": "rota.csv", "type": "text/csv", "size": 9, "content_id": null }]));
    assert_eq!(body["inline"], json!([{ "content_id": "logo@example.org", "type": "image/png", "size": 4 }]));
    // Reading names the remote picture as text and fetches nothing.
    assert!(session.take_jobs().is_empty());
}

#[test]
fn mail_attachment_gives_a_file_or_an_inline_part_in_base64() {
    let mut session = session();
    put_body(&mut session, 1, NEWSLETTER);
    let file = session.run_as(Caller::Agent, "mail.attachment", &json!({ "id": 1, "index": 0 })).unwrap();
    assert_eq!(file, json!({ "name": "rota.csv", "type": "text/csv", "size": 9, "data": "d2VlayxuYW1l" }));
    let inline = session.run("mail.attachment", &json!({ "id": 1, "content_id": "logo@example.org" })).unwrap();
    assert_eq!(inline, json!({ "name": null, "type": "image/png", "size": 4, "data": "iVBORw==" }));
}

#[test]
fn mail_attachment_refuses_what_is_not_there() {
    let mut session = session();
    assert_eq!(session.run("mail.attachment", &json!({ "id": 99, "index": 0 })).unwrap_err(), EngineError::NotFound { what: "message", id: 99 });
    assert_eq!(session.run("mail.attachment", &json!({ "id": 1, "index": 0 })).unwrap_err(), EngineError::NotFound { what: "body", id: 1 });
    put_body(&mut session, 1, NEWSLETTER);
    assert_eq!(session.run("mail.attachment", &json!({ "id": 1, "index": 1 })).unwrap_err(), EngineError::NotFound { what: "attachment", id: 1 });
    let missing = session.run("mail.attachment", &json!({ "id": 1, "content_id": "other@example.org" })).unwrap_err();
    assert_eq!(missing.code(), "not_found");
    put_body(&mut session, 2, b"Content-Type: multipart/mixed\r\n\r\nno boundary");
    assert_eq!(session.run("mail.attachment", &json!({ "id": 2, "index": 0 })).unwrap_err(), EngineError::NotFound { what: "attachment", id: 0 });
    for params in
        [json!({ "id": 1 }), json!({ "id": 1, "index": 0, "content_id": "x" }), json!({ "id": 1, "index": -1 }), json!({ "id": 1, "name": "x" })]
    {
        assert_eq!(session.run("mail.attachment", &params).unwrap_err().code(), "bad_params", "{params}");
    }
}

/// A message of the test mailbox with its `Message-ID` and the one it answers.
fn linked(id: u64, folder: u64, uid: u32, subject: &str, received: i64, message_id: &str, answers: Option<&str>) -> inboxcraft_model::Message {
    use inboxcraft_model::{AccountId, Envelope, FolderId, Message, MessageId, MessageRef, Summary, Uid};
    let account = if folder >= 4 { 2 } else { 1 };
    let envelope = Envelope::new(subject)
        .with_from(vec![crate::test_support::address("Anna Example", "anna@example.org")])
        .with_message_id(MessageRef::new(message_id))
        .with_in_reply_to(answers.and_then(MessageRef::new).into_iter().collect());
    let summary = Summary::new(Uid::new(uid).unwrap(), envelope).with_received(Some(at(received)));
    Message::new(MessageId::new(id), AccountId::new(account), FolderId::new(folder), summary)
}

fn threaded() -> Session {
    use inboxcraft_store::{Batch, Change, Storage};
    let mut store = mailbox();
    let batch = Batch::new()
        .with(Change::PutMessage(linked(1, 1, 10, "Budget for 2026", NOW - 3600, "budget@example.org", None)))
        .with(Change::PutMessage(linked(5, 2, 3, "Re: Budget for 2026", NOW - 1800, "re1@example.org", Some("budget@example.org"))))
        .with(Change::PutMessage(linked(3, 4, 7, "Re: Re: Budget for 2026", NOW - 600, "re2@example.org", Some("re1@example.org"))));
    store.apply(&batch).unwrap();
    session_with(store, Registry::builtin().unwrap())
}

fn subjects(answer: &Value) -> Vec<String> {
    answer["messages"].as_array().unwrap().iter().map(|m| m["subject"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn mail_thread_lists_the_thread_oldest_first_across_folders_and_accounts() {
    let mut session = threaded();
    let answer = session.run_as(Caller::Agent, "mail.thread", &json!({ "id": 5 })).unwrap();
    assert_eq!(subjects(&answer), ["Budget for 2026", "Re: Budget for 2026", "Re: Re: Budget for 2026"]);
    assert_eq!(answer["total"], json!(3));
    assert_eq!(answer["messages"][2]["account"], json!(2));
    assert_eq!(session.run("mail.thread", &json!({ "id": 1 })).unwrap()["messages"], answer["messages"]);
    // A message that names nothing is a thread of one.
    assert_eq!(subjects(&session.run("mail.thread", &json!({ "id": 2 })).unwrap()), ["Lunch on Friday"]);
    assert!(session.take_jobs().is_empty());
}

#[test]
fn mail_thread_refuses_what_does_not_fit() {
    let mut session = session();
    assert_eq!(session.run("mail.thread", &json!({ "id": 99 })).unwrap_err(), EngineError::NotFound { what: "message", id: 99 });
    for params in [json!({}), json!({ "id": "1" }), json!({ "id": 1, "limit": 3 })] {
        assert_eq!(session.run("mail.thread", &params).unwrap_err().code(), "bad_params", "{params}");
    }
}

#[test]
fn remote_pictures_of_a_sender_load_once_the_owner_allows_them_always() {
    let mut session = session();
    let allowed = |session: &mut Session, id: u64| session.run("mail.read", &json!({ "id": id })).unwrap()["remote_allowed"].clone();
    assert_eq!(allowed(&mut session, 1), json!(false), "blocked until the owner says otherwise");
    let answer = session.run("remote.allow", &json!({ "address": "ANNA@Example.org" })).unwrap();
    assert_eq!(answer, json!({ "address": "anna@example.org", "allowed": true }));
    assert_eq!(allowed(&mut session, 1), json!(true));
    assert_eq!(allowed(&mut session, 3), json!(true), "the same sender in another account");
    session.run("remote.allow", &json!({ "address": "anna@example.org" })).unwrap();
    assert_eq!(session.run("remote.list", &json!({})).unwrap(), json!({ "senders": ["anna@example.org"] }));
    let answer = session.run("remote.forget", &json!({ "address": "anna@example.org" })).unwrap();
    assert_eq!(answer, json!({ "address": "anna@example.org", "allowed": false }));
    assert_eq!(allowed(&mut session, 1), json!(false));
    assert_eq!(session.run("remote.list", &json!({})).unwrap(), json!({ "senders": [] }));
    session.run("remote.forget", &json!({ "address": "anna@example.org" })).unwrap();
    assert!(session.take_jobs().is_empty(), "allowing fetches nothing by itself");
}

#[test]
fn only_the_owner_allows_remote_pictures() {
    let mut session = session();
    for id in ["remote.allow", "remote.forget"] {
        let refused = session.run_as(Caller::Agent, id, &json!({ "address": "anna@example.org" })).unwrap_err();
        assert_eq!(refused.code(), "not_allowed", "{id}");
    }
    assert_eq!(session.run("mail.read", &json!({ "id": 1 })).unwrap()["remote_allowed"], json!(false));
    assert_eq!(session.run_as(Caller::Agent, "remote.list", &json!({})).unwrap(), json!({ "senders": [] }));
}

#[test]
fn remote_allow_refuses_what_is_no_address() {
    let mut session = session();
    for params in [json!({}), json!({ "address": "no at sign" }), json!({ "address": 3 }), json!({ "address": "a@example.org", "x": 1 })] {
        assert_eq!(session.run("remote.allow", &params).unwrap_err().code(), "bad_params", "{params}");
        assert_eq!(session.run("remote.forget", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert_eq!(session.run("remote.list", &json!({ "x": 1 })).unwrap_err().code(), "bad_params");
}

#[test]
fn a_message_without_a_sender_never_has_remote_pictures_allowed() {
    use inboxcraft_store::{Batch, Change, Storage};
    let mut store = mailbox();
    let nobody = inboxcraft_model::Message::new(
        inboxcraft_model::MessageId::new(9),
        inboxcraft_model::AccountId::new(1),
        inboxcraft_model::FolderId::new(1),
        inboxcraft_model::Summary::new(inboxcraft_model::Uid::new(30).unwrap(), inboxcraft_model::Envelope::new("No sender")),
    );
    store.apply(&Batch::new().with(Change::PutMessage(nobody))).unwrap();
    let mut session = session_with(store, Registry::builtin().unwrap());
    session.run("remote.allow", &json!({ "address": "anna@example.org" })).unwrap();
    assert_eq!(session.run("mail.read", &json!({ "id": 9 })).unwrap()["remote_allowed"], json!(false));
}
