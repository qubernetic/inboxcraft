// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the reader asks for: the HTML part of a body, an attachment's content, a thread, and the senders whose
//! remote pictures load without asking.

use serde_json::json;

use crate::test_support::session;
use crate::{Caller, EngineError, Session};

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
