// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server's COMPRESS=DEFLATE (RFC 4978): negotiation, and a session that goes on compressed.

use std::time::Duration;

use inboxcraft_testkit::imap::{Capability, FakeImap, PASSWORD, USER};
use inboxcraft_testkit::{Clock, Error, ErrorKind, Listen, TestClient, TestPki};

const TIMEOUT: Duration = Duration::from_secs(5);

fn signed_in(fake: &FakeImap, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(format!("s1 LOGIN {USER} {PASSWORD}\r\n{input}").as_bytes())).into_owned();
    out.split_terminator("\r\n").skip(2).map(str::to_owned).collect()
}

#[test]
fn compress_deflate_is_negotiated_after_sign_in() {
    let fake = FakeImap::sample();
    assert_eq!(signed_in(&fake, "a1 COMPRESS DEFLATE\r\na2 NOOP\r\n"), ["a1 OK DEFLATE active"]);
    assert_eq!(signed_in(&fake, "a1 COMPRESS GZIP\r\na2 COMPRESS\r\n"), ["a1 BAD Unknown compression", "a2 BAD Unknown compression"]);
    let before = String::from_utf8_lossy(&fake.answer(b"a1 COMPRESS DEFLATE\r\n")).into_owned();
    assert!(before.ends_with("a1 BAD COMPRESS is not available now\r\n"), "{before}");
    let off = signed_in(&fake.without(Capability::CompressDeflate), "a1 COMPRESS DEFLATE\r\n");
    assert_eq!(off, ["a1 BAD COMPRESS is not available now"]);
}

#[test]
fn a_compressed_session_goes_on() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::sample())?;
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, TIMEOUT)?;
    assert!(client.read_line()?.contains("COMPRESS=DEFLATE"));
    client.command(&format!("a1 LOGIN {USER} {PASSWORD}"))?;
    assert_eq!(client.command("a2 COMPRESS DEFLATE")?, ["a2 OK DEFLATE active"]);
    client.compress();
    let caps = client.command("a3 CAPABILITY")?;
    assert!(caps.first().is_some_and(|line| line.starts_with("* CAPABILITY IMAP4rev1") && !line.contains("COMPRESS")), "{caps:?}");
    assert_eq!(client.command("a4 COMPRESS DEFLATE")?, ["a4 NO [COMPRESSIONACTIVE] Compression is already active"]);
    let fetched = client.command("a5 EXAMINE INBOX")?;
    assert_eq!(fetched.last().map(String::as_str), Some("a5 OK [READ-ONLY] EXAMINE completed"));
    assert_eq!(client.command("a6 LOGOUT")?, ["* BYE Logging out", "a6 OK LOGOUT completed"]);
    assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    assert_eq!(server.finish()?.received().len(), 6);
    Ok(())
}

#[test]
fn a_compressed_line_that_inflates_past_the_cap_ends_the_session() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start_imap(FakeImap::sample().preauth())?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.read_line()?;
    assert_eq!(client.command("a1 COMPRESS DEFLATE")?, ["a1 OK DEFLATE active"]);
    client.compress();
    client.send_line(&"a".repeat(3 << 20))?;
    let err = server.finish().err().ok_or("the server read it")?;
    assert!(matches!(&err, Error::Io(io) if io.kind() == std::io::ErrorKind::InvalidData), "{err}");
    Ok(())
}
