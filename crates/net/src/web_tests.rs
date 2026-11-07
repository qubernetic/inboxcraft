// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use inboxcraft_testkit::{CertFault, HOST, TestPki};
use inboxcraft_time::SystemClock;
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use super::{Https, MAX_BODY, Web};
use crate::{TlsSettings, Trust};

/// The request head a server got.
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

/// A server for one request that answers `response` (over TLS with `tls`); the request head it got.
fn serve(tls: Option<Arc<ServerConfig>>, response: Vec<u8>) -> (SocketAddr, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let thread = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        match tls {
            Some(config) => {
                let mut stream = StreamOwned::new(ServerConnection::new(config).unwrap(), socket);
                let request = head(&mut stream);
                let _written = stream.write_all(&response);
                stream.conn.send_close_notify();
                let _flushed = stream.flush();
                request
            }
            None => {
                let mut stream = socket;
                let request = head(&mut stream);
                let _written = stream.write_all(&response);
                request
            }
        }
    });
    (addr, thread)
}

fn client(pki: &TestPki, addr: SocketAddr, timeout: Duration) -> Https {
    Https::with_addresses(TlsSettings::new(Trust::Roots(vec![pki.ca()]), Arc::new(SystemClock::new())), timeout, vec![addr])
}

fn ok(body: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

#[test]
fn a_plain_request_tells_nothing_but_the_url_and_gives_the_redirect() {
    let pki = TestPki::new().unwrap();
    let (addr, server) = serve(None, b"HTTP/1.1 301 Moved\r\nLocation: https://autoconfig.example.net/x\r\nContent-Length: 0\r\n\r\n".to_vec());
    let answer = client(&pki, addr, Duration::from_secs(5)).get("http://autoconfig.example.org/mail/config-v1.1.xml").unwrap();
    assert_eq!((answer.status, answer.location.as_deref()), (301, Some("https://autoconfig.example.net/x")));
    let request = server.join().unwrap().to_lowercase();
    assert!(request.starts_with("get /mail/config-v1.1.xml http/1.1\r\n"), "{request}");
    assert!(request.contains("host: autoconfig.example.org"), "{request}");
    for header in ["user-agent", "accept", "cookie", "authorization"] {
        assert!(!request.contains(&format!("\r\n{header}")), "{header} in {request}");
    }
}

#[test]
fn https_with_a_trusted_certificate_gives_the_page() {
    let pki = TestPki::new().unwrap();
    let (addr, server) = serve(Some(pki.server_config(None).unwrap()), ok("<clientConfig/>"));
    let answer = client(&pki, addr, Duration::from_secs(5)).get(&format!("https://{HOST}/mail/config-v1.1.xml")).unwrap();
    assert_eq!((answer.status, answer.body.as_slice()), (200, b"<clientConfig/>".as_slice()));
    assert!(server.join().unwrap().starts_with("GET /mail/config-v1.1.xml"));
}

#[test]
fn https_refuses_an_untrusted_or_wrong_certificate() {
    let pki = TestPki::new().unwrap();
    let (addr, _server) = serve(Some(pki.server_config(Some(CertFault::UnknownCa)).unwrap()), ok("x"));
    let error = client(&pki, addr, Duration::from_secs(5)).get(&format!("https://{HOST}/")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::CertificateUnknownIssuer, "{error}");
    let (addr, _server) = serve(Some(pki.server_config(None).unwrap()), ok("x"));
    let error = client(&pki, addr, Duration::from_secs(5)).get("https://other.example.org/").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::CertificateWrongHost, "{error}");
}

#[test]
fn a_larger_answer_than_an_autoconfig_file_is_refused() {
    let pki = TestPki::new().unwrap();
    let body = "x".repeat(usize::try_from(MAX_BODY).unwrap() + 10);
    let (addr, _server) = serve(None, ok(&body));
    let error = client(&pki, addr, Duration::from_secs(5)).get("http://autoconfig.example.org/").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TooLarge, "{error}");
}

#[test]
fn a_silent_server_times_out_and_a_closed_port_refuses() {
    let pki = TestPki::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let holder = thread::spawn(move || {
        let held: Option<TcpStream> = listener.accept().ok().map(|(socket, _)| socket);
        thread::sleep(Duration::from_secs(1));
        drop(held);
    });
    let error = client(&pki, addr, Duration::from_millis(200)).get("http://autoconfig.example.org/").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TimedOut, "{error}");
    holder.join().unwrap();

    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let error = client(&pki, closed, Duration::from_secs(5)).get("http://autoconfig.example.org/").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ConnectionRefused, "{error}");
}

#[test]
fn a_client_with_the_platform_trust_can_be_made() {
    let https = Https::new(TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new())), Duration::from_secs(5));
    assert!(format!("{https:?}").contains("Platform"));
    let error = Https::with_addresses(TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new())), Duration::from_secs(1), Vec::new())
        .get("not a url")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Protocol, "{error}");
}
