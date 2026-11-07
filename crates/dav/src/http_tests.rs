// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_BODY, MAX_HEAD, MAX_HEADERS, Method, Request, Response, ResponseReader};
use crate::{Credentials, DavError, Origin};

fn origin(url: &str) -> Origin {
    Origin::parse(url).unwrap().0
}

fn read_all(bytes: &[u8]) -> Result<Option<Response>, DavError> {
    ResponseReader::new().receive(bytes)
}

#[test]
fn a_request_is_written_with_host_headers_length_and_body() {
    let request = Request::new(Method::Propfind, &origin("https://dav.example.org:8443/"), "/cal/")
        .with_header("Depth", "0")
        .with_body("application/xml; charset=utf-8", b"<x/>".to_vec());
    assert_eq!((request.method(), request.path(), request.body()), (Method::Propfind, "/cal/", &b"<x/>"[..]));
    assert_eq!(request.header("depth"), Some("0"));
    assert_eq!(request.header("If-Match"), None);
    assert_eq!(request.origin().to_string(), "https://dav.example.org:8443");
    let bytes = request.encode(None).unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "PROPFIND /cal/ HTTP/1.1\r\nHost: dav.example.org:8443\r\nDepth: 0\r\nContent-Type: application/xml; charset=utf-8\r\n\
         Content-Length: 4\r\n\r\n<x/>"
    );
}

#[test]
fn a_put_says_its_length_even_when_empty_and_a_get_does_not() {
    let site = origin("https://dav.example.org/");
    let put = Request::new(Method::Put, &site, "/e.ics").encode(None).unwrap();
    assert!(String::from_utf8(put).unwrap().contains("Content-Length: 0\r\n"));
    let get = Request::new(Method::Get, &site, "/e.ics").encode(None).unwrap();
    assert_eq!(String::from_utf8(get).unwrap(), "GET /e.ics HTTP/1.1\r\nHost: dav.example.org\r\n\r\n");
}

#[test]
fn basic_auth_goes_over_https_only() {
    let credentials = Credentials::new("ada@example.org", "not-a-real-password").unwrap();
    let request = Request::new(Method::Delete, &origin("https://dav.example.org/"), "/e.ics");
    let bytes = String::from_utf8(request.encode(Some(&credentials)).unwrap()).unwrap();
    assert!(bytes.contains("\r\nAuthorization: Basic YWRhQGV4YW1wbGUub3JnOm5vdC1hLXJlYWwtcGFzc3dvcmQ=\r\n"), "{bytes}");
    let clear = Request::new(Method::Delete, &origin("http://dav.example.org/"), "/e.ics");
    assert_eq!(clear.encode(Some(&credentials)), Err(DavError::NotEncrypted));
    assert!(clear.encode(None).is_ok());
}

#[test]
fn method_names() {
    let names: Vec<String> = [Method::Propfind, Method::Report, Method::Get, Method::Put, Method::Delete].iter().map(ToString::to_string).collect();
    assert_eq!(names, ["PROPFIND", "REPORT", "GET", "PUT", "DELETE"]);
}

#[test]
fn a_response_with_a_length_is_read_in_pieces() {
    let bytes = b"HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: 5\r\nETag:  \"1\" \r\n\r\nhello";
    let mut reader = ResponseReader::new();
    for byte in bytes.iter().take(bytes.len() - 1) {
        assert_eq!(reader.receive(&[*byte]), Ok(None));
    }
    let response = reader.receive(b"o").unwrap().unwrap();
    assert_eq!(response.status(), 207);
    assert_eq!(response.header("etag"), Some("\"1\""));
    assert_eq!(response.header("Location"), None);
    assert_eq!(response.body(), b"hello");
    assert!(!response.closes());
    assert_eq!(reader.end(), Ok(None));
}

