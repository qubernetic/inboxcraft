// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server's mailboxes: the invented fixtures, LIST with SPECIAL-USE, SELECT, EXAMINE, STATUS, CLOSE,
//! UID FETCH and UID SEARCH.

use inboxcraft_testkit::imap::{Capability, FakeImap, Mailbox, Message, PASSWORD, USER};

fn lines(fake: &FakeImap, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(input.as_bytes())).into_owned();
    out.split_terminator("\r\n").map(str::to_owned).collect()
}

/// What the fake answers after the greeting and a sign-in.
fn signed_in(fake: &FakeImap, input: &str) -> Vec<String> {
    lines(fake, &format!("s1 LOGIN {USER} {PASSWORD}\r\n{input}")).into_iter().skip(2).collect()
}

#[test]
fn list_shows_the_sample_mailboxes_with_their_special_use() {
    let got = signed_in(&FakeImap::sample(), "a1 LIST \"\" \"*\" RETURN (SPECIAL-USE)\r\n");
    assert_eq!(
        got,
        [
            "* LIST (\\HasNoChildren) \"/\" \"INBOX\"",
            "* LIST (\\HasNoChildren \\Sent) \"/\" \"Sent\"",
            "* LIST (\\HasNoChildren \\Drafts) \"/\" \"Drafts\"",
            "* LIST (\\HasNoChildren \\Archive) \"/\" \"Archive\"",
            "* LIST (\\HasNoChildren \\Junk) \"/\" \"Junk\"",
            "* LIST (\\HasNoChildren \\Trash) \"/\" \"Trash\"",
            "* LIST (\\HasChildren) \"/\" \"Lists\"",
            "* LIST (\\HasNoChildren) \"/\" \"Lists/Rust\"",
            "* LIST (\\HasNoChildren) \"/\" {11}",
            "Elküldött",
            "a1 OK LIST completed",
        ]
    );
}

#[test]
fn list_matches_patterns_and_the_hierarchy_root() {
    let fake = FakeImap::sample();
    let got = signed_in(&fake, "a1 LIST \"\" %\r\na2 LIST Lists/ %\r\na3 LIST \"\" \"\"\r\na4 list \"\" inbox\r\na5 LIST \"\" *u*\r\n");
    let names: Vec<&str> = got.iter().filter_map(|line| line.rsplit(' ').next()).collect();
    assert_eq!(
        names,
        [
            "\"INBOX\"",
            "\"Sent\"",
            "\"Drafts\"",
            "\"Archive\"",
            "\"Junk\"",
            "\"Trash\"",
            "\"Lists\"",
            "{11}",
            "Elküldött",
            "completed",
            "\"Lists/Rust\"",
            "completed",
            "\"\"",
            "completed",
            "\"INBOX\"",
            "completed",
            "\"Junk\"",
            "\"Lists/Rust\"",
            "completed",
        ]
    );
    assert_eq!(got.get(12).map(String::as_str), Some("* LIST (\\Noselect) \"/\" \"\""));
}

#[test]
fn without_special_use_list_shows_no_roles_and_refuses_to_return_them() {
    let fake = FakeImap::sample().without(Capability::SpecialUse);
    let got = signed_in(&fake, "a1 LIST \"\" Sent\r\na2 LIST \"\" * RETURN (SPECIAL-USE)\r\n");
    assert_eq!(got, ["* LIST (\\HasNoChildren) \"/\" \"Sent\"", "a1 OK LIST completed", "a2 BAD SPECIAL-USE is not offered"]);
}

#[test]
fn list_refuses_what_it_does_not_know() {
    let fake = FakeImap::sample();
    let got = signed_in(&fake, "a1 LIST \"\"\r\na2 LIST \"\" * RETURN (CHILDREN)\r\na3 LIST \"\" * EXTRA\r\na4 LIST (x) \"\" *\r\n");
    assert_eq!(
        got,
        [
            "a1 BAD LIST needs a reference and a pattern",
            "a2 BAD Unknown LIST return option",
            "a3 BAD LIST needs a reference and a pattern",
            "a4 BAD LIST needs a reference and a pattern",
        ]
    );
    let before = lines(&fake, "a1 LIST \"\" *\r\n");
    assert_eq!(before.get(1).map(String::as_str), Some("a1 BAD LIST is not available now"));
}

