// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server's write commands and E1.4c extensions: UID STORE, UID COPY, UID MOVE, UID EXPUNGE
//! (UIDPLUS), EXPUNGE and PREVIEW.

use inboxcraft_testkit::imap::{Capability, FakeImap, Mailbox, Message, PASSWORD, USER};

fn signed_in(fake: &FakeImap, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(format!("s1 LOGIN {USER} {PASSWORD}\r\n{input}").as_bytes())).into_owned();
    out.split_terminator("\r\n").skip(2).map(str::to_owned).collect()
}

/// The lines after the tagged answer to the command tagged `a1`.
fn after_a1(got: &[String]) -> Vec<String> {
    got.iter().skip_while(|line| !line.starts_with("a1 ")).skip(1).cloned().collect()
}

#[test]
fn uid_store_sets_adds_and_removes_flags() {
    let input = "a1 SELECT INBOX\r\na2 UID STORE 1:2 +FLAGS (\\Flagged)\r\na3 UID STORE 1 -FLAGS.SILENT (\\Seen)\r\na4 UID STORE 3 FLAGS \\Answered $Label1\r\na5 UID FETCH 1:* FLAGS\r\n";
    let got = signed_in(&FakeImap::sample(), input);
    assert_eq!(
        after_a1(&got),
        [
            "* 1 FETCH (UID 1 FLAGS (\\Seen \\Flagged))",
            "* 2 FETCH (UID 2 FLAGS (\\Flagged))",
            "a2 OK UID STORE completed",
            "a3 OK UID STORE completed",
            "* 3 FETCH (UID 3 FLAGS (\\Answered $Label1))",
            "a4 OK UID STORE completed",
            "* 1 FETCH (UID 1 FLAGS (\\Flagged))",
            "* 2 FETCH (UID 2 FLAGS (\\Flagged))",
            "* 3 FETCH (UID 3 FLAGS (\\Answered $Label1))",
            "a5 OK UID FETCH completed",
        ]
    );
}

#[test]
fn uid_store_unchangedsince_leaves_newer_messages_alone() {
    let input = "a1 SELECT INBOX (CONDSTORE)\r\na2 UID STORE 1:3 (UNCHANGEDSINCE 3) +FLAGS (\\Deleted)\r\na3 UID STORE 1 +FLAGS (\\Deleted)\r\n";
    let got = signed_in(&FakeImap::sample(), input);
    assert_eq!(
        after_a1(&got),
        [
            "* 1 FETCH (UID 1 FLAGS (\\Seen \\Deleted) MODSEQ (5))",
            "* 2 FETCH (UID 2 FLAGS (\\Deleted) MODSEQ (6))",
            "a2 OK [MODIFIED 3] Conditional STORE failed",
            "* 1 FETCH (UID 1 FLAGS (\\Seen \\Deleted) MODSEQ (5))",
            "a3 OK UID STORE completed",
        ]
    );
}

#[test]
fn uid_store_refuses_what_it_cannot_do() {
    let fake = FakeImap::sample();
    let cases = [
        ("a1 SELECT INBOX\r\na2 UID STORE 1 FLAGS", "a2 BAD UID STORE needs a UID set, an operation and flags"),
        ("a1 SELECT INBOX\r\na2 UID STORE 1 FROB (\\Seen)", "a2 BAD UID STORE needs a UID set, an operation and flags"),
        ("a1 SELECT INBOX\r\na2 UID STORE 1 FLAGS (\"x\")", "a2 BAD UID STORE needs a UID set, an operation and flags"),
        ("a1 SELECT INBOX\r\na2 UID STORE 0 FLAGS ()", "a2 BAD Bad UID set"),
        ("a1 SELECT INBOX\r\na2 UID STORE 1 (UNCHANGEDSINCE x) FLAGS ()", "a2 BAD Bad UNCHANGEDSINCE"),
        ("a1 SELECT INBOX\r\na2 UID STORE 1 (FROB 1) FLAGS ()", "a2 BAD Bad UNCHANGEDSINCE"),
        ("a1 EXAMINE INBOX\r\na2 UID STORE 1 FLAGS ()", "a2 NO [READ-ONLY] The mailbox is open read-only"),
    ];
    for (input, want) in cases {
        assert_eq!(signed_in(&fake, &format!("{input}\r\n")).last().map(String::as_str), Some(want), "{input}");
    }
    let off = signed_in(&fake.without(Capability::CondStore), "a1 SELECT INBOX\r\na2 UID STORE 1 (UNCHANGEDSINCE 1) FLAGS ()\r\n");
    assert_eq!(off.last().map(String::as_str), Some("a2 BAD Bad UNCHANGEDSINCE"));
}

