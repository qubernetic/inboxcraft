// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Deleted, MAX_PUT, Precondition, Written, delete, delete_outcome, put, put_outcome};
use crate::http::{Method, Response};
use crate::{DavError, Origin};

fn origin() -> Origin {
    Origin::parse("https://dav.example.org/").unwrap().0
}

const EVENT: &str = "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n";

#[test]
fn a_put_replaces_only_what_it_last_saw() {
    let request = put(&origin(), "/cal/work/e1.ics", EVENT, &Precondition::Replace("\"7\"".into())).unwrap();
    assert_eq!((request.method(), request.path()), (Method::Put, "/cal/work/e1.ics"));
    assert_eq!((request.header("If-Match"), request.header("If-None-Match")), (Some("\"7\""), None));
    assert_eq!(request.header("Content-Type"), Some("text/calendar; charset=utf-8"));
    assert_eq!(request.body(), EVENT.as_bytes());
    let created = put(&origin(), "e2.ics", EVENT, &Precondition::Create).unwrap();
    assert_eq!((created.path(), created.header("If-None-Match"), created.header("If-Match")), ("/e2.ics", Some("*"), None));
}

#[test]
fn what_a_put_cannot_send_is_refused() {
    let bad = Precondition::Replace("\"7\"\r\nX: y".into());
    assert_eq!(put(&origin(), "/e.ics", EVENT, &bad), Err(DavError::BadArgument("an entity tag that cannot be sent")));
    assert_eq!(
        put(&origin(), "/e.ics", EVENT, &Precondition::Replace(String::new())),
        Err(DavError::BadArgument("an entity tag that cannot be sent"))
    );
    assert_eq!(put(&origin(), "/e.ics", EVENT, &Precondition::Replace(" \"1\" ".into())).unwrap().header("If-Match"), Some("\"1\""));
    let away = put(&origin(), "https://other.example.org/e.ics", EVENT, &Precondition::Create);
    assert_eq!(away, Err(DavError::OtherOrigin { url: "https://other.example.org/e.ics".into() }));
    let large = "x".repeat(MAX_PUT + 1);
    assert_eq!(put(&origin(), "/e.ics", &large, &Precondition::Create), Err(DavError::TooLarge { what: "calendar to write", limit: MAX_PUT }));
}

#[test]
fn how_a_put_ended() {
    assert_eq!(put_outcome(&Response::new(201).with_header("ETag", "\"2\"")), Ok(Written::Stored { etag: Some("\"2\"".into()) }));
    assert_eq!(put_outcome(&Response::new(204)), Ok(Written::Stored { etag: None }));
    assert_eq!(put_outcome(&Response::new(200).with_header("ETag", "a\u{1}b")), Ok(Written::Stored { etag: None }));
    assert_eq!(put_outcome(&Response::new(412)), Ok(Written::Conflict));
    assert_eq!(put_outcome(&Response::new(401)), Err(DavError::Unauthorized));
    assert_eq!(put_outcome(&Response::new(403)), Err(DavError::Refused { status: 403 }));
    assert_eq!(put_outcome(&Response::new(302)), Err(DavError::Refused { status: 302 }));
}

#[test]
fn a_delete_removes_only_what_it_last_saw() {
    let request = delete(&origin(), "/cal/work/e1.ics", Some("\"7\"")).unwrap();
    assert_eq!((request.method(), request.header("If-Match"), request.body()), (Method::Delete, Some("\"7\""), &b""[..]));
    assert_eq!(delete(&origin(), "/e.ics", None).unwrap().header("If-Match"), None);
    assert_eq!(delete(&origin(), "/e.ics", Some("\n")), Err(DavError::BadArgument("an entity tag that cannot be sent")));
    assert_eq!(delete(&origin(), "ftp://x/", None), Err(DavError::BadUrl("a scheme other than https or http")));
}

#[test]
fn how_a_delete_ended() {
    assert_eq!(delete_outcome(&Response::new(204)), Ok(Deleted::Removed));
    assert_eq!(delete_outcome(&Response::new(200)), Ok(Deleted::Removed));
    assert_eq!(delete_outcome(&Response::new(404)), Ok(Deleted::Gone));
    assert_eq!(delete_outcome(&Response::new(410)), Ok(Deleted::Gone));
    assert_eq!(delete_outcome(&Response::new(412)), Ok(Deleted::Conflict));
    assert_eq!(delete_outcome(&Response::new(401)), Err(DavError::Unauthorized));
    assert_eq!(delete_outcome(&Response::new(423)), Err(DavError::Refused { status: 423 }));
}
