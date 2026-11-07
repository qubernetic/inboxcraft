// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The account worker sends through the fake SMTP server: a Send job goes out on a submission connection of its own,
//! to the envelope's recipients alone, and ends with `Sent` or with `SendFailed` and its cause.

mod support;

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use inboxcraft_model::{Account, AccountId, DraftId, EmailAddress, ErrorKind, Event, EventKind, Job, JobId, JobKind, Security};
use inboxcraft_net::{Backoff, Credentials, Password, Worker, WorkerSettings};
use inboxcraft_testkit::imap::FakeImap;
use inboxcraft_testkit::smtp::{FakeSmtp, Outbox, PASSWORD, Reply, USER, When};
use inboxcraft_testkit::{CertFault, FakeServer, Listen, TestPki};
use support::{Result, connector, far_off, server, submission, wall};

const ACCOUNT: AccountId = AccountId::new(1);
const DRAFT: DraftId = DraftId::new(7);
const WAIT: Duration = Duration::from_secs(10);
const MESSAGE: &[u8] = b"From: ada@example.org\r\nTo: bea@example.org\r\nSubject: Hi\r\n\r\nHello\r\n";

/// A password, or none; the same for both servers, as an account has one.
struct Fixed(Option<&'static str>);

impl Credentials for Fixed {
    fn password(&self, _account: &Account) -> Option<Password> {
        self.0.map(|text| Password::new(text.to_owned()))
    }
}

/// A worker whose account has its IMAP server on `imap` and its submission server (implicit TLS) on `smtp`.
struct Sending {
    worker: Worker,
    events: Receiver<Event>,
    imap: FakeServer,
    smtp: FakeServer,
    next: u64,
}

impl Sending {
    fn start(pki: &TestPki, imap: FakeImap, smtp: FakeSmtp, password: Option<&'static str>, fault: Option<CertFault>) -> Result<Self> {
        Self::with(pki, imap, smtp, password, fault, Some(Security::Tls))
    }

    fn with(
        pki: &TestPki,
        imap: FakeImap,
        smtp: FakeSmtp,
        password: Option<&'static str>,
        fault: Option<CertFault>,
        security: Option<Security>,
    ) -> Result<Self> {
        let imap = Listen::tls(pki.server_config(None)?).accept_timeout(Duration::from_secs(2)).start_imap(imap)?;
        let smtp = match security {
            Some(Security::StartTls) => Listen::plain().start_smtp(smtp.starttls(pki.server_config(fault)?))?,
            _ => Listen::tls(pki.server_config(fault)?).accept_timeout(Duration::from_secs(2)).start_smtp(smtp)?,
        };
        let submission_server = security.map(submission).transpose()?;
        let account =
            Account::new(ACCOUNT, "Test", EmailAddress::new(USER)?, USER, server(Security::Tls)?)?.with_submission(submission_server.clone());
        let clock = if fault == Some(CertFault::ClockFarOff) { far_off()? } else { wall() };
        let mut connector = connector(pki, clock, imap.addr());
        if let Some(on) = submission_server {
            connector = connector.with_addresses_on(on.port().get(), vec![smtp.addr()]);
        }
        let settings = WorkerSettings {
            backoff: Backoff { first: Duration::from_millis(20), cap: Duration::from_millis(80) },
            ..WorkerSettings::new(connector)
        };
        let (sender, events) = mpsc::channel();
        let worker = Worker::spawn(account, settings, Arc::new(Fixed(password)), sender)?;
        Ok(Self { worker, events, imap, smtp, next: 0 })
    }

    fn job(&mut self, kind: JobKind) -> Result<JobId> {
        self.next += 1;
        let id = JobId::new(self.next);
        self.worker.send(Job::new(id, ACCOUNT, kind)).map_err(|_| "job refused")?;
        Ok(id)
    }

    fn send(&mut self, to: &[&str]) -> Result<JobId> {
        let recipients = to.iter().map(|text| EmailAddress::new(text)).collect::<std::result::Result<Vec<_>, _>>()?;
        self.job(JobKind::Send { draft: DRAFT, from: EmailAddress::new(USER)?, recipients, raw: MESSAGE.to_vec() })
    }

    /// The events up to the end of the send `job`.
    fn sent(&self, job: JobId) -> Result<Event> {
        loop {
            let event = self.events.recv_timeout(WAIT)?;
            if event.job == Some(job) && matches!(event.kind, EventKind::Sent { .. } | EventKind::SendFailed { .. }) {
                return Ok(event);
            }
        }
    }

    /// Stops the worker; the lines the IMAP and the SMTP server got.
    fn finish(self) -> (Vec<String>, Vec<String>) {
        self.worker.stop();
        let (imap, _ended) = self.imap.finish_with_transcript();
        let (smtp, _ended) = self.smtp.finish_with_transcript();
        (imap.received().to_vec(), smtp.received().to_vec())
    }
}

fn failed(kind: ErrorKind, reply: Option<&str>) -> EventKind {
    EventKind::SendFailed { draft: DRAFT, kind, reply: reply.map(str::to_owned) }
}

#[test]
fn a_send_job_goes_to_every_recipient_of_the_envelope() -> Result {
    let pki = TestPki::new()?;
    let smtp = FakeSmtp::new();
    let outbox: Outbox = smtp.outbox();
    let mut run = Sending::start(&pki, FakeImap::new(), smtp, Some(PASSWORD), None)?;
    let job = run.send(&["bea@example.org", "hidden@example.org"])?;
    assert_eq!(run.sent(job)?.kind, EventKind::Sent { draft: DRAFT });
    let (imap, smtp) = run.finish();
    // The IMAP server is not needed for it.
    assert!(imap.is_empty(), "{imap:?}");
    assert!(smtp.last().is_some_and(|line| line == "QUIT"), "{smtp:?}");
    let sent = outbox.messages();
    let [message] = sent.as_slice() else { return Err(format!("{sent:?}").into()) };
    assert_eq!(message.recipients(), ["bea@example.org", "hidden@example.org"]);
    assert_eq!(message.data(), MESSAGE);
    assert_eq!(message.user(), Some(USER));
    Ok(())
}

#[test]
fn a_starttls_account_sends_after_the_handshake() -> Result {
    let pki = TestPki::new()?;
    let smtp = FakeSmtp::new().auth_in_clear();
    let outbox = smtp.outbox();
    let mut run = Sending::with(&pki, FakeImap::new(), smtp, Some(PASSWORD), None, Some(Security::StartTls))?;
    let job = run.send(&["bea@example.org"])?;
    assert_eq!(run.sent(job)?.kind, EventKind::Sent { draft: DRAFT });
    drop(run.finish());
    assert_eq!(outbox.messages().first().map(|sent| sent.tls()), Some(true));
    Ok(())
}

#[test]
fn a_refusal_names_the_servers_reply_and_sends_nothing() -> Result {
    let pki = TestPki::new()?;
    let smtp = FakeSmtp::new().refuse(When::rcpt("nobody@example.org"), Reply::new(550, "5.1.1", "No such user"));
    let outbox = smtp.outbox();
    let mut run = Sending::start(&pki, FakeImap::new(), smtp, Some(PASSWORD), None)?;
    let job = run.send(&["bea@example.org", "nobody@example.org"])?;
    assert_eq!(run.sent(job)?.kind, failed(ErrorKind::MessageRefused, Some("550 5.1.1 No such user")));
    drop(run.finish());
    assert!(outbox.messages().is_empty());
    // Without an enhanced code, the reply is the code and the words.
    let smtp =
        FakeSmtp::new().without(inboxcraft_testkit::smtp::Extension::EnhancedStatusCodes).refuse(When::Message, Reply::new(554, "", "Rejected"));
    let mut run = Sending::start(&pki, FakeImap::new(), smtp, Some(PASSWORD), None)?;
    let job = run.send(&["bea@example.org"])?;
    assert_eq!(run.sent(job)?.kind, failed(ErrorKind::MessageRefused, Some("554 Rejected")));
    drop(run.finish());
    Ok(())
}

#[test]
fn each_way_a_send_fails_has_its_own_kind() -> Result {
    let pki = TestPki::new()?;
    // No password stored: nothing is sent.
    let mut run = Sending::start(&pki, FakeImap::new(), FakeSmtp::new(), None, None)?;
    let job = run.send(&["bea@example.org"])?;
    assert_eq!(run.sent(job)?.kind, failed(ErrorKind::CredentialMissing, None));
    let (_imap, smtp) = run.finish();
    assert!(smtp.is_empty(), "{smtp:?}");
    // A password the submission server refuses.
    let mut run = Sending::start(&pki, FakeImap::new(), FakeSmtp::new().login(USER, "another-invented-password"), Some(PASSWORD), None)?;
    let job = run.send(&["bea@example.org"])?;
    assert_eq!(run.sent(job)?.kind, failed(ErrorKind::LoginRefused, None));
    drop(run.finish());
    // Each certificate fault of the submission server.
    for fault in CertFault::ALL {
        let mut run = Sending::start(&pki, FakeImap::new(), FakeSmtp::new(), Some(PASSWORD), Some(fault))?;
        let job = run.send(&["bea@example.org"])?;
        let expected = match fault {
            CertFault::UnknownCa => ErrorKind::CertificateUnknownIssuer,
            CertFault::Expired => ErrorKind::CertificateExpired,
            CertFault::WrongHost => ErrorKind::CertificateWrongHost,
            CertFault::ClockFarOff => ErrorKind::ClockFarOff,
        };
        assert_eq!(run.sent(job)?.kind, failed(expected, None), "{fault:?}");
        let (_imap, smtp) = run.finish();
        assert!(smtp.is_empty(), "{fault:?}: {smtp:?}");
    }
    // An account without a submission server.
    let mut run = Sending::with(&pki, FakeImap::new(), FakeSmtp::new(), Some(PASSWORD), None, None)?;
    let job = run.send(&["bea@example.org"])?;
    assert_eq!(run.sent(job)?.kind, failed(ErrorKind::NoSubmissionServer, None));
    drop(run.finish());
    // A recipient the client cannot send to (an empty envelope).
    let mut run = Sending::start(&pki, FakeImap::new(), FakeSmtp::new(), Some(PASSWORD), None)?;
    let job = run.send(&[])?;
    assert_eq!(run.sent(job)?.kind, failed(ErrorKind::CommandRefused, None));
    drop(run.finish());
    Ok(())
}

#[test]
fn after_a_refused_imap_password_nothing_is_sent() -> Result {
    let pki = TestPki::new()?;
    // The IMAP server refuses the password: the worker halts, and a send does not try it on the submission server.
    let mut run = Sending::start(&pki, FakeImap::new().login(USER, "another-invented-password"), FakeSmtp::new(), Some(PASSWORD), None)?;
    let sync = run.job(JobKind::SyncFolderList)?;
    loop {
        let event = run.events.recv_timeout(WAIT)?;
        if event.job == Some(sync) {
            break;
        }
    }
    let job = run.send(&["bea@example.org"])?;
    assert_eq!(run.sent(job)?.kind, failed(ErrorKind::LoginRefused, None));
    let (_imap, smtp) = run.finish();
    assert!(smtp.is_empty(), "{smtp:?}");
    Ok(())
}