#[test]
fn select_reports_the_mailbox() {
    let got = signed_in(&FakeImap::sample(), "a1 SELECT INBOX\r\n");
    assert_eq!(
        got,
        [
            "* 3 EXISTS",
            "* 0 RECENT",
            "* OK [UNSEEN 2] First unseen",
            "* OK [UIDVALIDITY 1] UIDs valid",
            "* OK [UIDNEXT 4] Predicted next UID",
            "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)",
            "* OK [PERMANENTFLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft \\*)] Flags allowed",
            "* OK [HIGHESTMODSEQ 4] Highest",
            "a1 OK [READ-WRITE] SELECT completed",
        ]
    );
}

#[test]
fn examine_is_read_only_and_without_condstore_has_no_modseq() {
    let fake = FakeImap::sample().without(Capability::CondStore);
    let got = signed_in(&fake, "a1 EXAMINE Archive\r\n");
    assert_eq!(
        got,
        [
            "* 1 EXISTS",
            "* 0 RECENT",
            "* OK [UIDVALIDITY 7] UIDs valid",
            "* OK [UIDNEXT 13] Predicted next UID",
            "* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)",
            "* OK [PERMANENTFLAGS ()] Read-only",
            "a1 OK [READ-ONLY] EXAMINE completed",
        ]
    );
}

#[test]
fn a_mailbox_that_does_not_exist_leaves_none_selected() {
    let got = signed_in(&FakeImap::sample(), "a1 SELECT INBOX\r\na2 SELECT Nowhere\r\na3 UID FETCH 1 FLAGS\r\na4 SELECT\r\n");
    let tail: Vec<&str> = got.iter().skip(9).map(String::as_str).collect();
    assert_eq!(tail, ["a2 NO [NONEXISTENT] No such mailbox", "a3 BAD UID FETCH is not available now", "a4 BAD SELECT needs a mailbox name"]);
}

#[test]
fn a_mailbox_name_may_come_as_a_literal() {
    let got = signed_in(&FakeImap::sample(), "a1 SELECT {11}\r\nElküldött\r\n");
    assert_eq!(got.first().map(String::as_str), Some("+ Ready for the literal"));
    assert_eq!(got.last().map(String::as_str), Some("a1 OK [READ-WRITE] SELECT completed"));
}

#[test]
fn status_counts_a_mailbox_without_selecting_it() {
    let fake = FakeImap::sample();
    let got = signed_in(
        &fake,
        "a1 STATUS INBOX (MESSAGES UIDNEXT UIDVALIDITY UNSEEN RECENT HIGHESTMODSEQ)\r\na2 STATUS Nowhere (MESSAGES)\r\na3 STATUS INBOX (SIZE)\r\na4 STATUS INBOX\r\n",
    );
    assert_eq!(
        got,
        [
            "* STATUS \"INBOX\" (MESSAGES 3 UIDNEXT 4 UIDVALIDITY 1 UNSEEN 2 RECENT 0 HIGHESTMODSEQ 4)",
            "a1 OK STATUS completed",
            "a2 NO [NONEXISTENT] No such mailbox",
            "a3 BAD Unknown STATUS item",
            "a4 BAD STATUS needs a mailbox name and items",
        ]
    );
    let off = signed_in(&fake.without(Capability::CondStore), "a1 STATUS INBOX (HIGHESTMODSEQ)\r\n");
    assert_eq!(off, ["a1 BAD Unknown STATUS item"]);
}

#[test]
fn uid_fetch_returns_what_was_asked_for() {
    let got = signed_in(&FakeImap::sample(), "a1 EXAMINE INBOX\r\na2 UID FETCH 1:2 (UID FLAGS INTERNALDATE RFC822.SIZE)\r\na3 UID FETCH 3 FLAGS\r\n");
    let tail: Vec<&str> = got.iter().skip(9).map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            "* 1 FETCH (UID 1 FLAGS (\\Seen) INTERNALDATE \"02-Nov-2026 09:15:00 +0000\" RFC822.SIZE 155)",
            "* 2 FETCH (UID 2 FLAGS () INTERNALDATE \"03-Nov-2026 02:00:00 +0000\" RFC822.SIZE 179)",
            "a2 OK UID FETCH completed",
            "* 3 FETCH (UID 3 FLAGS (\\Flagged))",
            "a3 OK UID FETCH completed",
        ]
    );
}

