// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server's live updates: scripted mailbox changes, IDLE and NOTIFY.

use std::time::Duration;

use inboxcraft_testkit::imap::{Action, Capability, FakeImap, Mailbox, Message, PASSWORD, USER, When};
use inboxcraft_testkit::{Fault, Listen, TestClient};

fn signed_in(fake: &FakeImap, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(format!("s1 LOGIN {USER} {PASSWORD}\r\n{input}").as_bytes())).into_owned();
    out.split_terminator("\r\n").skip(2).map(str::to_owned).collect()
}

/// The lines after the tagged answer to the command tagged `a1`.
fn after_a1(got: &[String]) -> Vec<String> {
    got.iter().skip_while(|line| !line.starts_with("a1 ")).skip(1).cloned().collect()
}

fn new_mail() -> Message {
    Message::new("From: carol@example.org\nSubject: Hello\n\nNew.\n")
}

fn deliver(mailbox: &str) -> Action {
    Action::Deliver { mailbox: mailbox.to_owned(), message: new_mail() }
}

#[test]
fn new_mail_arrives_while_idling() {
    let fake = FakeImap::sample().on(When::Idle, [deliver("INBOX")]);
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 IDLE\r\nDONE\r\na3 UID FETCH 4 (UID FLAGS)\r\n");
    assert_eq!(after_a1(&got), ["+ idling", "* 4 EXISTS", "a2 OK IDLE terminated", "* 4 FETCH (UID 4 FLAGS ())", "a3 OK UID FETCH completed"]);
}

#[test]
fn removed_mail_and_changed_flags_are_told_while_idling() {
    let flags = Action::Flags { mailbox: "INBOX".to_owned(), uid: 1, flags: vec!["\\Seen".to_owned(), "\\Answered".to_owned()] };
    let expunge = Action::Expunge { mailbox: "INBOX".to_owned(), uid: 2 };
    let fake = FakeImap::sample().on(When::Idle, [flags.clone(), expunge.clone()]);
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 IDLE\r\ndone\r\n");
    assert_eq!(after_a1(&got), ["+ idling", "* 1 FETCH (UID 1 FLAGS (\\Seen \\Answered))", "* 2 EXPUNGE", "a2 OK IDLE terminated"]);
    let fake = FakeImap::sample().on(When::Idle, [flags, expunge]);
    let got = signed_in(&fake, "a0 ENABLE QRESYNC\r\na1 SELECT INBOX\r\na2 IDLE\r\nDONE\r\n");
    assert_eq!(after_a1(&got), ["+ idling", "* 1 FETCH (UID 1 FLAGS (\\Seen \\Answered) MODSEQ (5))", "* VANISHED 2", "a2 OK IDLE terminated"]);
}

#[test]
fn idle_needs_done_and_its_capability() {
    let got = signed_in(&FakeImap::sample(), "a1 SELECT INBOX\r\na2 IDLE\r\nNOOP\r\n");
    assert_eq!(after_a1(&got), ["+ idling", "a2 BAD Expected DONE"]);
    let off = signed_in(&FakeImap::sample().without(Capability::Idle), "a1 IDLE\r\n");
    assert_eq!(off, ["a1 BAD IDLE is not available now"]);
    let gone = signed_in(&FakeImap::sample(), "a1 IDLE\r\n");
    assert_eq!(gone, ["+ idling"]);
    let authenticated = signed_in(&FakeImap::sample(), "a1 IDLE\r\nDONE\r\n");
    assert_eq!(authenticated, ["+ idling", "a1 OK IDLE terminated"]);
}

#[test]
fn changes_between_commands_wait_for_the_next_command() {
    let fake = FakeImap::sample().on(When::answered("NOOP"), [deliver("INBOX"), Action::Expunge { mailbox: "INBOX".to_owned(), uid: 1 }]);
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 NOOP\r\na3 NOOP\r\n");
    assert_eq!(after_a1(&got), ["a2 OK NOOP completed", "* 4 EXISTS", "* 1 EXPUNGE", "a3 OK NOOP completed"]);
}