#[test]
fn uid_copy_reports_copyuid() {
    let got = signed_in(
        &FakeImap::sample(),
        "a1 SELECT INBOX\r\na2 UID COPY 2:3 Archive\r\na3 UID COPY 9 Archive\r\na4 STATUS Archive (MESSAGES UIDNEXT)\r\n",
    );
    assert_eq!(
        after_a1(&got),
        [
            "a2 OK [COPYUID 7 2:3 13:14] UID COPY completed",
            "a3 OK UID COPY completed",
            "* STATUS \"Archive\" (MESSAGES 3 UIDNEXT 15)",
            "a4 OK STATUS completed",
        ]
    );
    let plain = signed_in(&FakeImap::sample().without(Capability::UidPlus), "a1 SELECT INBOX\r\na2 UID COPY 1 Archive\r\n");
    assert_eq!(after_a1(&plain), ["a2 OK UID COPY completed"]);
}

#[test]
fn uid_copy_into_the_open_mailbox_tells_of_the_new_messages() {
    let got = signed_in(&FakeImap::sample(), "a1 EXAMINE INBOX\r\na2 UID COPY 1 INBOX\r\n");
    assert_eq!(after_a1(&got), ["* 4 EXISTS", "a2 OK [COPYUID 1 1 4] UID COPY completed"]);
}

#[test]
fn uid_copy_and_move_need_a_mailbox_that_exists() {
    let fake = FakeImap::sample();
    let cases = [
        ("a2 UID COPY 1 Nowhere", "a2 NO [TRYCREATE] No such mailbox"),
        ("a2 UID MOVE 1 Nowhere", "a2 NO [TRYCREATE] No such mailbox"),
        ("a2 UID COPY 1", "a2 BAD UID COPY needs a UID set and a mailbox"),
        ("a2 UID MOVE x Archive", "a2 BAD Bad UID set"),
    ];
    for (line, want) in cases {
        assert_eq!(signed_in(&fake, &format!("a1 SELECT INBOX\r\n{line}\r\n")).last().map(String::as_str), Some(want), "{line}");
    }
}

#[test]
fn uid_move_copies_then_expunges() {
    let got = signed_in(&FakeImap::sample(), "a1 SELECT INBOX\r\na2 UID MOVE 1,3 Archive\r\na3 UID FETCH 1:* UID\r\n");
    assert_eq!(
        after_a1(&got),
        [
            "* OK [COPYUID 7 1,3 13:14] Moved",
            "* 1 EXPUNGE",
            "* 2 EXPUNGE",
            "a2 OK UID MOVE completed",
            "* 1 FETCH (UID 2)",
            "a3 OK UID FETCH completed"
        ]
    );
    let qresync = signed_in(&FakeImap::sample(), "a0 ENABLE QRESYNC\r\na1 SELECT INBOX\r\na2 UID MOVE 1:2 Archive\r\n");
    assert_eq!(after_a1(&qresync), ["* OK [COPYUID 7 1:2 13:14] Moved", "* VANISHED 1", "* VANISHED 2", "a2 OK UID MOVE completed"]);
    let none = signed_in(&FakeImap::sample(), "a1 SELECT INBOX\r\na2 UID MOVE 9 Archive\r\n");
    assert_eq!(after_a1(&none), ["a2 OK UID MOVE completed"]);
}

#[test]
fn uid_move_needs_its_capability_and_a_writable_mailbox() {
    let off = signed_in(&FakeImap::sample().without(Capability::Move), "a1 SELECT INBOX\r\na2 UID MOVE 1 Archive\r\n");
    assert_eq!(after_a1(&off), ["a2 BAD UID MOVE is not available now"]);
    let read_only = signed_in(&FakeImap::sample(), "a1 EXAMINE INBOX\r\na2 UID MOVE 1 Archive\r\n");
    assert_eq!(after_a1(&read_only), ["a2 NO [READ-ONLY] The mailbox is open read-only"]);
    let plain = signed_in(&FakeImap::sample().without(Capability::UidPlus), "a1 SELECT INBOX\r\na2 UID MOVE 1 Archive\r\n");
    assert_eq!(after_a1(&plain), ["* 1 EXPUNGE", "a2 OK UID MOVE completed"]);
}

