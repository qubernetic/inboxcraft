// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bounce stage: a delivery report passes the screener, its body is fetched, and once it is kept the sent message
//! it names by `Message-ID` is marked as failed with the reason.

use inboxcraft_model::{
    AccountId, Bounce, Envelope, Event, EventKind, FolderId, JobKind, Message, MessageId, MessageRef, Signs, Summary,
};
use inboxcraft_store::Change;
use serde_json::{Value, json};

use crate::arrive::screener::tests::{arrive, arrived, from, mode};
use crate::Session;
use crate::cmd::act_tests::{session as act_session, uid, validity};
use crate::test_support::{address, change};

/// The sent message: account 1's Sent folder (2) holds message 5.
const SENT: u64 = 5;

/// The `Message-ID` of the sent message.
const SENT_ID: &str = "budget-2026@example.org";

/// The act tests' session with message 5 sent with [`SENT_ID`], and the screener holding the first mail of new
/// senders.
fn session() -> Session {
    let mut session = act_session();
    let envelope = Envelope::new("Budget")
        .with_from(vec![address("Anna Example", "anna@example.org")])
        .with_to(vec![address("Béla", "bela@example.org")])
        .with_message_id(MessageRef::new(SENT_ID));
    change(&mut session, Change::PutMessage(Message::new(MessageId::new(SENT), AccountId::new(1), FolderId::new(2), Summary::new(uid(3), envelope))));
    mode(&mut session, "before");
    session.take_jobs();
    session
}

/// A delivery report with UID `n` from `sender`, as its header tells it.
fn report(n: u32, sender: &str) -> Summary {
    let signs = Signs::read([("Content-Type", "multipart/report; report-type=delivery-status; boundary=r")]);
    let envelope = Envelope::new("Undelivered Mail Returned to Sender")
        .with_from(vec![address("Mail Delivery System", sender)])
        .with_to(vec![address("Anna", "anna@example.org")])
        .with_signs(signs);
    Summary::new(uid(n), envelope)
}

/// A report's body: `recipients` (address, action, status, diagnostic), about the message `about`.
fn body(about: &str, recipients: &[(&str, &str, &str, Option<&str>)]) -> Vec<u8> {
    let mut text = String::from(concat!(
        "From: bounces@mx.example.net\r\n",
        "Content-Type: multipart/report; report-type=delivery-status; boundary=r\r\n",
        "\r\n",
        "--r\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "Your message could not be delivered.\r\n",
        "--r\r\n",
        "Content-Type: message/delivery-status\r\n",
        "\r\n",
        "Reporting-MTA: dns; mx.example.net\r\n",
    ));
    for (recipient, action, status, diagnostic) in recipients {
        text.push_str(&format!("\r\nFinal-Recipient: rfc822; {recipient}\r\nAction: {action}\r\nStatus: {status}\r\n"));
        if let Some(diagnostic) = diagnostic {
            text.push_str(&format!("Diagnostic-Code: smtp; {diagnostic}\r\n"));
        }
    }
    text.push_str(&format!("\r\n--r\r\nContent-Type: text/rfc822-headers\r\n\r\nSubject: Budget\r\nMessage-ID: {about}\r\n\r\n--r--\r\n"));
    text.into_bytes()
}

/// The body `raw` of the message with UID `n` in account 1's folder `folder` comes in.
fn fetched(session: &mut Session, folder: u64, n: u32, raw: Vec<u8>) {
    let kind = EventKind::BodyFetched { folder: FolderId::new(folder), uid_validity: validity(), uid: uid(n), raw };
    session.apply(&Event::new(AccountId::new(1), None, kind)).unwrap();
}

fn bounced(session: &Session) -> Option<Bounce> {
    session.store().message_state(MessageId::new(SENT)).unwrap().bounced
}

/// The UIDs of the bodies asked for.
fn fetches(session: &mut Session) -> Vec<u32> {
    session
        .take_jobs()
        .into_iter()
        .filter_map(|job| match job.kind {
            JobKind::FetchBody { uid, .. } => Some(uid.get()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_delivery_report_passes_the_screener_and_its_body_is_fetched() {
    let mut session = session();
    // A sender the screener would hold, and a sender of reports.
    arrive(&mut session, 1, vec![report(30, "bounces@mx.example.net"), report(31, "MAILER-DAEMON@mx.example.net")]);
    assert_eq!(fetches(&mut session), [30, 31]);
    for n in [30, 31] {
        assert_eq!(arrived(&session, n).folder(), FolderId::new(1), "{n}");
    }
    // Neither sender is known from it.
    let senders = session.store().senders().unwrap();
    assert!(!senders.iter().any(|sender| sender.address.as_str().contains("mx.example.net")), "{senders:?}");
    // Mail from the same sender that is no report is held as before.
    arrive(&mut session, 1, vec![from(32, "bounces@mx.example.net")]);
    assert!(arrived(&session, 32).moving().is_some() || arrived(&session, 32).folder() != FolderId::new(1));
}

#[test]
fn a_kept_report_marks_the_sent_message_it_names_as_failed() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, "bounces@mx.example.net")]);
    session.take_jobs();
    let raw = body(
        &format!("<{SENT_ID}>"),
        &[
            ("bela@example.org", "failed", "5.1.1", Some("550 5.1.1 The mailbox does not exist")),
            ("cili@example.org", "delayed", "4.4.1", None),
            ("dora@example.org", "failed", "5.2.2", None),
        ],
    );
    fetched(&mut session, 1, 30, raw);
    let report = arrived(&session, 30).id();
    let reason = "bela@example.org: 550 5.1.1 The mailbox does not exist; dora@example.org: 5.2.2".to_owned();
    assert_eq!(bounced(&session), Some(Bounce { report, reason: reason.clone() }));
    // The Sent folder's list and the reader show it.
    let listed = session.run("mail.list", &json!({ "folder": 2 })).unwrap();
    let row = listed["messages"].as_array().unwrap().iter().find(|row| row["id"] == json!(SENT)).unwrap().clone();
    assert_eq!(row["bounced"], json!({ "report": report.get(), "reason": reason }));
    let read = session.run("mail.read", &json!({ "id": SENT })).unwrap();
    assert_eq!(read["message"]["bounced"]["reason"], json!(reason));
    // A message that did not bounce shows none.
    let other = listed["messages"].as_array().unwrap().iter().find(|row| row["id"] != json!(SENT)).cloned();
    assert!(other.is_none_or(|row| row["bounced"] == Value::Null));
    // The report is kept as any other body.
    assert!(session.store().body(report).unwrap().is_some());
}

