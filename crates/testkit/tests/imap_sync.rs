// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server's sync extensions: ENABLE, CONDSTORE and QRESYNC.

use inboxcraft_testkit::imap::{Capability, FakeImap, Mailbox, Message, PASSWORD, USER};

fn signed_in(fake: &FakeImap, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(format!("s1 LOGIN {USER} {PASSWORD}\r\n{input}").as_bytes())).into_owned();
    out.split_terminator("\r\n").skip(2).map(str::to_owned).collect()
}

/// The lines after the answer to the `SELECT` or `EXAMINE` with tag `a1`.
fn after_select(got: &[String]) -> Vec<String> {
    got.iter().skip_while(|line| !line.starts_with("a1 ")).skip(1).cloned().collect()
}

/// INBOX with UIDs 1 and 2 (MODSEQ 2 and 3), UID 3 removed (MODSEQ 4) and UID 4 (MODSEQ 5), UIDVALIDITY 9.
fn resync_fake() -> FakeImap {
    FakeImap::new().mailbox(
        Mailbox::new("INBOX")
            .uid_validity(9)
            .message(Message::new("Subject: one\n\n1\n").flags(&["\\Seen"]))
            .message(Message::new("Subject: two\n\n2\n"))
            .removed(&[3])
            .message(Message::new("Subject: four\n\n4\n")),
    )
}

#[test]
fn enable_names_what_it_enabled() {
    let fake = FakeImap::sample();
    let got = signed_in(&fake, "a1 ENABLE CONDSTORE QRESYNC X-UNKNOWN\r\na2 ENABLE X-UNKNOWN\r\na3 ENABLE\r\na4 ENABLE \"CONDSTORE\"\r\n");
    assert_eq!(
        got,
        [
            "* ENABLED CONDSTORE QRESYNC",
            "a1 OK ENABLE completed",
            "* ENABLED",
            "a2 OK ENABLE completed",
            "a3 BAD ENABLE needs capabilities",
            "* ENABLED",
            "a4 OK ENABLE completed"
        ]
    );
    let off = signed_in(&fake.clone().without(Capability::QResync).without(Capability::CondStore), "a1 ENABLE QRESYNC CONDSTORE\r\n");
    assert_eq!(off, ["* ENABLED", "a1 OK ENABLE completed"]);
    let no_enable = signed_in(&fake.clone().without(Capability::Enable), "a1 ENABLE QRESYNC\r\n");
    assert_eq!(no_enable, ["a1 BAD ENABLE is not available now"]);
    let selected = signed_in(&fake, "a0 SELECT INBOX\r\na1 ENABLE QRESYNC\r\n");
    assert_eq!(selected.last().map(String::as_str), Some("a1 BAD ENABLE is not available now"));
}

#[test]
fn the_modseq_item_and_changedsince() {
    let got = signed_in(&FakeImap::sample(), "a0 EXAMINE INBOX\r\na1 UID FETCH 1:* (UID MODSEQ)\r\na2 UID FETCH 1:* (FLAGS) (CHANGEDSINCE 2)\r\n");
    let tail: Vec<String> = got.into_iter().skip(9).collect();
    assert_eq!(
        tail,
        [
            "* 1 FETCH (UID 1 MODSEQ (2))",
            "* 2 FETCH (UID 2 MODSEQ (3))",
            "* 3 FETCH (UID 3 MODSEQ (4))",
            "a1 OK UID FETCH completed",
            "* 2 FETCH (UID 2 FLAGS () MODSEQ (3))",
            "* 3 FETCH (UID 3 FLAGS (\\Flagged) MODSEQ (4))",
            "a2 OK UID FETCH completed",
        ]
    );
}

#[test]
fn marking_a_message_seen_raises_its_modseq() {
    let got = signed_in(
        &FakeImap::sample(),
        "a0 SELECT INBOX (CONDSTORE)\r\na1 UID FETCH 2 BODY[TEXT]\r\na2 UID FETCH 1:* FLAGS (CHANGEDSINCE 4)\r\na3 STATUS INBOX (HIGHESTMODSEQ)\r\n",
    );
    let tail: Vec<String> = got.into_iter().skip(9).collect();
    assert_eq!(
        tail,
        [
            "* 2 FETCH (UID 2 BODY[TEXT] {23}",
            "All 412 tests passed.",
            " FLAGS (\\Seen) MODSEQ (5))",
            "a1 OK UID FETCH completed",
            "* 2 FETCH (UID 2 FLAGS (\\Seen) MODSEQ (5))",
            "a2 OK UID FETCH completed",
            "* STATUS \"INBOX\" (HIGHESTMODSEQ 5)",
            "a3 OK STATUS completed",
        ]
    );
}

