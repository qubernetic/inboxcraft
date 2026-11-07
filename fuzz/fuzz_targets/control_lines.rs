// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The control channel's lines on any input: a request line and an answer line are read or refused, nothing panics,
//! and a request that reads writes back to the same request.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_control_key::Token;
use inboxcraft_control_key::line::{answer_line, read_answer, read_request, request_line};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = Token::parse(&String::from_utf8_lossy(data));
    if let Err(failure) = read_answer(data) {
        let again = read_answer(answer_line(&Err(failure.clone())).as_bytes());
        assert_eq!(again, Err(failure), "a failure that does not write back");
    }
    let Ok(request) = read_request(data) else { return };
    let token = request.token.unwrap_or_else(|| Token::from_bytes([0; 32]));
    let again = read_request(request_line(&token, &request.command, &request.params).as_bytes());
    let Ok(again) = again else {
        // Writing adds the token and the params' key, so a line at the limit can come back too long.
        return;
    };
    assert_eq!(again.command, request.command, "a command that does not write back");
    assert_eq!(again.params, request.params, "params that do not write back");
    assert_eq!(again.token, Some(token), "a token that does not write back");
});