#[test]
fn a_report_that_names_no_sent_message_marks_nothing() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, "bounces@mx.example.net"), report(31, "bounces@mx.example.net"), report(32, "bounces@mx.example.net")]);
    session.take_jobs();
    let failed = [("bela@example.org", "failed", "5.1.1", None)];
    // Another message's id, and no id at all.
    fetched(&mut session, 1, 30, body("<someone-else@example.org>", &failed));
    fetched(&mut session, 1, 31, body("", &failed));
    // Only delayed and delivered recipients: nothing failed.
    fetched(&mut session, 1, 32, body(&format!("<{SENT_ID}>"), &[("bela@example.org", "delayed", "4.4.1", None), ("cili@example.org", "delivered", "2.0.0", None)]));
    assert_eq!(bounced(&session), None);
}

#[test]
fn a_body_that_is_no_report_marks_nothing() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, "bounces@mx.example.net"), report(31, "bounces@mx.example.net")]);
    session.take_jobs();
    fetched(&mut session, 1, 30, format!("Content-Type: text/plain\r\n\r\nMessage-ID: <{SENT_ID}>\r\n").into_bytes());
    // A report whose status part cannot be read.
    let broken = body(&format!("<{SENT_ID}>"), &[("bela@example.org", "exploded", "5.1.1", None)]);
    fetched(&mut session, 1, 31, broken);
    assert_eq!(bounced(&session), None);
}

#[test]
fn a_report_in_another_account_does_not_reach_this_ones_sent_mail() {
    let mut session = session();
    let added = EventKind::MessagesAdded { folder: FolderId::new(4), uid_validity: validity(), messages: vec![report(30, "bounces@mx.example.net")] };
    session.apply(&Event::new(AccountId::new(2), None, added)).unwrap();
    session.take_jobs();
    let raw = body(&format!("<{SENT_ID}>"), &[("bela@example.org", "failed", "5.1.1", None)]);
    let kind = EventKind::BodyFetched { folder: FolderId::new(4), uid_validity: validity(), uid: uid(30), raw };
    session.apply(&Event::new(AccountId::new(2), None, kind)).unwrap();
    assert_eq!(bounced(&session), None);
}

#[test]
fn a_report_of_a_first_sync_or_outside_the_inbox_is_not_fetched() {
    let mut session = session();
    // The trash and the Sent folder are not looked at.
    arrive(&mut session, 6, vec![report(40, "bounces@mx.example.net")]);
    arrive(&mut session, 2, vec![report(41, "bounces@mx.example.net")]);
    assert_eq!(fetches(&mut session), Vec::<u32>::new());
    // A folder's first sync brings old mail, whose reports are not fetched.
    let added = EventKind::MessagesAdded { folder: FolderId::new(4), uid_validity: validity(), messages: vec![report(42, "bounces@mx.example.net")] };
    let mut fresh = act_session();
    fresh.apply(&Event::new(AccountId::new(2), None, added)).unwrap();
    assert_eq!(fetches(&mut fresh), Vec::<u32>::new());
    // A long reason is cut at a character.
    let many: Vec<(String, &str, &str, Option<String>)> =
        (0..200).map(|n| (format!("rcpt{n}@example.org"), "failed", "5.1.1", Some(format!("550 5.1.1 ünknown {n}")))).collect();
    let recipients: Vec<(&str, &str, &str, Option<&str>)> =
        many.iter().map(|(address, action, status, diagnostic)| (address.as_str(), *action, *status, diagnostic.as_deref())).collect();
    arrive(&mut session, 1, vec![report(43, "bounces@mx.example.net")]);
    session.take_jobs();
    fetched(&mut session, 1, 43, body(&format!("<{SENT_ID}>"), &recipients));
    let reason = bounced(&session).unwrap().reason;
    assert!(reason.chars().count() <= super::MAX_REASON, "{}", reason.len());
    assert!(reason.starts_with("rcpt0@example.org: 550 5.1.1 ünknown 0; "), "{reason}");
}
