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

/// A sender of delivery reports.
const REPORTER: &str = "MAILER-DAEMON@mx.example.net";

/// The act tests' session with message 5 sent with [`SENT_ID`], and the screener holding the first mail of new
/// senders.
fn session() -> Session {
    let mut session = act_session();
    let envelope = Envelope::new("Budget")
        .with_from(vec![address("Anna Example", "anna@example.org")])
        .with_to(vec![address("Béla", "bela@example.org")])
        .with_cc(vec![address("Cili", "cili@example.org"), address("Dóra", "Dora@Example.org")])
        .with_bcc(vec![address("Ede", "ede@example.org")])
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
    arrive(&mut session, 1, vec![report(30, REPORTER), report(31, "postmaster@mx.example.net")]);
    assert_eq!(fetches(&mut session), [30, 31]);
    for n in [30, 31] {
        assert_eq!(arrived(&session, n).folder(), FolderId::new(1), "{n}");
    }
    // Neither sender is known from it.
    let senders = session.store().senders().unwrap();
    assert!(!senders.iter().any(|sender| sender.address.as_str().contains("mx.example.net")), "{senders:?}");
}

#[test]
fn a_report_header_from_an_unknown_sender_is_still_held() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, "bounces@mx.example.net")]);
    let jobs = session.take_jobs();
    assert!(!jobs.iter().any(|job| matches!(job.kind, JobKind::FetchBody { .. })), "{jobs:?}");
    assert!(jobs.iter().any(|job| matches!(&job.kind, JobKind::CreateFolder { path } if path.name().ends_with("Screener"))), "{jobs:?}");
    let held = arrived(&session, 30);
    assert!(held.moving().is_some() || held.folder() != FolderId::new(1), "{held:?}");
    // Its sender is neither known nor decided.
    let address = inboxcraft_model::EmailAddress::new("bounces@mx.example.net").unwrap();
    assert!(session.store().sender(&address).unwrap().is_none_or(|sender| !sender.known && sender.screening.is_none()));
    // Mail from the same sender that is no report is held as well.
    arrive(&mut session, 1, vec![from(32, "bounces@mx.example.net")]);
    assert!(arrived(&session, 32).moving().is_some() || arrived(&session, 32).folder() != FolderId::new(1));
}

#[test]
fn a_kept_report_marks_the_sent_message_it_names_as_failed() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, REPORTER)]);
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
    arrive(&mut session, 1, vec![report(30, REPORTER), report(31, REPORTER), report(32, REPORTER)]);
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
    arrive(&mut session, 1, vec![report(30, REPORTER), report(31, REPORTER)]);
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
    let added = EventKind::MessagesAdded { folder: FolderId::new(4), uid_validity: validity(), messages: vec![report(30, REPORTER)] };
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
    arrive(&mut session, 6, vec![report(40, REPORTER)]);
    arrive(&mut session, 2, vec![report(41, REPORTER)]);
    assert_eq!(fetches(&mut session), Vec::<u32>::new());
    // A folder's first sync brings old mail, whose reports are not fetched.
    let added = EventKind::MessagesAdded { folder: FolderId::new(4), uid_validity: validity(), messages: vec![report(42, REPORTER)] };
    let mut fresh = act_session();
    fresh.apply(&Event::new(AccountId::new(2), None, added)).unwrap();
    assert_eq!(fetches(&mut fresh), Vec::<u32>::new());
}

#[test]
fn a_long_reason_is_cut_at_a_character() {
    let mut session = session();
    let long = format!("550 5.1.1 {}", "ü".repeat(1200));
    arrive(&mut session, 1, vec![report(43, REPORTER)]);
    session.take_jobs();
    fetched(&mut session, 1, 43, body(&format!("<{SENT_ID}>"), &[("bela@example.org", "failed", "5.1.1", Some(&long))]));
    let reason = bounced(&session).unwrap().reason;
    assert_eq!(reason.chars().count(), super::MAX_REASON);
    assert!(reason.starts_with("bela@example.org: 550 5.1.1 ü"), "{reason}");
}

#[test]
fn a_report_naming_a_recipient_the_message_did_not_have_marks_nothing() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, REPORTER), report(31, REPORTER)]);
    session.take_jobs();
    // A stranger, and the Bcc recipient: the copy names no Bcc, so a report about one marks nothing.
    fetched(&mut session, 1, 30, body(&format!("<{SENT_ID}>"), &[("stranger@example.net", "failed", "5.1.1", None)]));
    fetched(&mut session, 1, 31, body(&format!("<{SENT_ID}>"), &[("ede@example.org", "failed", "5.1.1", None)]));
    assert_eq!(bounced(&session), None);
}

