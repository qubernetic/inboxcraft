// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use inboxcraft_testkit::{HOST, TestPki};
use inboxcraft_time::SystemClock;
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use super::{MAX_PICTURE, PictureClient, USER_AGENT, is_public};
use crate::{TlsSettings, Trust};

fn head(stream: &mut impl Read) -> String {
    let mut seen = Vec::new();
    let mut byte = [0; 1];
    while !seen.ends_with(b"\r\n\r\n") && seen.len() < 65_536 {
        match stream.read(&mut byte) {
            Ok(1) => seen.push(byte[0]),
            _ => break,
        }
    }
    String::from_utf8_lossy(&seen).into_owned()
}

/// A TLS server that answers one request per answer in `answers`, in turn; the request heads it got.
fn serve(tls: Arc<ServerConfig>, answers: Vec<Vec<u8>>) -> (SocketAddr, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let thread = thread::spawn(move || {
        let mut heads = Vec::new();
        for answer in answers {
            let Ok((socket, _)) = listener.accept() else { break };
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut stream = StreamOwned::new(ServerConnection::new(tls.clone()).unwrap(), socket);
            heads.push(head(&mut stream));
            let _written = stream.write_all(&answer);
            stream.conn.send_close_notify();
            let _flushed = stream.flush();
        }
        heads
    });
    (addr, thread)
}

fn client(pki: &TestPki, addr: SocketAddr) -> PictureClient {
    PictureClient::with_addresses(TlsSettings::new(Trust::Roots(vec![pki.ca()]), Arc::new(SystemClock::new())), Duration::from_secs(5), vec![addr])
}

fn answer(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nSet-Cookie: id=1\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot really";

#[test]
fn a_picture_request_tells_the_server_nothing_but_the_url() {
    let pki = TestPki::new().unwrap();
    let (addr, server) = serve(pki.server_config(None).unwrap(), vec![answer("200 OK", "image/png", PNG)]);
    let picture = client(&pki, addr).fetch(&format!("https://{HOST}/news/p.png?u=42")).unwrap();
    assert_eq!(picture.media_type, "image/png");
    assert_eq!(picture.bytes, PNG);
    let head = server.join().unwrap().concat().to_lowercase();
    let mut lines: Vec<&str> = head.trim_end().split("\r\n").collect();
    lines.sort_unstable();
    let host = format!("host: {HOST}").to_lowercase();
    assert_eq!(lines, ["get /news/p.png?u=42 http/1.1", host.as_str(), "user-agent: inboxcraft"], "only these three lines");
    assert_eq!(USER_AGENT, "InboxCraft", "the same for everyone, no version");
}

#[test]
fn a_cookie_the_server_sets_is_never_sent_back() {
    let pki = TestPki::new().unwrap();
    let (addr, server) = serve(pki.server_config(None).unwrap(), vec![answer("200 OK", "image/png", PNG), answer("200 OK", "image/png", PNG)]);
    let client = client(&pki, addr);
    client.fetch(&format!("https://{HOST}/a.png")).unwrap();
    client.fetch(&format!("https://{HOST}/b.png")).unwrap();
    let heads = server.join().unwrap();
    assert_eq!(heads.len(), 2);
    for head in heads {
        let head = head.to_lowercase();
        for header in ["cookie", "referer", "accept", "authorization", "origin"] {
            assert!(!head.contains(&format!("\r\n{header}")), "{header} in {head}");
        }
    }
}

#[test]
fn a_redirect_to_a_private_address_is_never_followed() {
    let pki = TestPki::new().unwrap();
    for target in ["https://10.0.0.1/x.png", "https://127.0.0.1:1/x.png", "https://[::1]/x.png", "https://[64:ff9b::a00:1]/x.png"] {
        let moved = format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes();
        let (addr, server) = serve(pki.server_config(None).unwrap(), vec![moved]);
        let error = client(&pki, addr).fetch(&format!("https://{HOST}/p.png")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::HostNotFound, "{target}: {error}");
        assert_eq!(server.join().unwrap().len(), 1);
    }
}

#[test]
fn every_fetch_is_a_new_connection_with_a_full_handshake() {
    let pki = TestPki::new().unwrap();
    let tls = pki.server_config(None).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut kinds = Vec::new();
        for _ in 0..2 {
            let Ok((socket, _)) = listener.accept() else { break };
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut stream = StreamOwned::new(ServerConnection::new(tls.clone()).unwrap(), socket);
            head(&mut stream);
            kinds.push(stream.conn.handshake_kind());
            // Keep-alive: an answer that would let the client keep the connection for the next picture.
            let _written = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 1\r\n\r\nx");
            let _flushed = stream.flush();
            let mut rest = [0; 1];
            let _ = stream.read(&mut rest);
        }
        kinds
    });
    let client = client(&pki, addr);
    client.fetch(&format!("https://{HOST}/a.png")).unwrap();
    client.fetch(&format!("https://{HOST}/b.png")).unwrap();
    let kinds = server.join().unwrap();
    assert_eq!(kinds, [Some(rustls::HandshakeKind::Full), Some(rustls::HandshakeKind::Full)], "no reused connection, no resumed session");
}