#[test]
fn the_move_fallback_of_copy_store_and_uid_expunge() {
    let input = "a1 SELECT INBOX\r\na2 UID COPY 2 Archive\r\na3 UID STORE 2 +FLAGS.SILENT (\\Deleted)\r\na4 UID EXPUNGE 2\r\na5 UID SEARCH ALL\r\n";
    let got = signed_in(&FakeImap::sample().without(Capability::Move), input);
    assert_eq!(
        after_a1(&got),
        [
            "a2 OK [COPYUID 7 2 13] UID COPY completed",
            "a3 OK UID STORE completed",
            "* 2 EXPUNGE",
            "a4 OK UID EXPUNGE completed",
            "* SEARCH 1 3",
            "a5 OK UID SEARCH completed",
        ]
    );
}

#[test]
fn uid_expunge_removes_only_deleted_messages_in_its_set() {
    let fake = FakeImap::new().mailbox(
        Mailbox::new("INBOX")
            .message(Message::new("Subject: a\n\nx\n").flags(&["\\Deleted"]))
            .message(Message::new("Subject: b\n\ny\n").flags(&["\\Deleted"]))
            .message(Message::new("Subject: c\n\nz\n")),
    );
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 UID EXPUNGE 2:3\r\na3 EXPUNGE\r\na4 EXPUNGE\r\n");
    assert_eq!(after_a1(&got), ["* 2 EXPUNGE", "a2 OK UID EXPUNGE completed", "* 1 EXPUNGE", "a3 OK EXPUNGE completed", "a4 OK EXPUNGE completed"]);
    let cases = [
        ("a1 SELECT INBOX\r\na2 UID EXPUNGE", "a2 BAD UID EXPUNGE needs a UID set"),
        ("a1 SELECT INBOX\r\na2 UID EXPUNGE 0", "a2 BAD Bad UID set"),
        ("a1 EXAMINE INBOX\r\na2 UID EXPUNGE 1", "a2 NO [READ-ONLY] The mailbox is open read-only"),
        ("a1 EXAMINE INBOX\r\na2 EXPUNGE", "a2 NO [READ-ONLY] The mailbox is open read-only"),
    ];
    for (input, want) in cases {
        assert_eq!(signed_in(&fake, &format!("{input}\r\n")).last().map(String::as_str), Some(want), "{input}");
    }
    let off = signed_in(&fake.without(Capability::UidPlus), "a1 SELECT INBOX\r\na2 UID EXPUNGE 1\r\n");
    assert_eq!(off.last().map(String::as_str), Some("a2 BAD UID EXPUNGE is not available now"));
}

#[test]
fn preview_gives_one_short_line_of_the_text() {
    let long = format!("Subject: long\n\n{}\n", "word ".repeat(100));
    let fake = FakeImap::new().mailbox(
        Mailbox::new("INBOX")
            .message(Message::new("Subject: a\n\nFirst line\n\n   second  line\n"))
            .message(Message::new("Subject: b\n\n"))
            .message(Message::new("Subject: c\n\nx\n").preview("Set by the \"test\""))
            .message(Message::new("Subject: d\n\nÁrvíztűrő tükörfúrógép\n"))
            .message(Message::new(&long)),
    );
    let got =
        signed_in(&fake, "a1 EXAMINE INBOX\r\na2 UID FETCH 1:4 (PREVIEW)\r\na3 UID FETCH 1 (UID PREVIEW (LAZY) FLAGS)\r\na4 UID FETCH 5 PREVIEW\r\n");
    let tail = after_a1(&got);
    assert_eq!(
        tail.get(..9),
        Some(
            [
                "* 1 FETCH (UID 1 PREVIEW \"First line second line\")",
                "* 2 FETCH (UID 2 PREVIEW NIL)",
                "* 3 FETCH (UID 3 PREVIEW \"Set by the \\\"test\\\"\")",
                "* 4 FETCH (UID 4 PREVIEW {31}",
                "Árvíztűrő tükörfúrógép)",
                "a2 OK UID FETCH completed",
                "* 1 FETCH (UID 1 PREVIEW \"First line second line\" FLAGS ())",
                "a3 OK UID FETCH completed",
                &format!("* 5 FETCH (UID 5 PREVIEW \"{}\")", "word ".repeat(40).trim_end()),
            ]
            .map(|line| line.to_owned())
            .as_slice()
        )
    );
}

#[test]
fn preview_needs_its_capability() {
    let fake = FakeImap::sample();
    let off = signed_in(&fake.clone().without(Capability::Preview), "a1 EXAMINE INBOX\r\na2 UID FETCH 1 PREVIEW\r\n");
    assert_eq!(after_a1(&off), ["a2 BAD Unknown FETCH item"]);
    let bad = signed_in(&fake, "a1 EXAMINE INBOX\r\na2 UID FETCH 1 (PREVIEW (FAST))\r\n");
    assert_eq!(after_a1(&bad), ["a2 BAD Unknown FETCH item"]);
}