#[test]
fn two_responses_in_one_read_come_one_by_one() {
    let mut reader = ResponseReader::new();
    let first = reader.receive(b"HTTP/1.1 204 No Content\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").unwrap();
    assert_eq!(first.map(|response| response.status()), Some(204));
    let second = reader.receive(b"").unwrap().unwrap();
    assert_eq!((second.status(), second.body()), (200, &b"ok"[..]));
}

#[test]
fn interim_responses_are_skipped_and_bare_line_feeds_read() {
    let response = read_all(b"HTTP/1.1 100 Continue\n\nHTTP/1.0 201 Created\nContent-Length: 0\nConnection: keep-alive, Close\n\n").unwrap().unwrap();
    assert_eq!(response.status(), 201);
    assert!(response.closes());
}

#[test]
fn a_chunked_body_is_put_together() {
    let bytes = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, chunked\r\n\r\n4;ext=1\r\nWiki\r\n5\r\npedia\r\nE\n in\r\n\r\nchunks.\n0\r\nX-Trailer: 1\r\n\r\n";
    let mut reader = ResponseReader::new();
    for byte in bytes.iter().take(bytes.len() - 1) {
        assert_eq!(reader.receive(&[*byte]), Ok(None));
    }
    let response = reader.receive(b"\n").unwrap().unwrap();
    assert_eq!(response.body(), b"Wikipedia in\r\n\r\nchunks.");
    let whole = read_all(bytes).unwrap().unwrap();
    assert_eq!(whole.body(), response.body());
}

#[test]
fn a_body_until_close_ends_with_the_connection() {
    let mut reader = ResponseReader::new();
    assert_eq!(reader.receive(b"HTTP/1.1 200 OK\r\n\r\nsome"), Ok(None));
    assert_eq!(reader.receive(b" more"), Ok(None));
    let response = reader.end().unwrap().unwrap();
    assert_eq!(response.body(), b"some more");
    assert_eq!(reader.end(), Ok(None));
}

#[test]
fn a_connection_closed_inside_a_response_is_an_error() {
    for bytes in
        [&b"HTTP/1.1 200"[..], b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nshort", b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nab"]
    {
        let mut reader = ResponseReader::new();
        assert_eq!(reader.receive(bytes), Ok(None));
        assert_eq!(reader.end(), Err(DavError::BadResponse("the connection closed inside a response")));
    }
}

#[test]
fn what_is_not_http_is_refused() {
    let cases: &[(&[u8], DavError)] = &[
        (b"SMTP 220 hello\r\n\r\n", DavError::BadResponse("no status line")),
        (b"HTTP/1.1 2000 OK\r\n\r\n", DavError::BadResponse("a bad status code")),
        (b"HTTP/1.1 099 OK\r\n\r\n", DavError::BadResponse("a bad status code")),
        (b"HTTP/1.1 +20 OK\r\n\r\n", DavError::BadResponse("a bad status code")),
        (b"HTTP/1.1 200 OK\r\n folded\r\n\r\n", DavError::BadResponse("a folded header field")),
        (b"HTTP/1.1 200 OK\r\nNo colon\r\n\r\n", DavError::BadResponse("a header field without a colon")),
        (b"HTTP/1.1 200 OK\r\nBad name: x\r\n\r\n", DavError::BadResponse("a bad header name")),
        (b"HTTP/1.1 200 OK\r\n: x\r\n\r\n", DavError::BadResponse("a bad header name")),
        (b"HTTP/1.1 200 \xff\r\n\r\n", DavError::BadResponse("a head that is not text")),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n", DavError::BadResponse("a transfer coding other than chunked")),
        (b"HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\n", DavError::BadResponse("a bad content length")),
        (b"HTTP/1.1 200 OK\r\nContent-Length:\r\n\r\n", DavError::BadResponse("a bad content length")),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nab", DavError::BadResponse("two content lengths")),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 99999999999999999999999\r\n\r\n", DavError::TooLarge { what: "response body", limit: MAX_BODY }),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n", DavError::BadResponse("a bad chunk size")),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\r\n", DavError::BadResponse("a bad chunk size")),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n123456789\r\n", DavError::BadResponse("a bad chunk size")),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\xff\r\n", DavError::BadResponse("a bad chunk size")),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nab\r\n", DavError::BadResponse("a chunk without its end")),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1000001\r\n", DavError::TooLarge { what: "response body", limit: MAX_BODY }),
    ];
    for (bytes, error) in cases {
        assert_eq!(read_all(bytes).as_ref(), Err(error), "{:?}", String::from_utf8_lossy(bytes));
    }
}