#[test]
fn select_parameters_need_their_capability() {
    let fake = FakeImap::sample();
    let cases = [
        ("a1 SELECT INBOX x", "a1 BAD Bad SELECT parameters"),
        ("a1 SELECT INBOX (CONDSTORE) x", "a1 BAD Bad SELECT parameters"),
        ("a1 EXAMINE INBOX (FROB)", "a1 BAD Unknown EXAMINE parameter"),
        ("a1 SELECT INBOX (QRESYNC (9 1))", "a1 BAD QRESYNC is not enabled"),
    ];
    for (line, want) in cases {
        assert_eq!(signed_in(&fake, &format!("{line}\r\n")), [want], "{line}");
    }
    let off = signed_in(&fake.clone().without(Capability::CondStore), "a1 SELECT INBOX (CONDSTORE)\r\n");
    assert_eq!(off, ["a1 BAD Unknown SELECT parameter"]);
    let ok = signed_in(&fake, "a1 SELECT INBOX (CONDSTORE)\r\n");
    assert_eq!(ok.last().map(String::as_str), Some("a1 OK [READ-WRITE] SELECT completed"));
}

#[test]
fn condstore_switched_off_refuses_its_fetch_parts() {
    let fake = FakeImap::sample().without(Capability::CondStore);
    let got = signed_in(&fake, "a0 EXAMINE INBOX\r\na1 UID FETCH 1 MODSEQ\r\na2 UID FETCH 1 FLAGS (CHANGEDSINCE 1)\r\n");
    let tail: Vec<String> = got.into_iter().skip(8).collect();
    assert_eq!(tail, ["a1 BAD Unknown FETCH item", "a2 BAD Unknown FETCH modifier"]);
}

#[test]
fn fetch_modifiers_are_checked() {
    let fake = FakeImap::sample();
    let cases = [
        ("UID FETCH 1 FLAGS (CHANGEDSINCE)", "a2 BAD Bad CHANGEDSINCE"),
        ("UID FETCH 1 FLAGS (CHANGEDSINCE x)", "a2 BAD Bad CHANGEDSINCE"),
        ("UID FETCH 1 FLAGS (VANISHED)", "a2 BAD VANISHED needs QRESYNC and CHANGEDSINCE"),
        ("UID FETCH 1 FLAGS (CHANGEDSINCE 1 VANISHED)", "a2 BAD VANISHED needs QRESYNC and CHANGEDSINCE"),
        ("UID FETCH 1 FLAGS (FROB)", "a2 BAD Unknown FETCH modifier"),
        ("UID FETCH 1 FLAGS x", "a2 BAD UID FETCH needs a UID set and items"),
    ];
    for (line, want) in cases {
        let got = signed_in(&fake, &format!("a1 EXAMINE INBOX\r\na2 {line}\r\n"));
        assert_eq!(got.last().map(String::as_str), Some(want), "{line}");
    }
}

#[test]
fn qresync_select_reports_what_vanished_and_what_changed() {
    let got = signed_in(&resync_fake(), "a0 ENABLE QRESYNC\r\na1 SELECT INBOX (QRESYNC (9 3))\r\n");
    let select: Vec<&str> = got.iter().skip(2).map(String::as_str).collect();
    assert_eq!(
        select,
        [
            "* 3 EXISTS",
            "* 0 RECENT",
            "* OK [UNSEEN 2] First unseen",
            "* OK [UIDVALIDITY 9] UIDs valid",
            "* OK [UIDNEXT 5] Predicted next UID",
            "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)",
            "* OK [PERMANENTFLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft \\*)] Flags allowed",
            "* OK [HIGHESTMODSEQ 5] Highest",
            "* VANISHED (EARLIER) 3",
            "* 3 FETCH (UID 4 FLAGS () MODSEQ (5))",
            "a1 OK [READ-WRITE] SELECT completed",
        ]
    );
}

