// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use proptest::prelude::*;
use serde_json::{Value, json};

use super::{Failure, MAX_COMMAND, MAX_LINE, answer_line, read_answer, read_request, request_line};
use crate::Token;

fn token() -> Token {
    Token::from_bytes([0x42; 32])
}

#[test]
fn a_request_line_reads_back() {
    let line = request_line(&token(), "mail.list", &json!({ "limit": 5 }));
    assert!(line.ends_with('\n'));
    assert_eq!(line.matches('\n').count(), 1);
    let request = read_request(line.as_bytes()).unwrap();
    assert_eq!(request.token, Some(token()));
    assert_eq!(request.command, "mail.list");
    assert_eq!(request.params, json!({ "limit": 5 }));
}

#[test]
fn a_request_may_leave_the_params_out_and_end_in_crlf() {
    let line = format!("{{\"token\":\"{}\",\"command\":\"account.list\"}}\r\n", token().text());
    let request = read_request(line.as_bytes()).unwrap();
    assert_eq!(request.params, json!({}));
    let request = read_request(line.trim_end().as_bytes()).unwrap();
    assert_eq!(request.command, "account.list");
}

#[test]
fn a_request_without_a_readable_token_has_none() {
    for token in [json!(null), json!(""), json!("abc"), json!(42), json!(["x"])] {
        let line = json!({ "token": token, "command": "mail.list" }).to_string();
        assert_eq!(read_request(line.as_bytes()).unwrap().token, None, "{token}");
    }
    let line = json!({ "command": "mail.list" }).to_string();
    assert_eq!(read_request(line.as_bytes()).unwrap().token, None);
}

fn refused(line: &[u8]) -> Failure {
    match read_request(line) {
        Ok(request) => panic!("read {:?}", request.command),
        Err(failure) => failure,
    }
}

#[test]
fn a_line_that_is_no_request_is_refused_as_a_bad_line() {
    let token = token().text();
    for line in [
        b"".to_vec(),
        b"\n".to_vec(),
        b"not json\n".to_vec(),
        b"\xff\xfe\n".to_vec(),
        b"[1,2]\n".to_vec(),
        b"\"text\"\n".to_vec(),
        json!({ "token": token }).to_string().into_bytes(),
        json!({ "token": token, "command": 5 }).to_string().into_bytes(),
        json!({ "token": token, "command": "" }).to_string().into_bytes(),
        json!({ "token": token, "command": "x".repeat(MAX_COMMAND + 1) }).to_string().into_bytes(),
        json!({ "token": token, "command": "mail.list", "params": [1] }).to_string().into_bytes(),
        json!({ "token": token, "command": "mail.list", "params": "x" }).to_string().into_bytes(),
        format!("{}\n{{}}", json!({ "token": token, "command": "mail.list" })).into_bytes(),
    ] {
        let failure = refused(&line);
        assert_eq!(failure.code, "bad_line", "{:?}", String::from_utf8_lossy(&line));
        assert!(!failure.message.is_empty());
    }
}

#[test]
fn a_line_longer_than_the_limit_is_refused() {
    let mut line = request_line(&token(), "mail.list", &json!({ "query": "x".repeat(MAX_LINE) })).into_bytes();
    assert_eq!(refused(&line).code, "too_long");
    line.truncate(MAX_LINE);
    assert_eq!(refused(&line).code, "bad_line");
}

#[test]
fn a_refusal_never_repeats_the_line() {
    let secret = "s3cr3t-in-the-line";
    for line in [format!("{{\"token\":\"{secret}\""), format!("{{\"token\":\"{secret}\",\"command\":7}}")] {
        let failure = refused(line.as_bytes());
        assert!(!failure.message.contains(secret), "{}", failure.message);
    }
}

#[test]
fn an_answer_line_reads_back() {
    let answer = answer_line(&Ok(json!({ "messages": [] })));
    assert!(answer.ends_with('\n'));
    assert_eq!(read_answer(answer.as_bytes()), Ok(json!({ "messages": [] })));
    let failure = Failure::new("not_allowed", "only safe commands run");
    let answer = answer_line(&Err(failure.clone()));
    assert_eq!(read_answer(answer.as_bytes()), Err(failure));
}

#[test]
fn an_answer_that_cannot_be_read_is_a_bad_answer() {
    for line in [&b""[..], b"\n", b"{}\n", b"{\"ok\":1,\"error\":{}}\n", b"{\"error\":{\"code\":1}}\n", b"{\"error\":5}\n", b"[]\n", b"\xff\n"] {
        let failure = read_answer(line).unwrap_err();
        assert_eq!(failure.code, "bad_answer", "{:?}", String::from_utf8_lossy(line));
    }
}

#[test]
fn an_error_without_a_message_still_reads() {
    let failure = read_answer(b"{\"error\":{\"code\":\"busy\"}}\n").unwrap_err();
    assert_eq!(failure, Failure::new("busy", ""));
}

#[test]
fn a_failure_shows_its_code_and_message() {
    assert_eq!(Failure::new("unauthorized", "the token is wrong").to_string(), "the token is wrong (unauthorized)");
}

proptest! {
    #[test]
    fn no_line_panics(line in proptest::collection::vec(any::<u8>(), 0..300)) {
        let _ = read_request(&line);
        let _ = read_answer(&line);
    }

    /// The fuzz target's property (`fuzz/fuzz_targets/control_lines.rs`) on lines shaped like requests.
    #[test]
    fn a_request_that_reads_writes_back(command in "\\PC{0,20}", key in "\\PC{0,8}", text in "\\PC{0,20}", token in "[0-9a-fA-F]{0,66}") {
        let line = json!({ "token": token, "command": command, "params": { key: text } }).to_string();
        if let Ok(request) = read_request(line.as_bytes()) {
            let shown = request.token.clone().unwrap_or_else(|| Token::from_bytes([0; 32]));
            let again = read_request(request_line(&shown, &request.command, &request.params).as_bytes()).unwrap();
            prop_assert_eq!(again.command, request.command);
            prop_assert_eq!(again.params, request.params);
            prop_assert_eq!(again.token, Some(shown));
        }
    }

    #[test]
    fn every_command_and_text_param_reads_back(command in "[a-z.]{1,40}", text in "\\PC{0,60}") {
        let params = json!({ "query": text });
        let request = read_request(request_line(&token(), &command, &params).as_bytes()).unwrap();
        prop_assert_eq!(request.command, command);
        prop_assert_eq!(request.params, params);
    }

    #[test]
    fn every_answer_reads_back(text in "\\PC{0,60}", number in any::<i64>()) {
        let value = json!({ "text": text, "number": number, "list": [Value::Null] });
        prop_assert_eq!(read_answer(answer_line(&Ok(value.clone())).as_bytes()), Ok(value));
    }
}
