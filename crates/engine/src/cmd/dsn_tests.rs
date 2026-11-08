// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Refusals and bounces against the fake SMTP server: the message `mail.send` hands out is played through
//! [`FakeSmtp::answer`], a refusal comes back as the worker reports it, and the fake's delivery report, fetched from
//! the fake IMAP server's mailbox, arrives in the inbox and marks the sent message as failed.

use inboxcraft_mime::Headers;
use inboxcraft_model::{
    AccountId, DraftId, Envelope, ErrorKind, Event, EventKind, FolderId, FolderState, JobKind, MessageId, MessageRef, Signs, Summary, Uid,
};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_testkit::imap::FakeImap;
use inboxcraft_testkit::smtp::{Bounce, FakeSmtp, Reply, When};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use super::compose_tests::validity;
use super::send_tests::{message_id, store as sending_store};
use crate::Session;
use crate::test_support::{NOW, address, at, send_now};

/// EHLO and AUTH PLAIN with the fake's user and password.
const SIGN_IN: &[u8] = b"EHLO c.example.org\r\nAUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk\r\n";

/// The recipient the fake refuses or reports as failed.
const NOBODY: &str = "nobody@example.org";

/// The sending store with account 1's INBOX (1) and Sent (2) synced at UIDVALIDITY 7.
fn store() -> Memory {
    let mut store = sending_store();
    let batch = Batch::new()
        .with(Change::SetFolderState(FolderId::new(1), FolderState::new(validity())))
        .with(Change::SetFolderState(FolderId::new(2), FolderState::new(validity())));
    store.apply(&batch).unwrap();
    store
}

fn session() -> (Session, FakeClock) {
    let clock = FakeClock::new(at(NOW));
    (Session::new(Box::new(store()), Box::new(clock.clone())).unwrap(), clock)
}

/// A draft of account 1 to [`NOBODY`], sent: its id, its `Message-ID` and the job's envelope and message.
fn send(session: &mut Session, clock: &FakeClock) -> (u64, String, Vec<String>, Vec<u8>) {
    let id = session.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": id, "to": [NOBODY], "subject": "Budget", "text": "The numbers." })).unwrap();
    session.take_jobs();
    send_now(session, clock, "mail.send", id);
    let jobs = session.take_jobs();
    let [job] = jobs.as_slice() else { panic!("{jobs:?}") };
    let JobKind::Send { recipients, raw, .. } = &job.kind else { panic!("{job:?}") };
    (id, message_id(raw), recipients.iter().map(|address| address.as_str().to_owned()).collect(), raw.clone())
}

/// What the fake answers to one submission of `raw` to `recipients`, as the worker sends it.
fn submit(fake: &FakeSmtp, recipients: &[String], raw: &[u8]) -> String {
    let mut input = SIGN_IN.to_vec();
    input.extend_from_slice(b"MAIL FROM:<anna@example.org>\r\n");
    for recipient in recipients {
        input.extend_from_slice(format!("RCPT TO:<{recipient}>\r\n").as_bytes());
    }
    input.extend_from_slice(b"DATA\r\n");
    for line in raw.split_inclusive(|byte| *byte == b'\n') {
        if line.starts_with(b".") {
            input.push(b'.');
        }
        input.extend_from_slice(line);
    }
    if !raw.ends_with(b"\r\n") {
        input.extend_from_slice(b"\r\n");
    }
    input.extend_from_slice(b".\r\nQUIT\r\n");
    String::from_utf8_lossy(&fake.answer(&input)).into_owned()
}

/// The first message in the fake IMAP server's INBOX, as `UID FETCH ... BODY[]` gives it.
fn delivered(imap: &FakeImap) -> Vec<u8> {
    let out = imap.answer(b"a1 LOGIN ada@example.org not-a-real-password\r\na2 SELECT INBOX\r\na3 UID FETCH 1:* BODY[]\r\n");
    let start = out.windows(8).position(|window| window == b"BODY[] {").unwrap() + 8;
    let close = start + out[start..].iter().position(|byte| *byte == b'}').unwrap();
    let length: usize = String::from_utf8_lossy(&out[start..close]).parse().unwrap();
    out[close + 3..close + 3 + length].to_vec()
}

/// The summary of `raw` with UID `n` from `from` to `to`, as the sync reads its header.
fn summary(n: u32, raw: &[u8], from: &str, to: &str) -> Summary {
    let end = raw.windows(4).position(|window| window == b"\r\n\r\n").unwrap() + 4;
    let headers = Headers::parse(&raw[..end]).unwrap();
    let fields = headers.fields().iter().map(|field| (field.name(), field.value()));
    let envelope = Envelope::new(&headers.text("subject").unwrap_or_default())
        .with_from(vec![address("From", from)])
        .with_to(vec![address("To", to)])
        .with_message_id(headers.get("message-id").and_then(MessageRef::new))
        .with_signs(Signs::read(fields));
    Summary::new(Uid::new(n).unwrap(), envelope)
}