#[test]
fn qresync_limits_vanished_to_the_known_uids_and_ignores_another_uidvalidity() {
    let fake = resync_fake();
    let known = signed_in(&fake, "a0 ENABLE QRESYNC\r\na1 SELECT INBOX (QRESYNC (9 1 1:2 (1:2 1:2)))\r\n");
    assert!(!known.iter().any(|line| line.starts_with("* VANISHED")), "{known:?}");
    assert_eq!(known.iter().filter(|line| line.contains(" FETCH ")).count(), 3, "{known:?}");
    let all = signed_in(&fake, "a0 ENABLE QRESYNC\r\na1 SELECT INBOX (QRESYNC (9 1 1:4))\r\n");
    assert!(all.contains(&"* VANISHED (EARLIER) 3".to_owned()), "{all:?}");
    let other = signed_in(&fake, "a0 ENABLE QRESYNC\r\na1 SELECT INBOX (QRESYNC (8 1))\r\n");
    assert!(!other.iter().any(|line| line.starts_with("* VANISHED") || line.contains(" FETCH ")), "{other:?}");
    for bad in ["(QRESYNC)", "(QRESYNC (9))", "(QRESYNC (x 1))", "(QRESYNC (9 1 0))", "(QRESYNC 9)"] {
        let got = signed_in(&fake, &format!("a0 ENABLE QRESYNC\r\na1 SELECT INBOX {bad}\r\n"));
        assert_eq!(got.last().map(String::as_str), Some("a1 BAD Bad QRESYNC parameter"), "{bad}");
    }
}

#[test]
fn qresync_closes_the_previous_mailbox_first() {
    let got = signed_in(&FakeImap::sample(), "a0 ENABLE QRESYNC\r\na1 SELECT INBOX\r\na2 SELECT Archive\r\n");
    let second: Vec<&str> = got.iter().skip_while(|line| !line.starts_with("a1 ")).skip(1).map(String::as_str).collect();
    assert_eq!(second.first().copied(), Some("* OK [CLOSED] Previous mailbox closed"));
    let without = signed_in(&FakeImap::sample(), "a1 SELECT INBOX\r\na2 SELECT Archive\r\n");
    assert!(!without.iter().any(|line| line.contains("[CLOSED]")));
}

#[test]
fn uid_fetch_vanished_reports_removed_uids_in_the_set() {
    let got = signed_in(
        &resync_fake(),
        "a0 ENABLE QRESYNC\r\na1 SELECT INBOX\r\na2 UID FETCH 1:* FLAGS (CHANGEDSINCE 2 VANISHED)\r\na3 UID FETCH 4 FLAGS (CHANGEDSINCE 2 VANISHED)\r\n",
    );
    let tail = after_select(&got);
    assert_eq!(
        tail,
        [
            "* VANISHED (EARLIER) 3",
            "* 2 FETCH (UID 2 FLAGS () MODSEQ (3))",
            "* 3 FETCH (UID 4 FLAGS () MODSEQ (5))",
            "a2 OK UID FETCH completed",
            "* 3 FETCH (UID 4 FLAGS () MODSEQ (5))",
            "a3 OK UID FETCH completed",
        ]
    );
}

#[test]
fn close_records_what_it_removed_for_qresync() {
    let fake = FakeImap::new().mailbox(
        Mailbox::new("INBOX")
            .message(Message::new("Subject: a\n\nx\n").flags(&["\\Deleted"]))
            .message(Message::new("Subject: b\n\ny\n").flags(&["\\Deleted"])),
    );
    let got = signed_in(&fake, "a0 ENABLE QRESYNC\r\nb1 SELECT INBOX\r\nb2 CLOSE\r\na1 SELECT INBOX (QRESYNC (1 3))\r\n");
    assert!(got.contains(&"* VANISHED (EARLIER) 1:2".to_owned()), "{got:?}");
}

#[test]
fn removed_uids_raise_the_next_uid() {
    let fake = FakeImap::new().mailbox(Mailbox::new("INBOX").removed(&[7, 5, 6]).message(Message::new("Subject: a\n\nx\n")));
    let got = signed_in(&fake, "a0 ENABLE QRESYNC\r\na1 SELECT INBOX (QRESYNC (1 1))\r\n");
    assert!(got.contains(&"* VANISHED (EARLIER) 5:7".to_owned()), "{got:?}");
    assert!(got.contains(&"* 1 FETCH (UID 8 FLAGS () MODSEQ (5))".to_owned()), "{got:?}");
}