#[test]
fn the_same_length_twice_is_fine() {
    let response = read_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\ncontent-length: 2\r\n\r\nab").unwrap().unwrap();
    assert_eq!(response.body(), b"ab");
}

#[test]
fn the_head_and_its_fields_are_capped() {
    let mut reader = ResponseReader::new();
    let long = vec![b'a'; MAX_HEAD + 1];
    assert_eq!(reader.receive(b"HTTP/1.1 200 OK\r\nX: "), Ok(None));
    assert_eq!(reader.receive(&long), Err(DavError::TooLarge { what: "response head", limit: MAX_HEAD }));
    let mut whole = b"HTTP/1.1 200 OK\r\nX: ".to_vec();
    whole.extend_from_slice(&long);
    whole.extend_from_slice(b"\r\n\r\n");
    assert_eq!(read_all(&whole), Err(DavError::TooLarge { what: "response head", limit: MAX_HEAD }));
    let fields = "X: 1\r\n".repeat(MAX_HEADERS + 1);
    let many = format!("HTTP/1.1 200 OK\r\n{fields}\r\n");
    assert_eq!(read_all(many.as_bytes()), Err(DavError::TooLarge { what: "header count", limit: MAX_HEADERS }));
}

#[test]
fn a_body_until_close_is_capped() {
    let mut reader = ResponseReader::new();
    assert_eq!(reader.receive(b"HTTP/1.1 200 OK\r\n\r\n"), Ok(None));
    assert_eq!(reader.receive(&vec![b'a'; MAX_BODY + 1]), Err(DavError::TooLarge { what: "response body", limit: MAX_BODY }));
}

#[test]
fn chunk_lines_and_trailers_are_capped() {
    let long_size = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1;{}", "x".repeat(2000));
    assert_eq!(read_all(long_size.as_bytes()), Err(DavError::TooLarge { what: "chunk-size line", limit: 1024 }));
    let long_line = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1;{}\r\n", "x".repeat(2000));
    assert_eq!(read_all(long_line.as_bytes()), Err(DavError::TooLarge { what: "chunk-size line", limit: 1024 }));
    let trailers = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n{}", "T: 1\r\n".repeat(MAX_HEADERS + 1));
    assert_eq!(read_all(trailers.as_bytes()), Err(DavError::TooLarge { what: "header count", limit: MAX_HEADERS }));
}

#[test]
fn a_chunk_end_may_come_in_two_reads() {
    let mut reader = ResponseReader::new();
    assert_eq!(reader.receive(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r"), Ok(None));
    assert_eq!(reader.receive(b"\n0\r\n\r\n").unwrap().map(|response| response.body().to_vec()), Some(b"a".to_vec()));
}

#[test]
fn a_built_response_reads_like_one_from_the_wire() {
    let response = Response::new(412).with_header("ETag", "\"2\"").with_body(b"x".to_vec());
    assert_eq!((response.status(), response.header("ETAG"), response.body()), (412, Some("\"2\""), &b"x"[..]));
    assert!(!response.is_success());
    assert!(Response::new(204).is_success());
    assert!(Response::new(200).with_header("Connection", "close").closes());
}

#[test]
fn a_response_without_a_body_takes_none() {
    let response = read_all(b"HTTP/1.1 304 Not Modified\r\nContent-Length: 10\r\n\r\n").unwrap().unwrap();
    assert_eq!(response.body(), b"");
}