#[test]
fn a_later_report_with_foreign_recipients_keeps_the_first_reason() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, REPORTER), report(31, REPORTER)]);
    session.take_jobs();
    // The original recipient counts too, without case.
    let first = "Final-Recipient: rfc822; forwarded@example.net\r\nOriginal-Recipient: rfc822; <DORA@example.org>";
    let raw = body(&format!("<{SENT_ID}>"), &[("bela@example.org", "delivered", "2.0.0", None)]);
    let raw = String::from_utf8(raw).unwrap().replacen("Final-Recipient: rfc822; bela@example.org", first, 1).replacen("Action: delivered", "Action: failed", 1).replacen("Status: 2.0.0", "Status: 5.2.2", 1);
    fetched(&mut session, 1, 30, raw.into_bytes());
    let reason = "forwarded@example.net: 5.2.2".to_owned();
    let report = arrived(&session, 30).id();
    assert_eq!(bounced(&session), Some(Bounce { report, reason: reason.clone() }));
    // A second report, about recipients the message never had, leaves it as it is.
    let foreign = [("stranger@example.net", "failed", "5.1.1", Some("550 5.1.1 Unknown")), ("cili@example.org", "delayed", "4.4.1", None)];
    fetched(&mut session, 1, 31, body(&format!("<{SENT_ID}>"), &foreign));
    assert_eq!(bounced(&session), Some(Bounce { report, reason }));
}

#[test]
fn a_bounce_whose_report_is_gone_names_no_report() {
    let mut session = session();
    arrive(&mut session, 1, vec![report(30, REPORTER)]);
    session.take_jobs();
    fetched(&mut session, 1, 30, body(&format!("<{SENT_ID}>"), &[("bela@example.org", "failed", "5.1.1", None)]));
    let report = arrived(&session, 30).id();
    let row = |session: &mut Session| {
        let listed = session.run("mail.list", &json!({ "folder": 2 })).unwrap();
        listed["messages"].as_array().unwrap().iter().find(|row| row["id"] == json!(SENT)).unwrap()["bounced"].clone()
    };
    assert_eq!(row(&mut session)["report"], json!(report.get()));
    // The report is expunged: the mark stays, naming no report.
    let gone = EventKind::MessagesExpunged { folder: FolderId::new(1), uid_validity: validity(), uids: std::iter::once(uid(30)).collect() };
    session.apply(&Event::new(AccountId::new(1), None, gone)).unwrap();
    assert_eq!(row(&mut session), json!({ "report": null, "reason": "bela@example.org: 5.1.1" }));
    // Its id given to a message that is no report, or to one of another account, names none either.
    let plain = Message::new(report, AccountId::new(1), FolderId::new(1), from(33, "anna@example.org"));
    change(&mut session, Change::PutMessage(plain));
    assert_eq!(row(&mut session)["report"], Value::Null);
    let elsewhere = Message::new(report, AccountId::new(2), FolderId::new(4), report_summary(34));
    change(&mut session, Change::PutMessage(elsewhere));
    assert_eq!(row(&mut session)["report"], Value::Null);
    let read = session.run("mail.read", &json!({ "id": SENT })).unwrap();
    assert_eq!(read["message"]["bounced"]["report"], Value::Null);
}

fn report_summary(n: u32) -> Summary {
    report(n, REPORTER)
}


#[test]
fn a_report_held_by_the_screener_links_nothing_when_it_is_read() {
    let mut store = crate::cmd::act_tests::store();
    let screener = crate::test_support::folder(9, 1, "InboxCraft/Screener", vec![]);
    let batch = inboxcraft_store::Batch::new()
        .with(Change::PutFolder(screener))
        .with(Change::SetFolderState(FolderId::new(9), inboxcraft_model::FolderState::new(validity())));
    inboxcraft_store::Storage::apply(&mut store, &batch).unwrap();
    let mut session = Session::new(Box::new(store), Box::new(crate::test_support::clock())).unwrap();
    let envelope = Envelope::new("Budget").with_to(vec![address("Béla", "bela@example.org")]).with_message_id(MessageRef::new(SENT_ID));
    change(&mut session, Change::PutMessage(Message::new(MessageId::new(SENT), AccountId::new(1), FolderId::new(2), Summary::new(uid(3), envelope))));
    arrive(&mut session, 9, vec![report(30, REPORTER)]);
    assert_eq!(fetches(&mut session), Vec::<u32>::new());
    fetched(&mut session, 9, 30, body(&format!("<{SENT_ID}>"), &[("bela@example.org", "failed", "5.1.1", None)]));
    assert!(session.store().body(arrived(&session, 30).id()).unwrap().is_some());
    assert_eq!(bounced(&session), None);
}