#[test]
fn uid_fetch_returns_bodies_as_literals() {
    let raw = "From: anna@example.org\nSubject: Hi\n\nHello\n";
    let fake = FakeImap::new().mailbox(Mailbox::new("INBOX").message(Message::new(raw)));
    let got = signed_in(&fake, "a1 EXAMINE INBOX\r\na2 UID FETCH 1 (BODY.PEEK[HEADER] BODY.PEEK[TEXT] BODY.PEEK[])\r\n");
    let tail: Vec<&str> = got.iter().skip(9).map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            "* 1 FETCH (UID 1 BODY[HEADER] {39}",
            "From: anna@example.org",
            "Subject: Hi",
            "",
            " BODY[TEXT] {7}",
            "Hello",
            " BODY[] {46}",
            "From: anna@example.org",
            "Subject: Hi",
            "",
            "Hello",
            ")",
            "a2 OK UID FETCH completed",
        ]
    );
}

#[test]
fn fetching_a_body_without_peek_marks_it_seen_in_a_mailbox_open_for_writing() {
    let raw = "Subject: Hi\n\nHello\n";
    let fake = FakeImap::new().mailbox(Mailbox::new("INBOX").message(Message::new(raw)));
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 UID FETCH 1 BODY[TEXT]\r\na3 UID FETCH 1 (RFC822.HEADER RFC822.TEXT RFC822 FLAGS)\r\n");
    let tail: Vec<&str> = got.iter().skip(9).map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            "* 1 FETCH (UID 1 BODY[TEXT] {7}",
            "Hello",
            " FLAGS (\\Seen))",
            "a2 OK UID FETCH completed",
            "* 1 FETCH (UID 1 RFC822.HEADER {15}",
            "Subject: Hi",
            "",
            " RFC822.TEXT {7}",
            "Hello",
            " RFC822 {22}",
            "Subject: Hi",
            "",
            "Hello",
            " FLAGS (\\Seen))",
            "a3 OK UID FETCH completed",
        ]
    );
    let examined = signed_in(&fake, "a1 EXAMINE INBOX\r\na2 UID FETCH 1 BODY[]\r\na3 UID FETCH 1 FLAGS\r\n");
    assert!(examined.contains(&"* 1 FETCH (UID 1 FLAGS ())".to_owned()), "{examined:?}");
}

#[test]
fn a_message_without_a_blank_line_is_all_header() {
    let fake = FakeImap::new().mailbox(Mailbox::new("INBOX").message(Message::new("Subject: only")));
    let got = signed_in(&fake, "a1 EXAMINE INBOX\r\na2 UID FETCH 1 (BODY.PEEK[HEADER] BODY.PEEK[TEXT])\r\n");
    assert_eq!(got.get(9..12), Some(["* 1 FETCH (UID 1 BODY[HEADER] {13}", "Subject: only BODY[TEXT] {0}", ")"].map(str::to_owned).as_slice()));
}

#[test]
fn uid_sets_read_ranges_stars_and_lists() {
    let fake = FakeImap::sample();
    let got = signed_in(&fake, "a1 EXAMINE INBOX\r\na2 UID FETCH 3:2,1 UID\r\na3 UID FETCH 9:* UID\r\na4 UID FETCH * UID\r\na5 UID FETCH 7 UID\r\n");
    let tail: Vec<&str> = got.iter().skip(9).map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            "* 1 FETCH (UID 1)",
            "* 2 FETCH (UID 2)",
            "* 3 FETCH (UID 3)",
            "a2 OK UID FETCH completed",
            "* 3 FETCH (UID 3)",
            "a3 OK UID FETCH completed",
            "* 3 FETCH (UID 3)",
            "a4 OK UID FETCH completed",
            "a5 OK UID FETCH completed",
        ]
    );
    let empty = signed_in(&FakeImap::new(), "a1 EXAMINE INBOX\r\na2 UID FETCH 1:* UID\r\n");
    assert_eq!(empty.last().map(String::as_str), Some("a2 OK UID FETCH completed"));
}