fn event(kind: EventKind) -> Event {
    Event::new(AccountId::new(1), None, kind)
}

fn arrive(session: &mut Session, folder: u64, summary: Summary) {
    session.apply(&event(EventKind::MessagesAdded { folder: FolderId::new(folder), uid_validity: validity(), messages: vec![summary] })).unwrap();
}

#[test]
fn a_refusal_shows_on_the_outbox_item_with_the_reply_and_its_code_and_the_draft_is_kept() {
    let (mut session, clock) = session();
    let fake = FakeSmtp::new().refuse(When::rcpt(NOBODY), Reply::new(550, "5.1.1", "No such user"));
    let (id, _, recipients, raw) = send(&mut session, &clock);
    let out = submit(&fake, &recipients, &raw);
    // The worker reports the refusal with the server's reply line, as `net` does.
    let reply = out.lines().find(|line| line.starts_with("550")).unwrap().to_owned();
    assert_eq!(reply, "550 5.1.1 No such user");
    assert!(fake.outbox().messages().is_empty());
    session.apply(&event(EventKind::SendFailed { draft: DraftId::new(id), kind: ErrorKind::MessageRefused, reply: Some(reply) })).unwrap();
    let listed = session.run("outbox.list", &json!({})).unwrap();
    let item = &listed["items"][0];
    assert_eq!(
        (&item["state"], &item["reply"], &item["code"], &item["permanent"]),
        (&json!("refused"), &json!("550 5.1.1 No such user"), &json!("5.1.1"), &json!(true))
    );
    let draft = session.store().draft(DraftId::new(id)).unwrap().unwrap();
    assert_eq!((draft.to.as_slice(), draft.text.as_str()), ([NOBODY.to_owned()].as_slice(), "The numbers."));
    // Sent again by command, to a server that takes it now.
    session.run("compose.update", &json!({ "id": id, "to": ["bela@example.org"] })).unwrap();
    session.take_jobs();
    assert_eq!(send_now(&mut session, &clock, "outbox.retry", id)["state"], json!("waiting"));
    let jobs = session.take_jobs();
    let [job] = jobs.as_slice() else { panic!("{jobs:?}") };
    let JobKind::Send { recipients, raw, .. } = &job.kind else { panic!("{job:?}") };
    let recipients: Vec<String> = recipients.iter().map(|address| address.as_str().to_owned()).collect();
    let out = submit(&fake, &recipients, raw);
    assert!(out.contains("250 2.0.0 Queued as 1\r\n"), "{out}");
    session.apply(&event(EventKind::Sent { draft: DraftId::new(id) })).unwrap();
    assert_eq!(session.run("outbox.list", &json!({})).unwrap()["items"], json!([]));
}

#[test]
fn the_fakes_delivery_report_marks_the_sent_message_as_failed() {
    let (mut session, clock) = session();
    let imap = FakeImap::new();
    let fake = FakeSmtp::new().bounce(Bounce::new(NOBODY, &imap.postbox()));
    let (id, message_id, recipients, raw) = send(&mut session, &clock);
    let out = submit(&fake, &recipients, &raw);
    assert!(out.contains("250 2.0.0 Queued as 1\r\n"), "{out}");
    session.apply(&event(EventKind::Sent { draft: DraftId::new(id) })).unwrap();
    // The Sent copy comes back with the next sync of the Sent folder.
    let appended = session.take_jobs().into_iter().find_map(|job| match job.kind {
        JobKind::Append { raw, .. } => Some(raw),
        _ => None,
    });
    let sent = summary(5, &appended.unwrap(), "anna@example.org", NOBODY);
    assert_eq!(sent.envelope().message_id(), MessageRef::new(&message_id).as_ref());
    arrive(&mut session, 2, sent);
    // The report lands in the inbox, from the fake's MAILER-DAEMON, and its body is asked for.
    let report = delivered(&imap);
    arrive(&mut session, 1, summary(9, &report, "MAILER-DAEMON@smtp.example.org", "anna@example.org"));
    let jobs = session.take_jobs();
    assert!(jobs.iter().any(|job| matches!(job.kind, JobKind::FetchBody { uid, .. } if uid.get() == 9)), "{jobs:?}");
    session
        .apply(&event(EventKind::BodyFetched { folder: FolderId::new(1), uid_validity: validity(), uid: Uid::new(9).unwrap(), raw: report }))
        .unwrap();
    let listed = session.run("mail.list", &json!({ "folder": 2 })).unwrap();
    let rows = listed["messages"].as_array().unwrap();
    let row = rows.iter().find(|row| row["subject"] == json!("Budget")).unwrap();
    assert_eq!(row["bounced"]["reason"], json!("nobody@example.org: 550 5.1.1 The mailbox does not exist"));
    // The report is the inbox's message, not held by the screener.
    let report = row["bounced"]["report"].as_u64().unwrap();
    let held = session.store().message(MessageId::new(report)).unwrap().unwrap();
    assert_eq!(held.message().folder(), FolderId::new(1));
    assert_ne!(row["bounced"], Value::Null);
}