#[test]
fn changes_before_an_answer_come_with_it() {
    let fake = FakeImap::sample().on(When::command("UID SEARCH"), [deliver("INBOX")]);
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 UID SEARCH ALL\r\n");
    assert_eq!(after_a1(&got), ["* 4 EXISTS", "* SEARCH 1 2 3 4", "a2 OK UID SEARCH completed"]);
}

#[test]
fn a_new_uidvalidity_renumbers_the_mailbox() {
    let fake = FakeImap::new()
        .mailbox(Mailbox::new("INBOX").message(new_mail().uid(10)).message(new_mail()))
        .on(When::Idle, [Action::UidValidity { mailbox: "INBOX".to_owned(), value: 99 }]);
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 IDLE\r\nDONE\r\na3 UID FETCH 1:* UID\r\na4 STATUS INBOX (UIDNEXT UIDVALIDITY)\r\n");
    assert_eq!(
        after_a1(&got),
        [
            "+ idling",
            "* OK [UIDVALIDITY 99] UIDs changed",
            "* 2 EXISTS",
            "a2 OK IDLE terminated",
            "* 1 FETCH (UID 1)",
            "* 2 FETCH (UID 2)",
            "a3 OK UID FETCH completed",
            "* STATUS \"INBOX\" (UIDNEXT 3 UIDVALIDITY 99)",
            "a4 OK STATUS completed",
        ]
    );
}

#[test]
fn changes_elsewhere_show_in_status_and_unknown_targets_are_ignored() {
    let fake = FakeImap::sample().on(
        When::answered("NOOP"),
        [
            deliver("Archive"),
            deliver("Nowhere"),
            Action::Expunge { mailbox: "Nowhere".to_owned(), uid: 1 },
            Action::Expunge { mailbox: "INBOX".to_owned(), uid: 99 },
            Action::Flags { mailbox: "INBOX".to_owned(), uid: 99, flags: Vec::new() },
            Action::Flags { mailbox: "Junk".to_owned(), uid: 1, flags: vec!["\\Seen".to_owned()] },
            Action::UidValidity { mailbox: "Nowhere".to_owned(), value: 5 },
            Action::Expunge { mailbox: "Sent".to_owned(), uid: 1 },
        ],
    );
    let got = signed_in(
        &fake,
        "a1 SELECT INBOX\r\na2 NOOP\r\na3 STATUS Archive (MESSAGES UNSEEN)\r\na4 STATUS Junk (UNSEEN)\r\na5 STATUS Sent (MESSAGES)\r\n",
    );
    assert_eq!(
        after_a1(&got),
        [
            "a2 OK NOOP completed",
            "* STATUS \"Archive\" (MESSAGES 2 UNSEEN 1)",
            "a3 OK STATUS completed",
            "* STATUS \"Junk\" (UNSEEN 0)",
            "a4 OK STATUS completed",
            "* STATUS \"Sent\" (MESSAGES 0)",
            "a5 OK STATUS completed",
        ]
    );
}

#[test]
fn notify_tells_of_changes_as_they_happen() {
    let notify = "NOTIFY SET (SELECTED (MessageNew (UID FLAGS MODSEQ) MessageExpunge FlagChange)) (PERSONAL (MessageNew MessageExpunge))";
    let fake = FakeImap::sample().on(When::answered("NOTIFY"), [deliver("INBOX"), deliver("Archive")]);
    let got = signed_in(&fake, &format!("a1 SELECT INBOX\r\na2 {notify}\r\n"));
    assert_eq!(
        after_a1(&got),
        [
            "a2 OK NOTIFY completed",
            "* 4 EXISTS",
            "* 4 FETCH (UID 4 FLAGS () MODSEQ (5))",
            "* STATUS \"Archive\" (MESSAGES 2 UIDNEXT 14 UIDVALIDITY 7 UNSEEN 1)",
        ]
    );
}