#[test]
fn plain_http_and_other_schemes_are_refused_before_any_connection() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let pki = TestPki::new().unwrap();
    for url in
        ["http://pictures.example.org/p.png", "ftp://pictures.example.org/p.png", "file:///etc/passwd", "data:image/png;base64,AA==", "", "p.png"]
    {
        let error = client(&pki, closed).fetch(url).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Protocol, "{url}: {error}");
    }
}

#[test]
fn a_redirect_is_followed_over_https_only() {
    let pki = TestPki::new().unwrap();
    let moved = format!("HTTP/1.1 302 Found\r\nLocation: https://{HOST}/real.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes();
    let (addr, server) = serve(pki.server_config(None).unwrap(), vec![moved, answer("200 OK", "image/jpeg", b"\xff\xd8\xff")]);
    let picture = client(&pki, addr).fetch(&format!("https://{HOST}/p.png")).unwrap();
    assert_eq!(picture.media_type, "image/jpeg");
    let heads = server.join().unwrap();
    assert!(heads[1].starts_with("GET /real.png HTTP/1.1\r\n"), "{heads:?}");
    assert!(!heads[1].to_lowercase().contains("referer"), "{heads:?}");

    let down = format!("HTTP/1.1 302 Found\r\nLocation: http://{HOST}/plain.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes();
    let (addr, server) = serve(pki.server_config(None).unwrap(), vec![down]);
    let error = client(&pki, addr).fetch(&format!("https://{HOST}/p.png")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Protocol, "{error}");
    assert_eq!(server.join().unwrap().len(), 1, "the plain address was not asked");
}

#[test]
fn too_many_redirects_are_refused() {
    let pki = TestPki::new().unwrap();
    let moved = format!("HTTP/1.1 302 Found\r\nLocation: https://{HOST}/again.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes();
    let (addr, _server) = serve(pki.server_config(None).unwrap(), vec![moved; 6]);
    let error = client(&pki, addr).fetch(&format!("https://{HOST}/p.png")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Protocol, "{error}");
}

#[test]
fn what_is_no_picture_is_refused() {
    let pki = TestPki::new().unwrap();
    for (status, content_type) in [("200 OK", "text/html"), ("200 OK", "image"), ("404 Not Found", "image/png"), ("200 OK", "")] {
        let (addr, _server) = serve(pki.server_config(None).unwrap(), vec![answer(status, content_type, PNG)]);
        let error = client(&pki, addr).fetch(&format!("https://{HOST}/p.png")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Protocol, "{status} {content_type}: {error}");
    }
    let (addr, _server) = serve(pki.server_config(None).unwrap(), vec![answer("200 OK", "Image/PNG; charset=x", PNG)]);
    assert_eq!(client(&pki, addr).fetch(&format!("https://{HOST}/p.png")).unwrap().media_type, "image/png");
}

#[test]
fn a_picture_larger_than_the_cap_is_refused() {
    let pki = TestPki::new().unwrap();
    let big = vec![0_u8; usize::try_from(MAX_PICTURE).unwrap() + 1];
    let (addr, _server) = serve(pki.server_config(None).unwrap(), vec![answer("200 OK", "image/png", &big)]);
    let error = client(&pki, addr).fetch(&format!("https://{HOST}/p.png")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TooLarge, "{error}");
}

#[test]
fn only_public_addresses_are_asked() {
    for ip in [
        "127.0.0.1",
        "10.1.2.3",
        "172.16.0.1",
        "192.168.1.1",
        "169.254.1.1",
        "100.64.0.1",
        "0.0.0.0",
        "255.255.255.255",
        "224.0.0.1",
        "192.0.2.1",
        "198.18.0.1",
        "240.0.0.1",
        "192.0.0.8",
    ] {
        assert!(!is_public(ip.parse::<Ipv4Addr>().unwrap().into()), "{ip}");
    }
    for ip in ["::1", "::", "fc00::1", "fd12::1", "fe80::1", "ff02::1", "2001:db8::1", "::ffff:127.0.0.1", "::ffff:10.0.0.1"] {
        assert!(!is_public(ip.parse::<Ipv6Addr>().unwrap().into()), "{ip}");
    }
    for ip in ["93.184.215.14", "1.1.1.1", "8.8.8.8"] {
        assert!(is_public(IpAddr::V4(ip.parse().unwrap())), "{ip}");
    }
    for ip in ["2606:4700::1111", "::ffff:1.1.1.1"] {
        assert!(is_public(IpAddr::V6(ip.parse().unwrap())), "{ip}");
    }
}

#[test]
fn an_ipv6_form_that_carries_an_ipv4_address_is_judged_by_that_address() {
    // IPv4-compatible (deprecated), NAT64, 6to4; the local-use NAT64 prefix is never public.
    for ip in [
        "::10.0.0.1",
        "::127.0.0.1",
        "64:ff9b::10.0.0.1",
        "64:ff9b::127.0.0.1",
        "64:ff9b::169.254.169.254",
        "2002:a00:1::1",
        "2002:7f00:1::",
        "64:ff9b:1::1.1.1.1",
        "::ffff:0:10.0.0.1",
        "::ffff:0:127.0.0.1",
    ] {
        assert!(!is_public(ip.parse::<Ipv6Addr>().unwrap().into()), "{ip}");
    }
    // Site-local (deprecated), discard-only, benchmarking, the second documentation prefix, SRv6 segment ids.
    for ip in ["fec0::1", "feff::1", "100::1", "2001:2::1", "3fff::1", "3fff:fff::1", "5f00::1"] {
        assert!(!is_public(ip.parse::<Ipv6Addr>().unwrap().into()), "{ip}");
    }
    for ip in ["64:ff9b::1.1.1.1", "2002:101:101::1", "::1.1.1.1", "::ffff:0:1.1.1.1", "100:0:0:1::1", "2001:3::1", "3fff:1000::1", "5f01::1"] {
        assert!(is_public(ip.parse::<Ipv6Addr>().unwrap().into()), "{ip}");
    }
}

#[test]
fn a_host_on_this_computer_or_the_local_network_is_never_asked() {
    let client = PictureClient::new(TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new())), Duration::from_secs(2));
    assert!(format!("{client:?}").contains("Platform"));
    for url in ["https://localhost/p.png", "https://127.0.0.1/p.png", "https://[::1]/p.png", "https://192.168.1.1/p.png"] {
        let error = client.fetch(url).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::HostNotFound, "{url}: {error}");
    }
}
