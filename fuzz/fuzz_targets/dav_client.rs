// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The CalDAV client on any bytes from the server: the response reader, then discovery and sync fed what it read.
//! Nothing panics, every request stays on its origin and its lines carry no control character, and the exchanges end
//! within a bounded number of answers.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_dav::{Credentials, Discovery, Origin, Request, Response, ResponseReader, Sync};
use libfuzzer_sys::fuzz_target;

/// How many answers one exchange is fed at most.
const ROUNDS: usize = 64;

fn check(request: &Request, origin: &Origin) {
    assert_eq!(request.origin(), origin);
    let Ok(credentials) = Credentials::new("ada@example.org", "not-a-real-password") else { return };
    let Ok(bytes) = request.encode(Some(&credentials)) else { return };
    let head_end = bytes.windows(4).position(|window| window == b"\r\n\r\n").unwrap_or(bytes.len());
    let head = bytes.get(..head_end).unwrap_or_default();
    assert!(head.split(|&byte| byte == b'\n').all(|line| !line.strip_suffix(b"\r").unwrap_or(line).iter().any(|&byte| byte < 0x20)));
}

fuzz_target!(|data: &[u8]| {
    let mut reader = ResponseReader::new();
    let mut responses: Vec<Response> = Vec::new();
    let mut input = data;
    while let Ok(Some(response)) = reader.receive(input) {
        responses.push(response);
        input = &[];
        if responses.len() >= ROUNDS {
            break;
        }
    }
    if let Ok(Some(last)) = reader.end() {
        responses.push(last);
    }
    let Ok(mut discovery) = Discovery::new("https://dav.example.org") else { return };
    let origin = discovery.origin().clone();
    for response in &responses {
        let Ok(request) = discovery.request() else { break };
        check(&request, &origin);
        if !matches!(discovery.receive(response), Ok(None)) {
            break;
        }
    }
    let known = [("/calendars/ada/work/a.ics".to_owned(), "\"1\"".to_owned())];
    let Ok(mut sync) = Sync::new(&origin, "/calendars/ada/work/", Some("t1"), known) else { return };
    for response in responses.iter().cycle().take(ROUNDS) {
        let Ok(request) = sync.request() else { break };
        check(&request, &origin);
        if !matches!(sync.receive(response), Ok(None)) {
            break;
        }
    }
});