#[test]
fn notify_without_the_selected_filter_leaves_the_open_mailbox_for_the_next_command() {
    let fake = FakeImap::sample().on(When::answered("NOTIFY"), [deliver("INBOX"), deliver("Lists/Rust")]);
    let got = signed_in(&fake, "a0 ENABLE CONDSTORE\r\na1 SELECT INBOX\r\na2 NOTIFY SET (SUBTREE Lists (MessageNew MessageExpunge))\r\na3 NOOP\r\n");
    assert_eq!(
        after_a1(&got),
        [
            "a2 OK NOTIFY completed",
            "* STATUS \"Lists/Rust\" (MESSAGES 2 UIDNEXT 3 UIDVALIDITY 1 UNSEEN 2 HIGHESTMODSEQ 3)",
            "* 4 EXISTS",
            "a3 OK NOOP completed",
        ]
    );
}

#[test]
fn notify_with_selected_none_leaves_the_open_mailbox_for_the_next_command() {
    let fake = FakeImap::sample().on(When::answered("NOTIFY"), [deliver("INBOX")]);
    assert_eq!(after_a1(&signed_in(&fake, "a1 SELECT INBOX\r\na2 NOTIFY SET (SELECTED NONE)\r\n")), ["a2 OK NOTIFY completed"]);
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 NOTIFY SET (SELECTED NONE)\r\na3 NOOP\r\n");
    assert_eq!(after_a1(&got), ["a2 OK NOTIFY completed", "* 4 EXISTS", "a3 OK NOOP completed"]);
}

#[test]
fn notify_filters_pick_their_mailboxes() {
    let cases = [
        ("(INBOXES (MessageNew MessageExpunge))", vec!["INBOX"]),
        ("(MAILBOXES Archive (MessageNew MessageExpunge))", vec!["Archive"]),
        ("(MAILBOXES (Archive Junk) (MessageNew MessageExpunge))", vec!["Archive", "Junk"]),
        ("(SUBTREE (Lists) (MessageNew MessageExpunge))", vec!["Lists", "Lists/Rust"]),
        ("(SUBSCRIBED (MessageNew MessageExpunge))", vec!["INBOX", "Sent", "Drafts", "Archive", "Junk", "Trash", "Lists", "Lists/Rust", "Elküldött"]),
        ("(SELECTED-DELAYED (MessageNew MessageExpunge)) (MAILBOXES Trash NONE)", vec![]),
    ];
    for (filters, want) in cases {
        let got = signed_in(&FakeImap::sample(), &format!("a1 NOTIFY SET STATUS {filters}\r\n"));
        let named: Vec<String> = got
            .iter()
            .filter(|line| line.starts_with("* STATUS"))
            .filter_map(|line| line.split(' ').nth(2))
            .map(|name| name.trim_matches('"').to_owned())
            .collect();
        let want: Vec<String> = want.iter().map(|name| if name.is_ascii() { (*name).to_owned() } else { format!("{{{}}}", name.len()) }).collect();
        assert_eq!(named, want, "{filters}");
        assert_eq!(got.last().map(String::as_str), Some("a1 OK NOTIFY completed"), "{filters}");
    }
}

#[test]
fn notify_none_stops_the_updates() {
    let fake = FakeImap::sample().on(When::answered("NOTIFY"), [deliver("Archive")]);
    let got = signed_in(&fake, "a1 NOTIFY SET (PERSONAL (MessageNew MessageExpunge))\r\na2 NOTIFY NONE\r\n");
    assert_eq!(got, ["a1 OK NOTIFY completed", "* STATUS \"Archive\" (MESSAGES 2 UIDNEXT 14 UIDVALIDITY 7 UNSEEN 1)", "a2 OK NOTIFY completed"]);
    let quiet = FakeImap::sample().on(When::answered("NOTIFY"), [deliver("Archive")]);
    assert_eq!(signed_in(&quiet, "a1 NOTIFY NONE\r\n"), ["a1 OK NOTIFY completed"]);
}

