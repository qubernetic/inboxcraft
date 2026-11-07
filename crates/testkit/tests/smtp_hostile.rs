// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake SMTP server on hostile input: any bytes and any mix of command pieces never make it panic, and what it
//! sends is always the greeting first and whole CRLF lines.

use inboxcraft_testkit::imap::FakeImap;
use inboxcraft_testkit::smtp::{Bounce, FakeSmtp};
use proptest::collection::vec;
use proptest::prelude::{ProptestConfig, any, prop_assert, proptest};
use proptest::sample::select;

const PIECES: &[&str] = &[
    "EHLO c.example.org",
    "HELO [192.0.2.1]",
    "AUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk",
    "AUTH PLAIN",
    "AUTH LOGIN YWRhQGV4YW1wbGUub3Jn",
    "AUTH LOGIN",
    "bm90LWEtcmVhbC1wYXNzd29yZA==",
    "*",
    "=",
    "MAIL FROM:<ada@example.org> SIZE=12 BODY=8BITMIME SMTPUTF8",
    "MAIL FROM:<>",
    "MAIL FROM:<zoé@example.org>",
    "RCPT TO:<nobody@example.org>",
    "RCPT TO:<bea@example.org> NOTIFY=NEVER",
    "DATA",
    ".",
    "..",
    "Subject: x",
    "RSET",
    "NOOP",
    "STARTTLS",
    "QUIT",
    "SIZE=99999999999999999999999",
    "<<<>>>",
    ":",
    "\u{0}\u{ff}",
    " ",
    "\r",
];

fn check(input: &[u8]) -> Result<(), proptest::test_runner::TestCaseError> {
    let imap = FakeImap::new();
    let fake = FakeSmtp::new().max_size(64).bounce(Bounce::new("nobody@example.org", &imap.postbox()));
    let out = fake.answer(input);
    prop_assert!(out.starts_with(b"220 smtp.example.org "), "{:?}", String::from_utf8_lossy(&out));
    prop_assert!(out.ends_with(b"\r\n"), "{:?}", String::from_utf8_lossy(&out));
    let _ = imap.answer(b"a1 LOGIN ada@example.org not-a-real-password\r\na2 SELECT INBOX\r\na3 UID FETCH 1:* BODY[]\r\n");
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn any_bytes_never_panic(input in vec(any::<u8>(), 0..512)) {
        check(&input)?;
    }

    #[test]
    fn any_mix_of_command_pieces_never_panics(pieces in vec((select(PIECES), select(&["\r\n", "\n", " ", ""][..])), 0..40)) {
        let input: String = pieces.iter().map(|(piece, end)| format!("{piece}{end}")).collect();
        check(input.as_bytes())?;
    }
}
