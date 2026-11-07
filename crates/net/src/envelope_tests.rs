// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{MAX_ADDRESSES, MessageRef};
use inboxcraft_time::Timestamp;

use super::{MAX_LIST_CHARS, addresses, envelope, message_refs};

fn pairs(value: &str) -> Vec<(Option<String>, String)> {
    addresses(value).iter().map(|address| (address.name().map(str::to_owned), address.email().as_str().to_owned())).collect()
}

fn named(name: &str, email: &str) -> (Option<String>, String) {
    (Some(name.to_owned()), email.to_owned())
}

fn bare(email: &str) -> (Option<String>, String) {
    (None, email.to_owned())
}

#[test]
fn a_header_block_gives_the_envelope() {
    let header = b"From: Anna Example <anna@example.org>\r\n\
        To: ada@example.org, \"Bob, the builder\" <bob@example.org>\r\n\
        Cc: =?UTF-8?Q?Zs=C3=B3fia?= <zsofia@example.org>\r\n\
        Reply-To: replies@example.org\r\n\
        Subject: =?UTF-8?Q?K=C3=A9p?= and text\r\n\
        Date: Mon, 2 Nov 2026 09:14:00 +0100\r\n\
        Message-ID: <one@example.org>\r\n\
        In-Reply-To: <zero@example.org>\r\n\
        References: <minus@example.org> <zero@example.org>\r\n\
        \r\n";
    let envelope = envelope(header);
    assert_eq!(envelope.subject(), "Kép and text");
    assert_eq!(envelope.date(), Timestamp::from_unix(1_793_607_240, 0));
    let from: Vec<_> = envelope.from().iter().map(|a| (a.name().map(str::to_owned), a.email().as_str().to_owned())).collect();
    assert_eq!(from, [named("Anna Example", "anna@example.org")]);
    assert_eq!(envelope.to().len(), 2);
    assert_eq!(envelope.to()[1].name(), Some("Bob, the builder"));
    assert_eq!(envelope.cc()[0].name(), Some("Zsófia"));
    assert_eq!(envelope.reply_to()[0].email().as_str(), "replies@example.org");
    assert!(envelope.bcc().is_empty());
    assert_eq!(envelope.message_id().map(MessageRef::as_str), Some("one@example.org"));
    assert_eq!(envelope.in_reply_to().iter().map(MessageRef::as_str).collect::<Vec<_>>(), ["zero@example.org"]);
    assert_eq!(envelope.references().iter().map(MessageRef::as_str).collect::<Vec<_>>(), ["minus@example.org", "zero@example.org"]);
}

#[test]
fn bytes_that_are_no_header_give_an_empty_envelope() {
    for bytes in [&b""[..], b"\r\n", b"\xff\xfe\x00", b"no colon here\r\n\r\n"] {
        let envelope = envelope(bytes);
        assert_eq!(envelope.subject(), "");
        assert!(envelope.from().is_empty());
        assert_eq!(envelope.date(), None);
    }
}

#[test]
fn a_date_that_cannot_be_read_is_none() {
    assert_eq!(envelope(b"Date: someday\r\n\r\n").date(), None);
}

#[test]
fn a_subject_with_line_breaks_and_marks_is_one_clean_line() {
    let envelope = envelope(b"Subject: one\r\n two\x07 \xe2\x80\xaethree\r\n\r\n");
    assert!(!envelope.subject().chars().any(char::is_control));
    assert!(!envelope.subject().contains('\u{202e}'));
    assert!(envelope.subject().starts_with("one"));
}

#[test]
fn address_lists_of_every_form() {
    assert_eq!(pairs("anna@example.org"), [bare("anna@example.org")]);
    assert_eq!(pairs("<anna@example.org>"), [bare("anna@example.org")]);
    assert_eq!(pairs("Anna <anna@example.org>"), [named("Anna", "anna@example.org")]);
    assert_eq!(pairs("\"Anna \\\"A\\\" Example\" <anna@example.org>"), [named("Anna \"A\" Example", "anna@example.org")]);
    assert_eq!(pairs("anna@example.org (Anna Example)"), [named("Anna Example", "anna@example.org")]);
    assert_eq!(pairs("Anna (the first) <anna@example.org>"), [named("Anna", "anna@example.org")]);
    assert_eq!(pairs("a@example.org,,b@example.org ,"), [bare("a@example.org"), bare("b@example.org")]);
    assert_eq!(pairs("=?ISO-8859-1?Q?Andr=E9?= <andre@example.org>"), [named("André", "andre@example.org")]);
}

