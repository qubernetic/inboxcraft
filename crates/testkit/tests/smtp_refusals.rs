// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake SMTP server's scripts: refusals with their enhanced codes at the greeting, a command, one recipient and
//! the message, and the faults of a script.

use std::time::Duration;

use inboxcraft_testkit::Fault;
use inboxcraft_testkit::smtp::{Action, Extension, FakeSmtp, Reply, When};

/// EHLO and AUTH PLAIN with the fake's user and password.
const SIGN_IN: &str = "EHLO c.example.org\r\nAUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk\r\n";
/// One message from Ada to Bea, after the sign-in.
const MESSAGE: &str = "MAIL FROM:<ada@example.org>\r\nRCPT TO:<bea@example.org>\r\nDATA\r\nHi\r\n.\r\n";

fn lines(out: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(out).split_terminator("\r\n").map(str::to_owned).collect()
}

/// The server's lines after the sign-in, for `input`.
fn after_sign_in(fake: &FakeSmtp, input: &str) -> Vec<String> {
    let out = lines(&fake.answer(format!("{SIGN_IN}{input}").as_bytes()));
    let start = out.iter().position(|line| line.starts_with("235 ")).map_or(out.len(), |at| at + 1);
    out.get(start..).unwrap_or_default().to_vec()
}

#[test]
fn a_refused_greeting_serves_nothing_but_quit() {
    let fake = FakeSmtp::new().refuse(When::Greeting, Reply::new(554, "5.3.2", "Not accepting mail now"));
    let out = lines(&fake.answer(b"EHLO c.example.org\r\nQUIT\r\n"));
    assert_eq!(out, ["554 5.3.2 Not accepting mail now", "503 5.5.1 No service", "221 2.0.0 Bye"]);
    let fake = FakeSmtp::new().refuse(When::Greeting, Reply::new(220, "", "smtp.example.org another greeting"));
    let out = lines(&fake.answer(b"NOOP\r\n"));
    assert_eq!(out, ["220 smtp.example.org another greeting", "250 2.0.0 OK"]);
}

#[test]
fn one_recipient_is_refused_and_the_others_are_kept() {
    let fake = FakeSmtp::new().refuse(When::rcpt("Nobody@Example.org"), Reply::new(550, "5.1.1", "No such user"));
    let outbox = fake.outbox();
    let out = after_sign_in(
        &fake,
        "MAIL FROM:<ada@example.org>\r\nRCPT TO:<nobody@example.org>\r\nRCPT TO:<bea@example.org>\r\nRCPT TO:<NOBODY@example.org>\r\nDATA\r\nHi\r\n.\r\n",
    );
    assert_eq!(
        out.get(1..4),
        Some(&["550 5.1.1 No such user".to_owned(), "250 2.1.5 Recipient OK".to_owned(), "250 2.1.5 Recipient OK".to_owned()][..])
    );
    let sent = outbox.messages();
    assert_eq!(sent.first().map(|sent| sent.recipients().to_vec()), Some(vec!["bea@example.org".to_owned(), "NOBODY@example.org".to_owned()]));
}

#[test]
fn a_temporary_refusal_is_played_once() {
    let fake = FakeSmtp::new().refuse(When::command("mail"), Reply::new(451, "4.3.0", "Try again later"));
    let out = after_sign_in(&fake, &format!("{MESSAGE}{MESSAGE}"));
    assert_eq!(out.first().map(String::as_str), Some("451 4.3.0 Try again later"));
    assert_eq!(out.last().map(String::as_str), Some("250 2.0.0 Queued as 1"));
}

#[test]
fn a_refused_message_is_not_captured() {
    let fake = FakeSmtp::new().refuse(When::Message, Reply::new(554, "5.6.0", "Content refused"));
    let outbox = fake.outbox();
    let out = after_sign_in(&fake, &format!("{MESSAGE}{MESSAGE}"));
    assert!(out.contains(&"554 5.6.0 Content refused".to_owned()), "{out:?}");
    assert_eq!(out.last().map(String::as_str), Some("250 2.0.0 Queued as 1"));
    assert_eq!(outbox.messages().len(), 1);
}