#[test]
fn uid_fetch_refuses_what_it_does_not_know() {
    let fake = FakeImap::sample();
    let input = "a1 EXAMINE INBOX\r\nb1 UID FETCH 0 UID\r\nb2 UID FETCH 1: UID\r\nb3 UID FETCH x UID\r\nb4 UID FETCH 1 ENVELOPE\r\nb5 UID FETCH 1\r\nb6 UID FETCH 1 (UID \"x\")\r\nb7 UID FETCH 1 UID (CHANGEDSINCE 1) x\r\nb8 FETCH 1 UID\r\nb9 UID FETCH 4294967296 UID\r\nc1 UID FETCH 1 ()\r\n";
    let got = signed_in(&fake, input);
    let tail: Vec<&str> = got.iter().skip(9).map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            "b1 BAD Bad UID set",
            "b2 BAD Bad UID set",
            "b3 BAD Bad UID set",
            "b4 BAD Unknown FETCH item",
            "b5 BAD UID FETCH needs a UID set and items",
            "b6 BAD Unknown FETCH item",
            "b7 BAD UID FETCH needs a UID set and items",
            "b8 BAD FETCH is not available now (the fake knows the UID form only)",
            "b9 BAD Bad UID set",
            "c1 BAD UID FETCH needs a UID set and items",
        ]
    );
    let many = format!("a1 EXAMINE INBOX\r\nb1 UID FETCH {} UID\r\n", (1..=1001).map(|n| (n * 2).to_string()).collect::<Vec<_>>().join(","));
    assert_eq!(signed_in(&fake, &many).last().map(String::as_str), Some("b1 BAD Bad UID set"));
}

#[test]
fn uid_search_finds_by_flags_dates_sizes_and_text() {
    let fake = FakeImap::sample();
    let cases = [
        ("ALL", "1 2 3"),
        ("SEEN", "1"),
        ("UNSEEN", "2 3"),
        ("FLAGGED", "3"),
        ("UNFLAGGED", "1 2"),
        ("ANSWERED", ""),
        ("UNANSWERED", "1 2 3"),
        ("DELETED", ""),
        ("UNDELETED", "1 2 3"),
        ("DRAFT", ""),
        ("UNDRAFT", "1 2 3"),
        ("SINCE 3-Nov-2026", "2 3"),
        ("BEFORE 3-Nov-2026", "1"),
        ("ON 03-Nov-2026", "2"),
        ("LARGER 170", "2 3"),
        ("SMALLER 170", "1"),
        ("UID 2:*", "2 3"),
        ("FROM ANNA", "1"),
        ("TO \"ada@example.org\"", "1 2 3"),
        ("CC bob", ""),
        ("SUBJECT {7}\r\nnightly", "2"),
        ("BODY quarter", "3"),
        ("TEXT friday", "1"),
        ("HEADER List-Id builds.example.org", "2"),
        ("HEADER X-None \"\"", ""),
        ("NOT SEEN", "2 3"),
        ("OR SEEN FLAGGED", "1 3"),
        ("(UNSEEN FROM bob)", "3"),
        ("CHARSET UTF-8 SUBJECT lunch", "1"),
        ("CHARSET us-ascii ALL", "1 2 3"),
        ("UNSEEN SINCE 1-Jan-2026 NOT DELETED", "2 3"),
    ];
    for (keys, want) in cases {
        let input = format!("a1 EXAMINE INBOX\r\na2 UID SEARCH {keys}\r\n");
        let got = signed_in(&fake, &input);
        let found = got.iter().find(|line| line.starts_with("* SEARCH")).map(String::as_str);
        let want = if want.is_empty() { "* SEARCH".to_owned() } else { format!("* SEARCH {want}") };
        assert_eq!(found, Some(want.as_str()), "{keys}: {got:?}");
        assert_eq!(got.last().map(String::as_str), Some("a2 OK UID SEARCH completed"), "{keys}");
    }
}

