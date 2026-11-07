// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake SMTP server's session without a socket: the greeting, EHLO and its extensions, a whole submission and the
//! messages it captures, the parameters of MAIL, and the commands out of order.

use inboxcraft_testkit::smtp::{Extension, FakeSmtp, MAX_SIZE};

/// AUTH PLAIN with the fake's user and password.
const SIGN_IN: &str = "AUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk\r\n";

fn lines(out: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(out).split_terminator("\r\n").map(str::to_owned).collect()
}

/// The server's lines after the greeting, EHLO and the sign-in, for `input`.
fn after_sign_in(fake: &FakeSmtp, input: &str) -> Vec<String> {
    let out = lines(&fake.answer(format!("EHLO c.example.org\r\n{SIGN_IN}{input}").as_bytes()));
    let start = out.iter().position(|line| line.starts_with("235 ")).map_or(out.len(), |at| at + 1);
    out.get(start..).unwrap_or_default().to_vec()
}

#[test]
fn greets_and_lists_its_extensions_after_ehlo() {
    let out = FakeSmtp::new().answer(b"EHLO client.example.org\r\nQUIT\r\n");
    assert_eq!(
        lines(&out),
        [
            "220 smtp.example.org ESMTP InboxCraft fake SMTP ready",
            "250-smtp.example.org greets client.example.org",
            "250-PIPELINING",
            "250-SIZE 10485760",
            "250-8BITMIME",
            "250-SMTPUTF8",
            "250-ENHANCEDSTATUSCODES",
            "250 AUTH PLAIN LOGIN",
            "221 2.0.0 Bye",
        ]
    );
}

#[test]
fn an_extension_switched_off_is_not_listed() {
    let fake = FakeSmtp::new().without(Extension::Pipelining).without(Extension::EnhancedStatusCodes).without(Extension::AuthLogin);
    let out = lines(&fake.answer(b"EHLO c.example.org\r\nQUIT\r\n"));
    assert!(!out.iter().any(|line| line.contains("PIPELINING") || line.contains("ENHANCEDSTATUSCODES")), "{out:?}");
    assert!(out.iter().any(|line| line == "250 AUTH PLAIN"), "{out:?}");
    assert_eq!(out.last().map(String::as_str), Some("221 Bye"));
    let out = lines(&fake.with(Extension::Pipelining).with(Extension::Pipelining).answer(b"EHLO c.example.org\r\n"));
    assert_eq!(out.iter().filter(|line| line.contains("PIPELINING")).count(), 1, "{out:?}");
}

#[test]
fn helo_greets_without_extensions() {
    let out = lines(&FakeSmtp::new().answer(b"HELO c.example.org\r\nNOOP\r\nNOOP anything\r\n"));
    assert_eq!(
        out.get(1..),
        Some(&["250 smtp.example.org greets c.example.org".to_owned(), "250 2.0.0 OK".to_owned(), "250 2.0.0 OK".to_owned()][..])
    );
}

#[test]
fn a_whole_submission_is_captured() {
    let fake = FakeSmtp::new();
    let outbox = fake.outbox();
    let out = after_sign_in(
        &fake,
        "MAIL FROM:<ada@example.org> SIZE=64 BODY=8BITMIME\r\nRCPT TO:<bea@example.org>\r\nRCPT TO:<cy@example.org>\r\nDATA\r\n\
         Subject: Lunch\r\n\r\nNoon?\r\n..\r\n...two dots\r\n.\r\nQUIT\r\n",
    );
    assert_eq!(
        out,
        [
            "250 2.1.0 Sender OK",
            "250 2.1.5 Recipient OK",
            "250 2.1.5 Recipient OK",
            "354 End data with <CR><LF>.<CR><LF>",
            "250 2.0.0 Queued as 1",
            "221 2.0.0 Bye",
        ]
    );
    let sent = outbox.messages();
    let [sent] = sent.as_slice() else { panic!("one message, not {sent:?}") };
    assert_eq!(sent.id(), 1);
    assert_eq!(sent.from(), "ada@example.org");
    assert_eq!(sent.params(), ["SIZE=64", "BODY=8BITMIME"]);
    assert_eq!(sent.recipients(), ["bea@example.org", "cy@example.org"]);
    assert_eq!(sent.data(), b"Subject: Lunch\r\n\r\nNoon?\r\n.\r\n..two dots\r\n");
    assert_eq!(sent.user(), Some("ada@example.org"));
    assert!(sent.tls());
}

