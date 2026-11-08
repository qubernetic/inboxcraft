// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bounce script: the fake SMTP server accepts a message, then drops an RFC 3464 delivery report about it into the
//! fake IMAP server's mailbox, where a client finds it as new mail.

use std::time::Duration;

use inboxcraft_mime::Action as DsnAction;
use inboxcraft_testkit::imap::{FakeImap, Mailbox, Message, PASSWORD, Postbox, USER};
use inboxcraft_testkit::smtp::{Bounce, FakeSmtp};
use inboxcraft_testkit::{Clock, Listen, TestClient, TestPki};

const TIMEOUT: Duration = Duration::from_secs(5);
/// EHLO and AUTH PLAIN with the fake's user and password.
const SIGN_IN: &str = "EHLO c.example.org\r\nAUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk\r\n";

/// Sends one message from `from` to `to` through `fake` and returns the server's last line.
fn send(fake: &FakeSmtp, from: &str, to: &[&str]) -> String {
    let recipients: String = to.iter().map(|to| format!("RCPT TO:<{to}>\r\n")).collect();
    let input =
        format!("{SIGN_IN}MAIL FROM:<{from}>\r\n{recipients}DATA\r\nMessage-ID: <lunch.1@example.org>\r\nSubject: Lunch\r\n\r\nNoon?\r\n.\r\n");
    let out = String::from_utf8_lossy(&fake.answer(input.as_bytes())).into_owned();
    out.split_terminator("\r\n").last().unwrap_or_default().to_owned()
}

/// The body of `uid` in INBOX, fetched from `imap` without a socket.
fn fetch(imap: &FakeImap, uid: u32) -> Vec<u8> {
    let out = imap.answer(format!("a1 LOGIN {USER} {PASSWORD}\r\na2 SELECT INBOX\r\na3 UID FETCH {uid} BODY[]\r\n").as_bytes());
    let Some(at) = out.windows(8).position(|window| window == b"BODY[] {") else { return Vec::new() };
    let rest = out.get(at + 8..).unwrap_or_default();
    let digits: String = rest.iter().take_while(|byte| byte.is_ascii_digit()).map(|&byte| char::from(byte)).collect();
    let len: usize = digits.parse().unwrap_or_default();
    rest.get(digits.len() + 3..digits.len() + 3 + len).unwrap_or_default().to_vec()
}