#[test]
fn notify_refuses_what_it_does_not_know() {
    let fake = FakeImap::sample();
    let cases = [
        ("NOTIFY", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET", "a1 BAD Bad NOTIFY"),
        ("NOTIFY FROB", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET x", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (PERSONAL)", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (EVERYWHERE (MessageNew MessageExpunge))", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (MAILBOXES (MessageNew MessageExpunge))", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (MAILBOXES (x (y)) (MessageNew MessageExpunge))", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (PERSONAL x)", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (PERSONAL (MessageNew))", "a1 BAD MessageNew, MessageExpunge and FlagChange go together"),
        ("NOTIFY SET (PERSONAL (FlagChange))", "a1 BAD MessageNew, MessageExpunge and FlagChange go together"),
        ("NOTIFY SET (PERSONAL (MessageNew (UID) MessageExpunge))", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (SELECTED (MessageNew (ENVELOPE) MessageExpunge))", "a1 BAD Bad NOTIFY"),
        ("NOTIFY SET (SELECTED (\"MessageNew\"))", "a1 BAD Bad NOTIFY"),
        (
            "NOTIFY SET (PERSONAL (MailboxMetadata))",
            "a1 NO [BADEVENT (MessageNew MessageExpunge FlagChange MailboxName SubscriptionChange)] Unsupported event",
        ),
        ("NOTIFY SET (PERSONAL (MailboxName SubscriptionChange)) extra", "a1 BAD Bad NOTIFY"),
    ];
    for (line, want) in cases {
        assert_eq!(signed_in(&fake, &format!("a1 {line}\r\n")).last().map(String::as_str), Some(want), "{line}");
    }
    let off = signed_in(&fake.without(Capability::Notify), "a1 NOTIFY NONE\r\n");
    assert_eq!(off, ["a1 BAD NOTIFY is not available now"]);
}

#[test]
fn notify_flag_changes_in_the_open_mailbox() {
    let flags = Action::Flags { mailbox: "INBOX".to_owned(), uid: 3, flags: Vec::new() };
    let fake = FakeImap::sample().on(When::answered("NOTIFY"), [flags]);
    let got =
        signed_in(&fake, "a1 SELECT INBOX\r\na2 NOTIFY SET (SELECTED (MessageNew MessageExpunge FlagChange)) (MAILBOXES Lists (MailboxName))\r\n");
    assert_eq!(after_a1(&got), ["a2 OK NOTIFY completed", "* 3 FETCH (UID 3 FLAGS ())"]);
}

#[test]
fn idle_on_a_socket_tells_of_mail_that_comes_later() -> Result<(), Box<dyn std::error::Error>> {
    let fake = FakeImap::sample().preauth().on(When::Idle, [Action::Fault(Fault::Delay(Duration::from_millis(50))), deliver("INBOX")]);
    let server = Listen::plain().start_imap(fake)?;
    let mut client = TestClient::plain(server.addr(), Duration::from_secs(5))?;
    client.read_line()?;
    client.command("a1 SELECT INBOX")?;
    client.send_line("a2 IDLE")?;
    assert_eq!(client.read_line()?, "+ idling");
    assert_eq!(client.read_line()?, "* 4 EXISTS");
    client.send_line("DONE")?;
    assert_eq!(client.read_line()?, "a2 OK IDLE terminated");
    drop(client);
    assert_eq!(server.finish()?.received(), ["a1 SELECT INBOX", "a2 IDLE", "DONE"]);
    Ok(())
}

#[test]
fn a_fault_while_idling_ends_the_session() {
    let fake = FakeImap::sample().on(When::Idle, [Action::Fault(Fault::Drop)]);
    let got = signed_in(&fake, "a1 SELECT INBOX\r\na2 IDLE\r\nDONE\r\n");
    assert_eq!(after_a1(&got), ["+ idling"]);
}