#[test]
fn uid_search_refuses_what_it_does_not_know() {
    let fake = FakeImap::sample();
    let cases = [
        ("", "a2 BAD UID SEARCH needs search keys"),
        ("KEYWORD x", "a2 BAD Unknown search key"),
        ("SINCE 31-Foo-2026", "a2 BAD Bad date"),
        ("SINCE 1-Jan", "a2 BAD Bad date"),
        ("SINCE x-Jan-2026", "a2 BAD Bad date"),
        ("SINCE", "a2 BAD Bad date"),
        ("LARGER x", "a2 BAD Bad number"),
        ("UID 0", "a2 BAD Bad UID set"),
        ("FROM", "a2 BAD Missing search text"),
        ("HEADER Subject", "a2 BAD Missing search text"),
        ("NOT", "a2 BAD UID SEARCH needs search keys"),
        ("OR SEEN", "a2 BAD UID SEARCH needs search keys"),
        ("CHARSET KOI8-R ALL", "a2 NO [BADCHARSET (UTF-8 US-ASCII)] Unknown charset"),
        ("CHARSET", "a2 BAD UID SEARCH needs search keys"),
        ("ALL ()", "a2 BAD UID SEARCH needs search keys"),
        ("\"ALL\"", "a2 BAD Unknown search key"),
        (&format!("{}SEEN", "NOT ".repeat(40)), "a2 BAD Search keys nested too deep"),
    ];
    for (keys, want) in cases {
        let got = signed_in(&fake, &format!("a1 EXAMINE INBOX\r\na2 UID SEARCH {keys}\r\n"));
        assert_eq!(got.last().map(String::as_str), Some(want), "{keys}");
    }
}

#[test]
fn close_expunges_deleted_messages_quietly() {
    let fake = FakeImap::new()
        .mailbox(Mailbox::new("INBOX").message(Message::new("Subject: a\n\nx\n").flags(&["\\Deleted"])).message(Message::new("Subject: b\n\ny\n")));
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 CLOSE\r\na3 SELECT INBOX\r\na4 CLOSE\r\na5 CLOSE\r\n");
    assert_eq!(got.get(9).map(String::as_str), Some("a2 OK CLOSE completed"));
    assert_eq!(got.get(10).map(String::as_str), Some("* 1 EXISTS"));
    assert_eq!(got.iter().rev().take(2).map(String::as_str).collect::<Vec<_>>(), ["a5 BAD CLOSE is not available now", "a4 OK CLOSE completed"]);
    let examined = signed_in(&fake, "a1 EXAMINE INBOX\r\na2 CLOSE\r\na3 EXAMINE INBOX\r\n");
    assert_eq!(examined.get(10).map(String::as_str), Some("* 2 EXISTS"));
}

#[test]
fn fixtures_keep_their_uids_flags_dates_and_previews() {
    let mailbox = Mailbox::new("Work")
        .uid_validity(42)
        .message(Message::new("Subject: a\n\nx\n").uid(10).flags(&["\\Seen", "$Label1"]).date("05-Nov-2026 10:00:00 +0100"))
        .message(Message::new("Subject: b\n\ny\n"));
    assert_eq!(mailbox.name(), "Work");
    let fake = FakeImap::new().mailbox(mailbox.clone()).mailbox(mailbox).mailbox(Mailbox::new("inbox").message(Message::new("Subject: c\n\nz\n")));
    let got = signed_in(&fake, "a1 EXAMINE Work\r\na2 UID FETCH 1:* (FLAGS INTERNALDATE)\r\na3 STATUS INBOX (MESSAGES)\r\na4 LIST \"\" *\r\n");
    assert!(got.contains(&"* OK [UIDVALIDITY 42] UIDs valid".to_owned()), "{got:?}");
    assert!(got.contains(&"* 1 FETCH (UID 10 FLAGS (\\Seen $Label1) INTERNALDATE \"05-Nov-2026 10:00:00 +0100\")".to_owned()), "{got:?}");
    assert!(got.contains(&"* 2 FETCH (UID 11 FLAGS () INTERNALDATE \"01-Jan-2026 00:00:00 +0000\")".to_owned()), "{got:?}");
    assert!(got.contains(&"* STATUS \"INBOX\" (MESSAGES 1)".to_owned()), "{got:?}");
    assert_eq!(got.iter().filter(|line| line.starts_with("* LIST")).count(), 2, "{got:?}");
}

#[test]
fn select_takes_no_parameters_it_does_not_know() {
    let got = signed_in(&FakeImap::sample(), "a1 SELECT INBOX x\r\n");
    assert_eq!(got, ["a1 BAD SELECT parameters are not offered"]);
}

#[test]
fn a_mailbox_name_with_quotes_is_written_escaped() {
    let got = signed_in(&FakeImap::new().mailbox(Mailbox::new("Say \"hi\" \\o/")), "a1 LIST \"\" Say*\r\n");
    assert_eq!(got, ["* LIST (\\HasNoChildren) \"/\" \"Say \\\"hi\\\" \\\\o/\"", "a1 OK LIST completed"]);
}
