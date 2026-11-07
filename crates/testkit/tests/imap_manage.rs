// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server's mailbox management: CREATE, DELETE and APPEND, which the live-shaped tests of `net` play
//! against the fake before the owner plays them against a real server.

use inboxcraft_testkit::imap::{Capability, FakeImap, PASSWORD, USER};

fn signed_in(fake: &FakeImap, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(format!("s1 LOGIN {USER} {PASSWORD}\r\n{input}").as_bytes())).into_owned();
    out.split_terminator("\r\n").skip(2).map(str::to_owned).collect()
}

/// The tagged answers in `got`.
fn tagged(got: &[String]) -> Vec<String> {
    got.iter().filter(|line| !line.starts_with('*') && !line.starts_with('+')).cloned().collect()
}

#[test]
fn create_makes_a_mailbox_list_shows() {
    let got = signed_in(&FakeImap::new(), "a1 CREATE \"InboxCraft-Test/run 1\"\r\na2 LIST \"\" \"InboxCraft-Test/*\"\r\na3 CREATE INBOX\r\n");
    assert_eq!(
        got,
        [
            "a1 OK CREATE completed",
            "* LIST (\\HasNoChildren) \"/\" \"InboxCraft-Test/run 1\"",
            "a2 OK LIST completed",
            "a3 NO [ALREADYEXISTS] The mailbox exists",
        ]
    );
}

#[test]
fn a_new_mailbox_is_empty_and_has_its_own_uidvalidity() {
    let got = signed_in(&FakeImap::new(), "a1 CREATE Fresh\r\na2 EXAMINE Fresh\r\n");
    assert!(got.contains(&"* 0 EXISTS".to_owned()), "{got:?}");
    assert!(got.iter().any(|line| line.starts_with("* OK [UIDVALIDITY ") && !line.starts_with("* OK [UIDVALIDITY 1]")), "{got:?}");
    assert!(got.contains(&"* OK [UIDNEXT 1] Predicted next UID".to_owned()), "{got:?}");
}

#[test]
fn create_and_delete_need_a_name() {
    let got = signed_in(&FakeImap::new(), "a1 CREATE\r\na2 DELETE\r\na3 CREATE \"\"\r\n");
    assert_eq!(tagged(&got), ["a1 BAD CREATE needs a mailbox name", "a2 BAD DELETE needs a mailbox name", "a3 BAD CREATE needs a mailbox name"]);
}

#[test]
fn delete_removes_a_mailbox_but_not_inbox_nor_the_open_one() {
    let input = "a1 CREATE Gone\r\na2 DELETE Gone\r\na3 EXAMINE Gone\r\na4 DELETE Gone\r\na5 DELETE inbox\r\na6 CREATE Open\r\na7 EXAMINE Open\r\na8 DELETE Open\r\n";
    let got = signed_in(&FakeImap::new(), input);
    assert_eq!(
        tagged(&got),
        [
            "a1 OK CREATE completed",
            "a2 OK DELETE completed",
            "a3 NO [NONEXISTENT] No such mailbox",
            "a4 NO [NONEXISTENT] No such mailbox",
            "a5 NO [CANNOT] INBOX cannot be deleted",
            "a6 OK CREATE completed",
            "a7 OK [READ-ONLY] EXAMINE completed",
            "a8 NO [INUSE] The mailbox is open",
        ]
    );
}

#[test]
fn deleting_a_mailbox_before_the_open_one_keeps_it_open() {
    let input = "a1 CREATE First\r\na2 CREATE Second\r\na3 SELECT Second\r\na4 DELETE First\r\na5 APPEND Second {12}\r\nSubject: x\r\n\r\na6 UID FETCH 1:* FLAGS\r\n";
    let got = signed_in(&FakeImap::new(), input);
    assert!(got.contains(&"a4 OK DELETE completed".to_owned()), "{got:?}");
    assert!(got.contains(&"* 1 EXISTS".to_owned()), "{got:?}");
    assert!(got.contains(&"* 1 FETCH (UID 1 FLAGS ())".to_owned()), "{got:?}");
}

#[test]
fn append_adds_a_message_with_flags_and_date_and_tells_its_uid() {
    let message = "From: anna@example.org\r\nSubject: Appended\r\n\r\nInvented text.\r\n";
    let input = format!(
        "a1 APPEND INBOX (\\Seen) \"02-Nov-2026 09:30:00 +0000\" {{{}}}\r\n{message}\r\na2 SELECT INBOX\r\na3 UID FETCH 1:* (FLAGS INTERNALDATE)\r\n",
        message.len()
    );
    let got = signed_in(&FakeImap::new(), &input);
    assert!(got.contains(&"+ Ready for the literal".to_owned()), "{got:?}");
    assert!(got.contains(&"a1 OK [APPENDUID 1 1] APPEND completed".to_owned()), "{got:?}");
    assert!(got.contains(&"* 1 FETCH (UID 1 FLAGS (\\Seen) INTERNALDATE \"02-Nov-2026 09:30:00 +0000\")".to_owned()), "{got:?}");
}

#[test]
fn append_without_flags_or_date_and_without_uidplus() {
    let input = "a1 APPEND INBOX {12}\r\nSubject: x\r\n\r\n";
    let got = signed_in(&FakeImap::new().without(Capability::UidPlus), input);
    assert_eq!(tagged(&got), ["a1 OK APPEND completed"]);
    let got = signed_in(&FakeImap::new(), "a1 APPEND INBOX \"02-Nov-2026 09:30:00 +0000\" {12}\r\nSubject: x\r\n\r\n");
    assert_eq!(tagged(&got), ["a1 OK [APPENDUID 1 1] APPEND completed"]);
}

#[test]
fn append_to_the_open_mailbox_tells_of_it_at_the_next_command() {
    let input = "a1 SELECT INBOX\r\na2 APPEND INBOX {12}\r\nSubject: x\r\n\r\na3 NOOP\r\n";
    let got = signed_in(&FakeImap::new(), input);
    let after = got.iter().skip_while(|line| !line.starts_with("a2 ")).cloned().collect::<Vec<_>>();
    assert_eq!(after, ["a2 OK [APPENDUID 1 1] APPEND completed", "* 1 EXISTS", "a3 OK NOOP completed"]);
}

#[test]
fn append_refuses_a_missing_mailbox_and_bad_arguments() {
    let input = "a1 APPEND Nowhere {12}\r\nSubject: x\r\n\r\na2 APPEND INBOX\r\na3 APPEND INBOX (\\Seen) Atom\r\na4 APPEND INBOX (\\Seen) \"date\" \"x\" \"y\"\r\n";
    let got = signed_in(&FakeImap::new(), input);
    assert_eq!(
        tagged(&got),
        [
            "a1 NO [TRYCREATE] No such mailbox",
            "a2 BAD APPEND needs a mailbox and a message",
            "a3 BAD APPEND needs a mailbox and a message",
            "a4 BAD APPEND needs a mailbox and a message",
        ]
    );
}

#[test]
fn the_management_commands_need_a_sign_in() {
    let out = FakeImap::new().answer(b"a1 CREATE Fresh\r\na2 DELETE Fresh\r\na3 APPEND INBOX \"x\"\r\n");
    let out = String::from_utf8_lossy(&out);
    assert!(out.contains("a1 BAD CREATE is not available now"), "{out}");
    assert!(out.contains("a2 BAD DELETE is not available now"), "{out}");
    assert!(out.contains("a3 BAD APPEND is not available now"), "{out}");
}