#[test]
fn groups_give_their_members() {
    assert_eq!(
        pairs("Team: a@example.org, B <b@example.org>;, c@example.org"),
        [bare("a@example.org"), named("B", "b@example.org"), bare("c@example.org")]
    );
    assert!(pairs("undisclosed-recipients:;").is_empty());
}

#[test]
fn what_is_no_address_is_left_out() {
    assert!(pairs("").is_empty());
    assert!(pairs("Anna").is_empty());
    assert!(pairs("<>").is_empty());
    assert!(pairs("Anna <anna>").is_empty());
    assert!(pairs("\"unclosed <a@example.org>").is_empty());
    assert!(pairs("(unclosed a@example.org").is_empty());
    assert!(pairs("<unclosed@example.org").is_empty());
    assert_eq!(pairs("broken, b@example.org"), [bare("b@example.org")]);
}

#[test]
fn a_comma_inside_quotes_brackets_or_comments_does_not_split() {
    assert_eq!(pairs("\"a, b\" <a@example.org>"), [named("a, b", "a@example.org")]);
    assert_eq!(pairs("a@example.org (one, two)"), [named("one, two", "a@example.org")]);
    assert_eq!(pairs("x (nested (deep, er)) <x@example.org>"), [named("x", "x@example.org")]);
}

#[test]
fn an_address_list_is_capped() {
    let many = vec!["a@example.org"; MAX_ADDRESSES + 10].join(",");
    assert_eq!(addresses(&many).len(), MAX_ADDRESSES);
}

#[test]
fn an_overlong_list_is_read_up_to_the_cap() {
    let long = format!("a@example.org, {}", "x".repeat(MAX_LIST_CHARS * 2));
    assert_eq!(pairs(&long), [bare("a@example.org")]);
}

#[test]
fn deep_comments_do_not_recurse() {
    let deep = format!("{}a@example.org", "(".repeat(100_000));
    assert!(addresses(&deep).is_empty());
}

#[test]
fn message_ids_are_read_between_angle_brackets() {
    let refs = |text: &str| message_refs(text).iter().map(|id| id.as_str().to_owned()).collect::<Vec<_>>();
    assert_eq!(refs("<a@example.org> <b@example.org>"), ["a@example.org", "b@example.org"]);
    assert_eq!(refs("<a@example.org>(comment)<b@example.org>"), ["a@example.org", "b@example.org"]);
    assert_eq!(refs("garbage <a@example.org> <unclosed"), ["a@example.org"]);
    assert_eq!(refs("<> < >"), Vec::<String>::new());
    assert_eq!(refs("bare@example.org"), Vec::<String>::new());
}

#[test]
fn a_header_block_gives_the_signs_of_the_messages_kind() {
    let header = b"From: Example News <news@example.org>\r\n\
        List-Id: Example News <news.example.org>\r\n\
        List-Unsubscribe: <mailto:leave@example.org>\r\n\
        Precedence: Bulk\r\n\
        Auto-Submitted: auto-generated\r\n\
        Content-Type: multipart/alternative; boundary=\"b1\"\r\n\
        \r\n";
    let envelope = envelope(header);
    let signs = envelope.signs();
    assert!(signs.list());
    assert_eq!(signs.precedence(), Some(inboxcraft_model::Precedence::Bulk));
    assert!(signs.automatic());
    assert!(!signs.calendar_type());
    assert!(!signs.calendar(), "the parts are not known from the header");
    assert!(super::envelope(b"From: anna@example.org\r\n\r\n").signs().is_empty());
}

#[test]
fn inboxcrafts_own_field_gives_the_time_a_snooze_ends() {
    let header = b"From: anna@example.org\r\n\
        InboxCraft-Snoozed-Until: Thu, 05 Nov 2026 18:00:00 +0000\r\n\
        inboxcraft-snoozed-until: Fri, 06 Nov 2026 08:00:00 +0000\r\n\
        \r\n";
    assert_eq!(envelope(header).snoozed_until(), Timestamp::from_unix(1_793_901_600, 0), "the first field counts");
    assert_eq!(envelope(b"From: anna@example.org\r\n\r\n").snoozed_until(), None);
    let broken = b"From: anna@example.org\r\nInboxCraft-Snoozed-Until: soon\r\n\r\n";
    assert_eq!(envelope(broken).snoozed_until(), None, "a time that cannot be read is no time");
}
