// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server on hostile input: any bytes and any mix of command pieces never make it panic, and what it
//! sends is always a greeting first and whole CRLF lines.

use inboxcraft_testkit::imap::{FakeImap, PASSWORD, USER};
use proptest::collection::vec;
use proptest::prelude::{ProptestConfig, any, prop_assert, proptest};
use proptest::sample::select;

const PIECES: &[&str] = &[
    "a1 ",
    "b2 UID ",
    "FETCH 1:* (UID FLAGS INTERNALDATE RFC822.SIZE BODY[] BODY.PEEK[HEADER] PREVIEW (LAZY) MODSEQ)",
    "(CHANGEDSINCE 1 VANISHED)",
    "SELECT INBOX (QRESYNC (1 1 1:9 (1 1)))",
    "EXAMINE Archive (CONDSTORE)",
    "ENABLE QRESYNC CONDSTORE",
    "IDLE\r\nDONE",
    "NOTIFY SET STATUS (SELECTED (MessageNew (UID FLAGS) MessageExpunge FlagChange)) (PERSONAL (MessageNew MessageExpunge))",
    "NOTIFY NONE",
    "MOVE 1:* Archive",
    "COPY 2 INBOX",
    "STORE 1:3 (UNCHANGEDSINCE 2) -FLAGS.SILENT (\\Seen \\Deleted)",
    "EXPUNGE 1:*",
    "SEARCH CHARSET UTF-8 OR NOT SEEN (FROM anna SINCE 1-Jan-2026) HEADER List-Id builds",
    "LIST \"\" * RETURN (SPECIAL-USE)",
    "STATUS INBOX (MESSAGES UIDNEXT UNSEEN HIGHESTMODSEQ)",
    "AUTHENTICATE PLAIN\r\n*",
    "LOGIN {15}\r\nada@example.org x",
    "{5+}\r\nab",
    "\"quoted \\\" string\"",
    "((((",
    ")",
    "*",
    "\r\n",
    " ",
    "\u{0}\u{ff}",
    "CLOSE",
    "COMPRESS DEFLATE",
    "LOGOUT",
];

fn check(input: &[u8]) -> Result<(), proptest::test_runner::TestCaseError> {
    let fake = FakeImap::sample();
    let signed_in = [format!("s1 LOGIN {USER} {PASSWORD}\r\ns2 SELECT INBOX\r\n").as_bytes(), input].concat();
    for input in [input, &signed_in] {
        let out = fake.answer(input);
        prop_assert!(out.starts_with(b"* OK [CAPABILITY "), "{:?}", String::from_utf8_lossy(&out));
        prop_assert!(out.ends_with(b"\r\n"), "{:?}", String::from_utf8_lossy(&out));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn hostile_bytes_never_panic(data in vec(any::<u8>(), 0..2048)) {
        check(&data)?;
    }

    #[test]
    fn hostile_commands_never_panic(pieces in vec(select(PIECES), 0..24)) {
        check(pieces.concat().as_bytes())?;
    }
}