#[test]
fn messages_one_after_the_other_take_the_next_queue_number() {
    let fake = FakeSmtp::new();
    let outbox = fake.outbox();
    let message = "MAIL FROM:<ada@example.org>\r\nRCPT TO:<bea@example.org>\r\nDATA\r\nHi\r\n.\r\n";
    let out = after_sign_in(&fake, &format!("{message}{message}RSET\r\n"));
    assert!(out.contains(&"250 2.0.0 Queued as 2".to_owned()), "{out:?}");
    assert_eq!(out.last().map(String::as_str), Some("250 2.0.0 Reset"));
    assert_eq!(outbox.messages().iter().map(|sent| sent.id()).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn rset_ends_the_transaction() {
    let out = after_sign_in(&FakeSmtp::new(), "MAIL FROM:<ada@example.org>\r\nRCPT TO:<bea@example.org>\r\nRSET\r\nDATA\r\n");
    assert_eq!(out.last().map(String::as_str), Some("503 5.5.1 Need MAIL first"));
}

#[test]
fn commands_out_of_order_are_refused() {
    let out = lines(&FakeSmtp::new().answer(b"MAIL FROM:<ada@example.org>\r\nAUTH PLAIN\r\n"));
    assert_eq!(out.get(1..), Some(&["503 5.5.1 Say EHLO first".to_owned(), "503 5.5.1 Say EHLO first".to_owned()][..]));
    let out = lines(&FakeSmtp::new().answer(b"EHLO c.example.org\r\nMAIL FROM:<ada@example.org>\r\n"));
    assert_eq!(out.last().map(String::as_str), Some("530 5.7.0 Authentication required"));
    let out = after_sign_in(
        &FakeSmtp::new(),
        "RCPT TO:<bea@example.org>\r\nDATA\r\nMAIL FROM:<ada@example.org>\r\nMAIL FROM:<ada@example.org>\r\nDATA\r\n",
    );
    assert_eq!(
        out,
        [
            "503 5.5.1 Need MAIL first",
            "503 5.5.1 Need MAIL first",
            "250 2.1.0 Sender OK",
            "503 5.5.1 Nested MAIL command",
            "554 5.5.1 No valid recipients",
        ]
    );
}

#[test]
fn a_server_without_auth_takes_mail_unsigned() {
    let fake = FakeSmtp::new().without(Extension::AuthPlain).without(Extension::AuthLogin);
    let outbox = fake.outbox();
    let out = lines(&fake.answer(b"EHLO c.example.org\r\nMAIL FROM:<>\r\nRCPT TO:<bea@example.org>\r\nDATA\r\n.\r\n"));
    assert!(!out.iter().any(|line| line.contains("AUTH")), "{out:?}");
    assert_eq!(out.last().map(String::as_str), Some("250 2.0.0 Queued as 1"));
    let sent = outbox.messages();
    assert_eq!(sent.first().map(|sent| (sent.from().to_owned(), sent.user().is_none(), sent.data().is_empty())), Some((String::new(), true, true)));
}

#[test]
fn the_parameters_of_mail_need_their_extension() {
    let cases = [
        ("SIZE=10485761", FakeSmtp::new(), "552 5.3.4 Message size exceeds fixed maximum message size"),
        ("SIZE=ten", FakeSmtp::new(), "501 5.5.4 SIZE needs a number"),
        ("SIZE", FakeSmtp::new(), "501 5.5.4 The parameter's value cannot be read"),
        ("BODY=BINARYMIME", FakeSmtp::new(), "501 5.5.4 The parameter's value cannot be read"),
        ("BODY=7BIT", FakeSmtp::new(), "250 2.1.0 Sender OK"),
        ("SMTPUTF8", FakeSmtp::new(), "250 2.1.0 Sender OK"),
        ("SMTPUTF8=yes", FakeSmtp::new(), "555 5.5.4 Parameter not recognised"),
        ("RET=HDRS", FakeSmtp::new(), "555 5.5.4 Parameter not recognised"),
        ("SIZE=1", FakeSmtp::new().without(Extension::Size), "555 5.5.4 SIZE is not offered"),
        ("BODY=8BITMIME", FakeSmtp::new().without(Extension::EightBitMime), "555 5.5.4 BODY is not offered"),
        ("SMTPUTF8", FakeSmtp::new().without(Extension::SmtpUtf8), "555 5.5.4 Parameter not recognised"),
    ];
    for (param, fake, expected) in cases {
        let out = after_sign_in(&fake, &format!("MAIL FROM:<ada@example.org> {param}\r\n"));
        assert_eq!(out, [expected], "{param}");
    }
}

#[test]
fn an_address_that_is_not_ascii_needs_smtputf8() {
    let fake = FakeSmtp::new();
    let outbox = fake.outbox();
    let out = after_sign_in(&fake, "MAIL FROM:<zoé@example.org>\r\nMAIL FROM:<ada@example.org>\r\nRCPT TO:<zoé@example.org>\r\n");
    let refused = "553 5.6.7 The address is not ASCII and SMTPUTF8 was not asked for";
    assert_eq!(out, [refused, "250 2.1.0 Sender OK", refused]);
    let out = after_sign_in(&fake, "MAIL FROM:<zoé@example.org> SMTPUTF8\r\nRCPT TO:<zoé@example.org>\r\nDATA\r\nHi\r\n.\r\n");
    assert_eq!(out.last().map(String::as_str), Some("250 2.0.0 Queued as 1"));
    assert_eq!(outbox.messages().first().map(|sent| sent.recipients().to_vec()), Some(vec!["zoé@example.org".to_owned()]));
}

#[test]
fn recipients_are_checked() {
    let out = after_sign_in(&FakeSmtp::new(), "MAIL FROM:<ada@example.org>\r\nRCPT TO:<>\r\nRCPT TO:<bea@example.org> NOTIFY=NEVER\r\n");
    assert_eq!(out, ["250 2.1.0 Sender OK", "501 5.1.3 Bad recipient address", "555 5.5.4 Parameter not recognised"]);
    let many: String = (0..101).map(|at| format!("RCPT TO:<r{at}@example.org>\r\n")).collect();
    let out = after_sign_in(&FakeSmtp::new(), &format!("MAIL FROM:<ada@example.org>\r\n{many}"));
    assert_eq!(out.last().map(String::as_str), Some("452 4.5.3 Too many recipients"));
    assert_eq!(out.iter().filter(|line| line.starts_with("250 2.1.5")).count(), 100);
}

#[test]
fn a_message_over_the_size_is_read_but_not_kept() {
    let fake = FakeSmtp::new().max_size(16);
    let outbox = fake.outbox();
    let out =
        after_sign_in(&fake, "MAIL FROM:<ada@example.org>\r\nRCPT TO:<bea@example.org>\r\nDATA\r\n0123456789\r\n0123456789\r\nmore\r\n.\r\nNOOP\r\n");
    assert_eq!(out.get(3..), Some(&["552 5.3.4 Message too big".to_owned(), "250 2.0.0 OK".to_owned()][..]));
    assert!(outbox.messages().is_empty());
    let out = lines(&fake.answer(b"EHLO c.example.org\r\n"));
    assert!(out.contains(&"250-SIZE 16".to_owned()), "{out:?}");
    assert_eq!(MAX_SIZE, 10 * 1024 * 1024);
}

#[test]
fn what_cannot_be_read_is_refused() {
    let out = after_sign_in(
        &FakeSmtp::new(),
        "VRFY ada\r\n\r\nEHLO\r\nEHLO a b\r\nDATA now\r\nAUTH\r\nMAIL TO:<ada@example.org>\r\nMAIL FROM:ada@example.org\r\n\
         MAIL FROM:<ada@example.org\r\nMAIL FROM:<a<b@example.org>\r\nMAIL FROM:<ada@example.org>SIZE=1\r\nMAIL FROM:\r\n\u{e9}\r\n",
    );
    let syntax = "501 5.5.4 Expected FROM:<path> or TO:<path>";
    assert_eq!(
        out,
        [
            "500 5.5.2 Command not recognised",
            "500 5.5.2 Command not recognised",
            "501 5.5.4 EHLO and HELO need one domain",
            "501 5.5.4 EHLO and HELO need one domain",
            "501 5.5.4 The command takes no arguments",
            "501 5.5.4 AUTH needs a mechanism",
            syntax,
            syntax,
            syntax,
            syntax,
            syntax,
            syntax,
            "500 5.5.2 Command not recognised",
        ]
    );
    let out = after_sign_in(&FakeSmtp::new(), "MAIL FROM: <ada@example.org>\r\n");
    assert_eq!(out, ["250 2.1.0 Sender OK"]);
}

#[test]
fn a_line_that_is_not_utf8_is_refused() {
    let out = lines(&FakeSmtp::new().answer(b"EHLO \xff\r\n"));
    assert_eq!(out.last().map(String::as_str), Some("501 5.5.4 The line is not UTF-8"));
}

#[test]
fn too_many_refusals_end_the_session() {
    let out = lines(&FakeSmtp::new().answer("BAD\r\n".repeat(25).as_bytes()));
    assert_eq!(out.iter().filter(|line| line.starts_with("500 ")).count(), 20);
    assert_eq!(out.last().map(String::as_str), Some("421 4.7.0 Too many errors"));
}

#[test]
fn a_client_that_goes_in_the_middle_of_a_message_sends_nothing() {
    let fake = FakeSmtp::new();
    let outbox = fake.outbox();
    let out = after_sign_in(&fake, "MAIL FROM:<ada@example.org>\r\nRCPT TO:<bea@example.org>\r\nDATA\r\nhalf a messa");
    assert_eq!(out.last().map(String::as_str), Some("354 End data with <CR><LF>.<CR><LF>"));
    assert!(outbox.messages().is_empty());
}

#[test]
fn a_line_too_long_ends_the_session() {
    let long = "x".repeat(9000);
    for input in [format!("EHLO {long}\r\n"), format!("{SIGN_IN}MAIL FROM:<ada@example.org>\r\nRCPT TO:<b@example.org>\r\nDATA\r\n{long}\r\n")] {
        let out = lines(&FakeSmtp::new().answer(format!("EHLO c.example.org\r\n{input}").as_bytes()));
        assert_eq!(out.last().map(String::as_str), Some("421 4.7.0 Line too long"));
    }
}