#[test]
fn a_bounce_arrives_as_a_delivery_report() -> Result<(), Box<dyn std::error::Error>> {
    let imap = FakeImap::new();
    let fake = FakeSmtp::new().bounce(Bounce::new("Nobody@example.org", &imap.postbox()));
    let outbox = fake.outbox();
    assert_eq!(send(&fake, "ada@example.org", &["bea@example.org", "nobody@example.org"]), "250 2.0.0 Queued as 1");
    assert_eq!(outbox.messages().first().map(|sent| sent.recipients().len()), Some(2));

    let bytes = fetch(&imap, 1);
    let report = inboxcraft_mime::parse(&bytes)?;
    assert_eq!(report.headers().get("From"), Some("Mail Delivery System <MAILER-DAEMON@smtp.example.org>"));
    assert_eq!(report.headers().get("To"), Some("<ada@example.org>"));
    let status = report.delivery_status()?;
    let [recipient] = status.recipients.as_slice() else { return Err(format!("{status:?}").into()) };
    assert_eq!(recipient.final_recipient.value, "nobody@example.org");
    assert_eq!(recipient.action, DsnAction::Failed);
    assert_eq!(recipient.status.to_string(), "5.1.1");
    assert_eq!(recipient.diagnostic_code.as_ref().map(|code| code.value.as_str()), Some("550 5.1.1 The mailbox does not exist"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Content-Type: text/rfc822-headers\r\n\r\nMessage-ID: <lunch.1@example.org>\r\nSubject: Lunch\r\n\r\n--"), "{text}");
    Ok(())
}

#[test]
fn a_bounce_takes_its_status_and_mailbox() -> Result<(), Box<dyn std::error::Error>> {
    let imap = FakeImap::new().mailbox(Mailbox::new("Reports"));
    let bounce = Bounce::new("full@example.org", &imap.postbox()).status("4.2.2", "452 4.2.2 Mailbox full").mailbox("Reports");
    let fake = FakeSmtp::new().bounce(bounce.clone()).bounce(bounce);
    send(&fake, "ada@example.org", &["full@example.org"]);
    let out = String::from_utf8_lossy(&imap.answer(format!("a1 LOGIN {USER} {PASSWORD}\r\na2 SELECT Reports\r\na3 SELECT INBOX\r\n").as_bytes()))
        .into_owned();
    assert!(out.contains("* 2 EXISTS\r\n") && out.contains("* 0 EXISTS\r\n"), "{out}");
    Ok(())
}

#[test]
fn a_message_from_the_null_path_is_not_reported() {
    let imap = FakeImap::new();
    let fake = FakeSmtp::new().bounce(Bounce::new("nobody@example.org", &imap.postbox()));
    assert_eq!(send(&fake, "", &["nobody@example.org"]), "250 2.0.0 Queued as 1");
    let out = String::from_utf8_lossy(&imap.answer(format!("a1 LOGIN {USER} {PASSWORD}\r\na2 SELECT INBOX\r\n").as_bytes())).into_owned();
    assert!(out.contains("* 0 EXISTS\r\n"), "{out}");
}

#[test]
fn a_postbox_reaches_its_own_fake_and_mailboxes_that_are_there() {
    let imap = FakeImap::new();
    let postbox: Postbox = imap.postbox();
    postbox.deliver("INBOX", Message::new("Subject: by hand\n\nHello"));
    postbox.deliver("Nowhere", Message::new("Subject: lost\n\nHello"));
    Postbox::new().deliver("INBOX", Message::new("Subject: not this fake's\n\nHello"));
    let out = String::from_utf8_lossy(&imap.answer(format!("a1 LOGIN {USER} {PASSWORD}\r\na2 SELECT INBOX\r\n").as_bytes())).into_owned();
    assert!(out.contains("* 1 EXISTS\r\n"), "{out}");
}

#[test]
fn a_client_watching_the_mailbox_hears_of_the_report() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let imap = FakeImap::sample();
    let postbox = imap.postbox();
    let imap_server = Listen::tls(pki.server_config(None)?).start_imap(imap)?;
    let mut reader = TestClient::tls(imap_server.addr(), &pki.client(Clock::wall())?, TIMEOUT)?;
    reader.read_line()?;
    reader.command(&format!("a1 LOGIN {USER} {PASSWORD}"))?;
    let selected = reader.command("a2 SELECT INBOX")?;
    assert!(selected.contains(&"* 3 EXISTS".to_owned()), "{selected:?}");

    let fake = FakeSmtp::new().bounce(Bounce::new("nobody@example.org", &postbox));
    assert_eq!(send(&fake, "ada@example.org", &["nobody@example.org"]), "250 2.0.0 Queued as 1");
    assert_eq!(reader.command("a3 NOOP")?, ["* 4 EXISTS", "a3 OK NOOP completed"]);
    reader.command("a4 LOGOUT")?;
    imap_server.finish()?;
    Ok(())
}

#[test]
fn a_bare_cr_in_the_paths_stays_out_of_the_report_header() -> Result<(), Box<dyn std::error::Error>> {
    let imap = FakeImap::new();
    let fake = FakeSmtp::new().bounce(Bounce::new("no\rbody@example.org", &imap.postbox()));
    assert_eq!(send(&fake, "ada\r@example.org", &["nobody@example.org"]), "250 2.0.0 Queued as 1");
    let bytes = fetch(&imap, 1);
    let end = bytes.windows(4).position(|window| window == b"\r\n\r\n").ok_or("no header block")?;
    let header = bytes.get(..end + 2).unwrap_or_default();
    let bare = header.iter().enumerate().any(|(at, &byte)| byte == b'\r' && header.get(at + 1) != Some(&b'\n'));
    assert!(!bare, "{}", String::from_utf8_lossy(header));
    let report = inboxcraft_mime::parse(&bytes)?;
    assert_eq!(report.headers().get("To"), Some("<ada@example.org>"));
    let status = report.delivery_status()?;
    let [recipient] = status.recipients.as_slice() else { return Err(format!("{status:?}").into()) };
    assert_eq!(recipient.final_recipient.value, "nobody@example.org");
    Ok(())
}

#[test]
fn a_report_reaches_every_later_session_once() -> Result<(), Box<dyn std::error::Error>> {
    let imap = FakeImap::new().without(inboxcraft_testkit::imap::Capability::LoginDisabled);
    let fake = FakeSmtp::new().bounce(Bounce::new("nobody@example.org", &imap.postbox()));
    send(&fake, "ada@example.org", &["nobody@example.org"]);
    let select = format!("a1 LOGIN {USER} {PASSWORD}\r\na2 SELECT INBOX\r\n");
    for _ in 0..2 {
        let out = String::from_utf8_lossy(&imap.answer(select.as_bytes())).into_owned();
        assert!(out.contains("* 1 EXISTS\r\n"), "{out}");
    }
    assert!(!fetch(&imap, 1).is_empty());

    // Over a socket the mailboxes carry over from one connection to the next, the report with them, never twice.
    let server = Listen::plain().accept_timeout(Duration::from_millis(500)).start_imap_many(imap, 2)?;
    for tag in ["b", "c"] {
        let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
        client.read_line()?;
        client.command(&format!("{tag}1 LOGIN {USER} {PASSWORD}"))?;
        let selected = client.command(&format!("{tag}2 SELECT INBOX"))?;
        assert!(selected.contains(&"* 1 EXISTS".to_owned()), "{selected:?}");
        client.command(&format!("{tag}3 LOGOUT"))?;
    }
    server.finish()?;
    Ok(())
}
