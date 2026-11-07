// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake CalDAV server on requests it does not read: it answers with a status and closes, never panics.

use inboxcraft_testkit::MAX_LINE;
use inboxcraft_testkit::dav::{FakeDav, HOME, MAX_REQUEST_BODY};

const AUTH: &str = "Authorization: Basic YWRhQGV4YW1wbGUub3JnOm5vdC1hLXJlYWwtcGFzc3dvcmQ=";

fn first_line(fake: &FakeDav, input: &[u8]) -> String {
    let out = fake.answer(input);
    String::from_utf8_lossy(&out).split("\r\n").next().unwrap_or_default().to_owned()
}

/// A PROPFIND of the home set with `body`.
fn propfind(body: &str) -> String {
    format!("PROPFIND {HOME} HTTP/1.1\r\n{AUTH}\r\nDepth: 0\r\nContent-Length: {}\r\n\r\n{body}", body.len())
}

#[test]
fn a_request_the_fake_does_not_read_is_refused_and_the_connection_closed() {
    let fake = FakeDav::new();
    let long_target = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(MAX_LINE));
    let long_field = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(MAX_LINE));
    let many_fields = format!("GET / HTTP/1.1\r\n{}\r\n", "X: 1\r\n".repeat(101));
    let too_large = format!("PUT /e.ics HTTP/1.1\r\nContent-Length: {}\r\n\r\n", MAX_REQUEST_BODY + 1);
    let cases: Vec<(String, &str)> = vec![
        ("get / HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET  / HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET / HTTP/1.1 x\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET / SPDY/3\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET x HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET /%zz HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET /%0a HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET /%ff HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET /%2 HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET / HTTP/1.1\r\nno colon\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET / HTTP/1.1\r\nBad name: x\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("GET / HTTP/1.1\r\n: x\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("PUT / HTTP/1.1\r\nContent-Length: +1\r\n\r\nx".into(), "HTTP/1.1 400 Bad Request"),
        ("PUT / HTTP/1.1\r\nContent-Length:\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
        ("PUT / HTTP/1.1\r\nContent-Length: 99999999999999999999999\r\n\r\n".into(), "HTTP/1.1 413 Content Too Large"),
        (too_large, "HTTP/1.1 413 Content Too Large"),
        ("PUT / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".into(), "HTTP/1.1 501 Not Implemented"),
        (long_target, "HTTP/1.1 414 URI Too Long"),
        (long_field, "HTTP/1.1 431 Request Header Fields Too Large"),
        (many_fields, "HTTP/1.1 431 Request Header Fields Too Large"),
        ("\r\n\r\n\r\n\r\n\r\nGET / HTTP/1.1\r\n\r\n".into(), "HTTP/1.1 400 Bad Request"),
    ];
    for (input, status) in cases {
        let out = String::from_utf8_lossy(&fake.answer(input.as_bytes())).into_owned();
        assert!(out.starts_with(&format!("{status}\r\nConnection: close\r\n")), "{input:?} gave {out:?}");
        assert_eq!(out.matches("HTTP/1.1 ").count(), 1, "{input:?}");
    }
}

#[test]
fn a_client_that_stops_inside_a_request_gets_no_answer() {
    let fake = FakeDav::new();
    for input in ["", "GET / HTTP/1.1\r\n", "GET / HTTP/1.1\r\nHost: x", "PUT /e.ics HTTP/1.1\r\nContent-Length: 10\r\n\r\nshort"] {
        assert!(fake.answer(input.as_bytes()).is_empty(), "{input:?}");
    }
}

#[test]
fn request_bodies_that_are_not_xml_it_reads_are_refused() {
    let fake = FakeDav::new();
    let deep = format!("{}{}", "<a>".repeat(33), "</a>".repeat(33));
    let wide = format!("<d:propfind xmlns:d=\"DAV:\"><d:prop>{}</d:prop></d:propfind>", "<d:x/>".repeat(10_000));
    let cases = [
        "<!DOCTYPE d:propfind><d:propfind xmlns:d=\"DAV:\"/>".to_owned(),
        "<d:propfind xmlns:d=\"DAV:\"></d:prop>".into(),
        "<d:propfind xmlns:d=\"DAV:\">".into(),
        "</d:propfind>".into(),
        "<p:propfind/>".into(),
        "<d:propfind xmlns:d=\"DAV:\" a=b/>".into(),
        "<d:propfind xmlns:d=\"DAV:\" a/>".into(),
        "<d:propfind xmlns:d=\"DAV:\" a='<'/>".into(),
        "<d:propfind xmlns:d=\"DAV:\" a='x/>".into(),
        "<d:propfind xmlns:d=\"DAV:\" 1a='x'/>".into(),
        "<d:propfind xmlns:d=\"DAV:\" xmlnsx='x'/>".into(),
        "<1d/>".into(),
        "<d:propfind xmlns:d=\"DAV:\">&bogus;</d:propfind>".into(),
        "<d:propfind xmlns:d=\"DAV:\">&#0;</d:propfind>".into(),
        "<d:propfind xmlns:d=\"DAV:\">&#xzz;</d:propfind>".into(),
        "<d:propfind xmlns:d=\"DAV:\">&amp</d:propfind>".into(),
        "<d:propfind xmlns:d=\"DAV:\"><!-- open".into(),
        "<?pi".into(),
        "<d:propfind xmlns:d=\"DAV:\"><![CDATA[x".into(),
        "text<d:propfind xmlns:d=\"DAV:\"/>".into(),
        "<![CDATA[x]]><d:propfind xmlns:d=\"DAV:\"/>".into(),
        "<d:propfind xmlns:d=\"DAV:\"/><d:propfind xmlns:d=\"DAV:\"/>".into(),
        "<d:propfind xmlns:d=\"DAV:\"/>trailing".into(),
        "<d:propfind xmlns:d=\"DAV:\"></d:propfind></d:propfind>".into(),
        "\u{ff}".into(),
        deep,
        wide,
    ];
    for body in cases {
        assert_eq!(first_line(&fake, propfind(&body).as_bytes()), "HTTP/1.1 400 Bad Request", "{body:.80}");
    }
    let mut bytes = propfind("<x/>").into_bytes();
    let at = bytes.len() - 2;
    bytes[at] = 0xff;
    assert_eq!(first_line(&fake, &bytes), "HTTP/1.1 400 Bad Request");
}

#[test]
fn request_bodies_it_reads_take_every_form_xml_allows() {
    let fake = FakeDav::new();
    let bodies = [
        "\u{feff}<?xml version='1.0'?><!-- c --><propfind xmlns=\"DAV:\"><prop><displayname/></prop></propfind>",
        "<d:propfind xmlns:d=\"DAV:\" xml:lang='en' a=\"&lt;&#65;&#x42;&quot;&apos;&gt;\"><d:prop><![CDATA[x]]><d:displayname\n/></d:prop ></d:propfind>",
        "<x:propfind xmlns:x=\"DAV:\"><x:prop><y:displayname xmlns:y=\"DAV:\"/></x:prop></x:propfind>",
        "<d:propfind xmlns:d=\"DAV:\"><d:prop xmlns:d=\"urn:other\"><d:displayname/></d:prop></d:propfind>",
        "<propfind xmlns='DAV:'><prop><displayname xmlns=''/></prop><?pi x?></propfind>",
    ];
    for body in bodies {
        let out = fake.answer(propfind(body).as_bytes());
        let out = String::from_utf8_lossy(&out);
        assert!(out.starts_with("HTTP/1.1 207 Multi-Status\r\n"), "{body}: {out}");
    }
    let named = String::from_utf8_lossy(&fake.answer(propfind(bodies[1]).as_bytes())).into_owned();
    assert!(named.contains("<d:displayname>Calendars</d:displayname>"), "{named}");
    let foreign = String::from_utf8_lossy(&fake.answer(propfind(bodies[3]).as_bytes())).into_owned();
    assert!(foreign.contains("<d:propstat><d:prop>"), "{foreign}");
}
