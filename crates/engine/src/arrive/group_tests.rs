// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The grouped inbox's classifier, as a table of invented header blocks.

use inboxcraft_mime::Headers;
use inboxcraft_model::{EmailAddress, Envelope, Group, Signs};

use super::{calendar, classify, robot};
use crate::test_support::address;

/// The envelope of an invented header block, read as `net` reads one: the first `From` and the signs.
fn envelope(header: &str) -> Envelope {
    let headers = Headers::parse(header.as_bytes()).unwrap();
    let from = headers.get("from").map(|from| vec![address("Someone", from.trim())]).unwrap_or_default();
    Envelope::new("Hello").with_from(from).with_signs(Signs::read(headers.fields().iter().map(|field| (field.name(), field.value()))))
}

#[test]
fn the_classifier_table() {
    use Group::{Invitations, Newsletters, People, Updates};
    let table: &[(&str, Group)] = &[
        // People: a person writing, with or without the fields a mail program adds.
        ("From: anna@example.org\r\nSubject: Lunch\r\n\r\n", People),
        ("From: anna@example.org\r\nContent-Type: text/plain; charset=utf-8\r\nAuto-Submitted: no\r\n\r\n", People),
        ("From: anna@example.org\r\nPrecedence: first-class\r\n\r\n", People),
        ("From: replies@example.org\r\n\r\n", People),
        ("Subject: no sender at all\r\n\r\n", People),
        // Newsletters: list fields, Precedence list or bulk.
        ("From: news@example.org\r\nList-Id: Example News <news.example.org>\r\n\r\n", Newsletters),
        ("From: news@example.org\r\nList-Unsubscribe: <https://example.org/leave>\r\n\r\n", Newsletters),
        ("From: club@example.org\r\nList-Post: <mailto:club@example.org>\r\n\r\n", Newsletters),
        ("From: offers@example.org\r\nPrecedence: bulk\r\n\r\n", Newsletters),
        ("From: offers@example.org\r\nPrecedence: LIST\r\n\r\n", Newsletters),
        ("From: no-reply@example.org\r\nList-Unsubscribe: <mailto:x@example.org>\r\n\r\n", Newsletters),
        // Updates: automatic mail, no-reply senders, Precedence junk, delivery reports.
        ("From: shop@example.org\r\nAuto-Submitted: auto-generated\r\n\r\n", Updates),
        ("From: anna@example.org\r\nAuto-Submitted: auto-replied\r\nPrecedence: bulk\r\n\r\n", Updates),
        ("From: robot@example.org\r\nAuto-Submitted: auto-notified; owner-email=x@example.org\r\nList-Id: <x.example.org>\r\n\r\n", Updates),
        ("From: no-reply@example.org\r\n\r\n", Updates),
        ("From: NoReply@example.org\r\n\r\n", Updates),
        ("From: do_not_reply@example.org\r\n\r\n", Updates),
        ("From: donotreply+receipts@example.org\r\n\r\n", Updates),
        ("From: no.reply.billing@example.org\r\n\r\n", Updates),
        ("From: noreplyanna@example.org\r\n\r\n", Updates),
        ("From: MAILER-DAEMON@example.org\r\n\r\n", Updates),
        ("From: postmaster@example.org\r\n\r\n", Updates),
        ("From: away@example.org\r\nPrecedence: junk\r\n\r\n", Updates),
        // Invitations: a calendar body, over every other sign.
        ("From: anna@example.org\r\nContent-Type: text/calendar; method=REQUEST\r\n\r\n", Invitations),
        (
            "From: calendar@example.org\r\nContent-Type: TEXT/CALENDAR\r\nAuto-Submitted: auto-generated\r\nList-Id: <c.example.org>\r\n\r\n",
            Invitations,
        ),
    ];
    for (header, group) in table {
        assert_eq!(classify(&envelope(header)), *group, "{header:?}");
    }
}

#[test]
fn a_calendar_part_found_in_the_body_makes_an_invitation() {
    let signs = Signs::read([("Content-Type", "multipart/mixed")]).with_calendar(true);
    let envelope = Envelope::new("Planning").with_from(vec![address("Anna", "anna@example.org")]).with_signs(signs);
    assert_eq!(classify(&envelope), Group::Invitations);
}

#[test]
fn robots_are_told_by_the_local_part_alone() {
    for robot_address in ["noreply@example.org", "no-reply-alerts@example.org", "Do-Not-Reply@example.org", "mailer-daemon@example.org"] {
        assert!(robot(&EmailAddress::new(robot_address).unwrap()), "{robot_address}");
    }
    for person in ["anna@noreply.example.org", "reply@example.org", "nore@example.org", "postmasters-club@example.org"] {
        assert!(!robot(&EmailAddress::new(person).unwrap()), "{person}");
    }
}

#[test]
fn a_body_holds_a_calendar_part_in_its_own_tree_only() {
    let invitation = b"From: anna@example.org\r\n\
        Content-Type: multipart/mixed; boundary=\"a\"\r\n\r\n\
        --a\r\n\
        Content-Type: multipart/alternative; boundary=\"b\"\r\n\r\n\
        --b\r\n\
        Content-Type: text/plain\r\n\r\n\
        Planning on Monday\r\n\
        --b\r\n\
        Content-Type: text/calendar; method=REQUEST\r\n\r\n\
        BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n\
        --b--\r\n\
        --a--\r\n";
    assert!(calendar(invitation));
    let forwarded = b"From: anna@example.org\r\n\
        Content-Type: multipart/mixed; boundary=\"a\"\r\n\r\n\
        --a\r\n\
        Content-Type: text/plain\r\n\r\n\
        See below\r\n\
        --a\r\n\
        Content-Type: message/rfc822\r\n\r\n\
        From: bob@example.org\r\n\
        Content-Type: text/calendar\r\n\r\n\
        BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n\
        --a--\r\n";
    assert!(!calendar(forwarded), "an attached message's invitation is not this message's");
    assert!(calendar(b"Content-Type: text/calendar\r\n\r\nBEGIN:VCALENDAR\r\n"));
    assert!(!calendar(b"From: anna@example.org\r\n\r\nHello\r\n"));
    assert!(!calendar(b"Content-Type: multipart/mixed\r\n\r\nno boundary\r\n"), "a body that cannot be read holds none");
}
