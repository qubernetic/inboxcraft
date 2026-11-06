// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Response, classify};
use crate::ProtoError;
use crate::imap::value::{Piece, Status, StatusLine, Value};

fn line(s: &str) -> Result<Response, ProtoError> {
    classify(vec![Piece::Text(s.as_bytes().to_vec())])
}

#[test]
fn the_four_kinds_of_response() {
    assert_eq!(line("+ go ahead"), Ok(Response::Continue));
    assert_eq!(line("+"), Ok(Response::Continue));
    assert_eq!(line("* OK [ALERT] hello"), Ok(Response::Status(StatusLine::new(Status::Ok, "[ALERT] hello"))));
    assert_eq!(line("* bye"), Ok(Response::Status(StatusLine::new(Status::Bye, ""))));
    assert_eq!(
        line("A0001 no [AUTHENTICATIONFAILED] wrong"),
        Ok(Response::Tagged { tag: "A0001".into(), line: StatusLine::new(Status::No, "[AUTHENTICATIONFAILED] wrong") })
    );
    assert_eq!(line("* 3 EXISTS"), Ok(Response::Data(vec![Value::Atom("3".into()), Value::Atom("EXISTS".into())])));
    assert_eq!(line("* "), Ok(Response::Data(vec![])));
    let pieces = vec![Piece::Text(b"* 1 FETCH (BODY[] {1}".to_vec()), Piece::Literal(b"x".to_vec()), Piece::Text(b")".to_vec())];
    assert!(matches!(classify(pieces), Ok(Response::Data(values)) if values.len() == 3));
}

#[test]
fn what_is_no_response_is_malformed() {
    assert_eq!(classify(vec![]), Err(ProtoError::Malformed("response")));
    assert_eq!(classify(vec![Piece::Literal(b"x".to_vec())]), Err(ProtoError::Malformed("response")));
    for tag in ["", " OK", "*", "* ", "A(1 OK", "A+ OK", "a\"b OK", "A{1} OK"] {
        let response = line(tag);
        assert!(matches!(response, Err(ProtoError::Malformed(_)) | Ok(Response::Data(_))), "{tag:?}: {response:?}");
    }
    assert_eq!(line("A0001 BYE now"), Err(ProtoError::Malformed("tagged response")));
    assert_eq!(line("A0001 PREAUTH"), Err(ProtoError::Malformed("tagged response")));
    assert_eq!(line("A0001 FETCH"), Err(ProtoError::Malformed("tagged response")));
    assert_eq!(line("A0001"), Err(ProtoError::Malformed("tagged response")));
    assert_eq!(line("*"), Err(ProtoError::Malformed("tag")));
    assert_eq!(line(""), Err(ProtoError::Malformed("tag")));
}