#[test]
fn a_refused_sign_in_signs_nobody_in() {
    let fake = FakeSmtp::new().refuse(When::command("AUTH"), Reply::new(454, "4.7.0", "Temporary authentication failure"));
    let out = lines(&fake.answer(format!("{SIGN_IN}MAIL FROM:<ada@example.org>\r\n").as_bytes()));
    assert_eq!(out.iter().rev().take(2).collect::<Vec<_>>(), ["530 5.7.0 Authentication required", "454 4.7.0 Temporary authentication failure"]);
}

#[test]
fn a_refusal_carries_its_enhanced_code_only_where_the_server_offers_them() {
    let reply = Reply::new(550, "5.7.1", "Relaying denied").line("Ask the postmaster");
    assert_eq!(reply.code(), 550);
    let out =
        after_sign_in(&FakeSmtp::new().refuse(When::command("RCPT"), reply.clone()), "MAIL FROM:<ada@example.org>\r\nRCPT TO:<x@example.org>\r\n");
    assert_eq!(out.get(1..), Some(&["550-5.7.1 Relaying denied".to_owned(), "550 5.7.1 Ask the postmaster".to_owned()][..]));
    let fake = FakeSmtp::new().without(Extension::EnhancedStatusCodes).refuse(When::command("RCPT"), reply);
    let out = after_sign_in(&fake, "MAIL FROM:<ada@example.org>\r\nRCPT TO:<x@example.org>\r\n");
    assert_eq!(out, ["250 Sender OK", "550-Relaying denied", "550 Ask the postmaster"]);
}

#[test]
fn a_reply_code_is_kept_in_range_and_its_text_on_one_line() {
    assert_eq!(Reply::new(9999, "", "x").code(), 599);
    assert_eq!(Reply::new(12, "", "x").code(), 200);
    let fake = FakeSmtp::new().refuse(When::command("NOOP"), Reply::new(250, "2.0.0", "two\r\nlines\0"));
    assert_eq!(lines(&fake.answer(b"NOOP\r\n")).get(1..), Some(&["250 2.0.0 twolines".to_owned()][..]));
}

#[test]
fn an_unasked_reply_comes_after_the_answer() {
    let fake = FakeSmtp::new()
        .on(When::answered("noop"), [Action::Reply(Reply::new(421, "4.3.2", "Shutting down")), Action::Fault(Fault::Drop)])
        .on(When::command("NOOP"), [Action::Send("250 early".to_owned())]);
    let out = lines(&fake.answer(b"NOOP\r\nNOOP\r\n"));
    assert_eq!(out.get(1..), Some(&["250 early".to_owned(), "250 2.0.0 OK".to_owned(), "421 4.3.2 Shutting down".to_owned()][..]));
}

#[test]
fn the_faults_of_a_script_are_played() {
    let fake = FakeSmtp::new().on(When::Greeting, [Action::Fault(Fault::Drop)]);
    assert!(fake.answer(b"EHLO c.example.org\r\n").is_empty());
    let fake = FakeSmtp::new().on(When::command("DATA"), [Action::Fault(Fault::Garbage(b"\xff\x00junk".to_vec())), Action::Fault(Fault::Drop)]);
    let out = fake.answer(format!("{SIGN_IN}{MESSAGE}").as_bytes());
    assert!(out.ends_with(b"250 2.1.5 Recipient OK\r\n\xff\x00junk"), "{:?}", String::from_utf8_lossy(&out));
    let fake =
        FakeSmtp::new().on(When::rcpt("bea@example.org"), [Action::Fault(Fault::Delay(Duration::from_millis(10))), Action::Fault(Fault::Drop)]);
    let out = lines(&fake.answer(format!("{SIGN_IN}{MESSAGE}").as_bytes()));
    assert_eq!(out.last().map(String::as_str), Some("250 2.1.0 Sender OK"));
    let fake = FakeSmtp::new().on(When::Message, [Action::Fault(Fault::Drop)]);
    let outbox = fake.outbox();
    let out = lines(&fake.answer(format!("{SIGN_IN}{MESSAGE}").as_bytes()));
    assert_eq!(out.last().map(String::as_str), Some("354 End data with <CR><LF>.<CR><LF>"));
    assert!(outbox.messages().is_empty());
}
