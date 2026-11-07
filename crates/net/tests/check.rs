// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `account.check` against the fake IMAP server: every step reports, each certificate fault and each port case has
//! its own kind and message, and no password goes out before the certificate verified and the owner saw the
//! settings.

mod support;

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::{CheckStep, ErrorKind, Security, StepReport, StepStatus};
use inboxcraft_net::{CheckTarget, Connector, Dialer, Password, check_account};
use inboxcraft_testkit::imap::{Capability, FakeImap, PASSWORD, USER};
use inboxcraft_testkit::smtp::FakeSmtp;
use inboxcraft_testkit::{CertFault, FakeServer, HOST, Listen, TestPki};
use support::{Result, connector, far_off, server, submission, wall};

/// A stand-in network: each port of the made-up server address goes to a local listener, refuses, or is silent.
#[derive(Debug, Default)]
struct Network {
    ports: HashMap<u16, Port>,
}

#[derive(Debug, Clone, Copy)]
enum Port {
    To(SocketAddr),
    Refused,
}

impl Network {
    fn port(mut self, number: u16, port: Port) -> Self {
        self.ports.insert(number, port);
        self
    }
}

impl Dialer for Network {
    fn dial(&self, address: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        match self.ports.get(&address.port()) {
            Some(Port::To(local)) => TcpStream::connect_timeout(local, timeout),
            Some(Port::Refused) => Err(io::ErrorKind::ConnectionRefused.into()),
            // A port the network drops: the connection never opens.
            None => Err(io::ErrorKind::TimedOut.into()),
        }
    }
}

/// The made-up address of the server; the [`Network`] decides what each of its ports does.
fn address(port: u16) -> SocketAddr {
    SocketAddr::from(([192, 0, 2, 1], port))
}

fn through(pki: &TestPki, clock: Arc<dyn inboxcraft_time::Clock>, network: Network) -> Connector {
    connector(pki, clock, address(993))
        .with_addresses_on(465, vec![address(465)])
        .with_addresses_on(587, vec![address(587)])
        .with_dialer(Arc::new(network))
}

fn to_fake(fake: &FakeServer) -> Network {
    Network::default().port(993, Port::To(fake.addr()))
}

/// Runs the check; the reports, and how often the password was asked for.
fn run(target: &CheckTarget, connector: &Connector, password: Option<&str>) -> (Vec<StepReport>, usize) {
    let mut reports = Vec::new();
    let mut asked = 0;
    check_account(
        target,
        connector,
        &mut || {
            asked += 1;
            password.map(|text| Password::new(text.to_owned()))
        },
        &mut |report| reports.push(report),
    );
    (reports, asked)
}

fn statuses(reports: &[StepReport]) -> Vec<StepStatus> {
    reports.iter().map(StepReport::status).collect()
}

fn status(reports: &[StepReport], step: CheckStep) -> Option<StepStatus> {
    reports.iter().find(|report| report.step() == step).map(StepReport::status)
}

fn target(security: Security) -> Result<CheckTarget> {
    Ok(CheckTarget::new(server(security)?, USER))
}

/// A fake submission server for `security`: implicit TLS, or STARTTLS on a plain connection.
fn submission_fake(pki: &TestPki, security: Security) -> Result<FakeServer> {
    Ok(match security {
        Security::Tls => Listen::tls(pki.server_config(None)?).start_smtp(FakeSmtp::new())?,
        Security::StartTls => Listen::plain().start_smtp(FakeSmtp::new().starttls(pki.server_config(None)?))?,
    })
}

/// The IMAP fake on 993 and `smtp` on the submission port of `security`.
fn both(imap: &FakeServer, smtp: &FakeServer, security: Security) -> Network {
    to_fake(imap).port(security.submission_port().get(), Port::To(smtp.addr()))
}

const PASSED: StepStatus = StepStatus::Passed;
const SKIPPED: StepStatus = StepStatus::Skipped;
const NOT_BUILT: StepStatus = StepStatus::NotBuilt;
const NOT_SET: StepStatus = StepStatus::NotSet;

fn signed_in(transcript: &[String]) -> bool {
    transcript.iter().any(|line| line.contains("LOGIN") || line.contains("AUTHENTICATE"))
}

#[test]
fn every_step_passes_and_names_the_server_and_the_issuer() -> Result {
    let pki = TestPki::new()?;
    for security in [Security::Tls, Security::StartTls] {
        let fake = match security {
            Security::Tls => Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?,
            Security::StartTls => Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(None)?))?,
        };
        let smtp = submission_fake(&pki, security)?;
        let target = target(security)?.with_submission(Some(submission(security)?));
        let (reports, asked) =
            run(&target, &through(&pki, wall(), to_fake(&fake).port(security.submission_port().get(), Port::To(smtp.addr()))), Some(PASSWORD));
        assert_eq!(statuses(&reports), [PASSED, PASSED, PASSED, PASSED, PASSED, PASSED, NOT_BUILT], "{security:?}");
        assert_eq!(reports.iter().map(StepReport::step).collect::<Vec<_>>(), CheckStep::ALL);
        let (imap, smtp_steps): (Vec<&StepReport>, Vec<&StepReport>) = reports.iter().partition(|report| report.step() != CheckStep::SmtpLogin);
        assert!(imap.iter().all(|report| report.host().map(|host| host.as_str()) == Some(HOST) && report.port() == Some(993)));
        assert!(smtp_steps.iter().all(|report| report.port() == Some(security.submission_port().get())), "{smtp_steps:?}");
        let smtp_transcript = smtp.finish()?;
        assert!(smtp_transcript.received().iter().any(|line| line.starts_with("AUTH")), "{security:?}");
        assert!(smtp_transcript.in_clear().iter().all(|line| !line.starts_with("AUTH")), "{security:?}");
        assert!(smtp_transcript.received().last().is_some_and(|line| line == "QUIT"));
        let issuer = reports.iter().find(|report| report.step() == CheckStep::Certificate).and_then(StepReport::issuer);
        assert_eq!(issuer, Some("CN=InboxCraft test CA"));
        assert_eq!(asked, 1);
        let transcript = fake.finish()?;
        assert!(signed_in(transcript.received()), "{:?}", transcript.received());
        assert!(transcript.received().last().is_some_and(|line| line.ends_with("LOGOUT")));
    }
    Ok(())
}

#[test]
fn each_certificate_fault_fails_the_certificate_step_with_its_own_message() -> Result {
    let pki = TestPki::new()?;
    let mut messages = HashSet::new();
    for fault in CertFault::ALL {
        let fake = Listen::tls(pki.server_config(Some(fault))?).start_imap(FakeImap::new())?;
        let clock = if fault == CertFault::ClockFarOff { far_off()? } else { wall() };
        let (reports, asked) = run(&target(Security::Tls)?, &through(&pki, clock, to_fake(&fake)), Some(PASSWORD));
        let expected = match fault {
            CertFault::UnknownCa => ErrorKind::CertificateUnknownIssuer,
            CertFault::Expired => ErrorKind::CertificateExpired,
            CertFault::WrongHost => ErrorKind::CertificateWrongHost,
            CertFault::ClockFarOff => ErrorKind::ClockFarOff,
        };
        assert_eq!(statuses(&reports), [PASSED, PASSED, PASSED, StepStatus::Failed(expected), SKIPPED, NOT_SET, NOT_BUILT], "{fault:?}");
        let issuer = reports.iter().find(|report| report.step() == CheckStep::Certificate).and_then(StepReport::issuer);
        let expected_issuer = if fault == CertFault::UnknownCa { "CN=Stranger test CA" } else { "CN=InboxCraft test CA" };
        assert_eq!(issuer, Some(expected_issuer), "{fault:?}");
        assert_eq!(asked, 0, "{fault:?}: no password is asked for before the certificate verified");
        let (transcript, _ended) = fake.finish_with_transcript();
        assert!(transcript.received().is_empty(), "{fault:?}: {:?}", transcript.received());
        assert!(messages.insert(expected.to_string()), "{fault:?}");
    }
    Ok(())
}

#[test]
fn a_certificate_fault_after_starttls_sends_no_password_either() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(Some(CertFault::Expired))?))?;
    let (reports, asked) = run(&target(Security::StartTls)?, &through(&pki, wall(), to_fake(&fake)), Some(PASSWORD));
    assert_eq!(status(&reports, CheckStep::Certificate), Some(StepStatus::Failed(ErrorKind::CertificateExpired)));
    assert_eq!(asked, 0);
    let (transcript, _ended) = fake.finish_with_transcript();
    assert_eq!(transcript.received(), ["A0001 STARTTLS"]);
    Ok(())
}

#[test]
fn each_port_case_has_its_own_kind_and_message() -> Result {
    let pki = TestPki::new()?;
    let web = TcpListener::bind("127.0.0.1:0")?;
    let mail = TcpListener::bind("127.0.0.1:0")?;
    let https = Port::To(web.local_addr()?);
    let cases = [
        (Network::default().port(993, Port::Refused), ErrorKind::ConnectionRefused),
        (Network::default(), ErrorKind::TimedOut),
        (Network::default().port(443, https), ErrorKind::MailPortsBlocked),
        (Network::default().port(443, https).port(587, Port::To(mail.local_addr()?)), ErrorKind::PortTimedOut),
        (Network::default().port(443, https).port(465, Port::Refused), ErrorKind::PortTimedOut),
    ];
    let mut messages = HashSet::new();
    for (network, expected) in cases {
        let (reports, asked) = run(&target(Security::Tls)?, &through(&pki, wall(), network), Some(PASSWORD));
        assert_eq!(statuses(&reports), [PASSED, StepStatus::Failed(expected), SKIPPED, SKIPPED, SKIPPED, NOT_SET, NOT_BUILT], "{expected:?}");
        assert_eq!(asked, 0);
        messages.insert(expected.to_string());
    }
    assert_eq!(messages.len(), 4, "{messages:?}");
    Ok(())
}

#[test]
fn a_port_that_is_not_a_mail_port_is_tested_against_all_three() -> Result {
    let pki = TestPki::new()?;
    let web = TcpListener::bind("127.0.0.1:0")?;
    let network = Network::default().port(443, Port::To(web.local_addr()?));
    let connector = connector(&pki, wall(), address(10_993)).with_dialer(Arc::new(network));
    let (reports, _asked) = run(&target(Security::Tls)?, &connector, Some(PASSWORD));
    assert_eq!(status(&reports, CheckStep::Tcp), Some(StepStatus::Failed(ErrorKind::MailPortsBlocked)));
    Ok(())
}

#[test]
fn a_name_without_an_address_fails_the_dns_step() -> Result {
    let pki = TestPki::new()?;
    let connector = Connector::new(inboxcraft_net::TlsSettings::new(inboxcraft_net::Trust::Roots(vec![pki.ca()]), wall())).with_addresses(Vec::new());
    let (reports, _asked) = run(&target(Security::Tls)?, &connector, Some(PASSWORD));
    assert_eq!(statuses(&reports), [StepStatus::Failed(ErrorKind::HostNotFound), SKIPPED, SKIPPED, SKIPPED, SKIPPED, NOT_SET, NOT_BUILT]);
    Ok(())
}

#[test]
fn a_server_without_starttls_fails_the_tls_step() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::plain().start_imap(FakeImap::new().without(Capability::LoginDisabled))?;
    let (reports, asked) = run(&target(Security::StartTls)?, &through(&pki, wall(), to_fake(&fake)), Some(PASSWORD));
    assert_eq!(statuses(&reports), [PASSED, PASSED, StepStatus::Failed(ErrorKind::StartTlsMissing), SKIPPED, SKIPPED, NOT_SET, NOT_BUILT]);
    assert_eq!(asked, 0);
    let (transcript, _ended) = fake.finish_with_transcript();
    assert!(transcript.received().is_empty(), "{:?}", transcript.received());
    Ok(())
}

#[test]
fn a_refused_password_and_a_missing_one_fail_the_sign_in() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let (reports, _asked) = run(&target(Security::Tls)?, &through(&pki, wall(), to_fake(&fake)), Some("wrong"));
    assert_eq!(status(&reports, CheckStep::ImapLogin), Some(StepStatus::Failed(ErrorKind::LoginRefused)));
    drop(fake.finish_with_transcript());

    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let (reports, asked) = run(&target(Security::Tls)?, &through(&pki, wall(), to_fake(&fake)), None);
    assert_eq!(status(&reports, CheckStep::ImapLogin), Some(StepStatus::Failed(ErrorKind::CredentialMissing)));
    assert_eq!(asked, 1);
    let transcript = fake.finish()?;
    assert!(!signed_in(transcript.received()), "{:?}", transcript.received());
    Ok(())
}

#[test]
fn detected_settings_get_no_password_until_the_owner_saw_them() -> Result {
    let pki = TestPki::new()?;
    let detected = CheckTarget::detected(server(Security::Tls)?, USER);
    assert!(!detected.is_reviewed());

    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let (reports, asked) = run(&detected, &through(&pki, wall(), to_fake(&fake)), Some(PASSWORD));
    assert_eq!(statuses(&reports), [PASSED, PASSED, PASSED, PASSED, StepStatus::NeedsReview, NOT_SET, NOT_BUILT]);
    assert_eq!(asked, 0);
    let transcript = fake.finish()?;
    assert!(!signed_in(transcript.received()), "{:?}", transcript.received());

    // Seen, but the certificate does not verify: still no password.
    let reviewed = detected.reviewed();
    assert!(reviewed.is_reviewed());
    let fake = Listen::tls(pki.server_config(Some(CertFault::UnknownCa))?).start_imap(FakeImap::new())?;
    let (_reports, asked) = run(&reviewed, &through(&pki, wall(), to_fake(&fake)), Some(PASSWORD));
    assert_eq!(asked, 0);
    let (transcript, _ended) = fake.finish_with_transcript();
    assert!(transcript.received().is_empty(), "{:?}", transcript.received());

    // Seen and verified: the password goes out.
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let (reports, asked) = run(&reviewed, &through(&pki, wall(), to_fake(&fake)), Some(PASSWORD));
    assert_eq!(status(&reports, CheckStep::ImapLogin), Some(PASSED));
    assert_eq!(asked, 1);
    assert!(signed_in(fake.finish()?.received()));
    Ok(())
}

#[test]
fn the_smtp_sign_in_fails_with_its_own_cause() -> Result {
    let pki = TestPki::new()?;
    let target = target(Security::Tls)?.with_submission(Some(submission(Security::Tls)?));
    // The submission server takes another password.
    let imap = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let smtp = Listen::tls(pki.server_config(None)?).start_smtp(FakeSmtp::new().login(USER, "another-invented-password"))?;
    let (reports, asked) = run(&target, &through(&pki, wall(), both(&imap, &smtp, Security::Tls)), Some(PASSWORD));
    assert_eq!(status(&reports, CheckStep::SmtpLogin), Some(StepStatus::Failed(ErrorKind::LoginRefused)));
    assert_eq!(asked, 1, "the password is asked for once for both sign-ins");
    drop((imap.finish()?, smtp.finish_with_transcript()));
    // Its certificate fails: no password goes to it.
    for fault in CertFault::ALL {
        let clock = if fault == CertFault::ClockFarOff { far_off()? } else { wall() };
        let imap = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
        let smtp = Listen::tls(pki.server_config(Some(fault))?).start_smtp(FakeSmtp::new())?;
        let (reports, _asked) = run(&target, &through(&pki, clock, both(&imap, &smtp, Security::Tls)), Some(PASSWORD));
        let smtp_status = status(&reports, CheckStep::SmtpLogin);
        let (_imap, _ended) = imap.finish_with_transcript();
        let (transcript, _ended) = smtp.finish_with_transcript();
        assert!(transcript.received().is_empty(), "{fault:?}: {:?}", transcript.received());
        if fault == CertFault::ClockFarOff {
            // The IMAP certificate fails first at that clock.
            assert_eq!(smtp_status, Some(SKIPPED));
        } else {
            let expected = match fault {
                CertFault::UnknownCa => ErrorKind::CertificateUnknownIssuer,
                CertFault::Expired => ErrorKind::CertificateExpired,
                CertFault::WrongHost | CertFault::ClockFarOff => ErrorKind::CertificateWrongHost,
            };
            assert_eq!(smtp_status, Some(StepStatus::Failed(expected)), "{fault:?}");
        }
    }
    // Nobody answers on the submission port.
    let imap = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let (reports, _asked) = run(&target, &through(&pki, wall(), to_fake(&imap)), Some(PASSWORD));
    assert_eq!(status(&reports, CheckStep::SmtpLogin), Some(StepStatus::Failed(ErrorKind::TimedOut)));
    drop(imap.finish()?);
    Ok(())
}

#[test]
fn the_smtp_sign_in_waits_like_the_imap_one() -> Result {
    let pki = TestPki::new()?;
    // Detected and not seen: neither server gets the password.
    let detected = CheckTarget::detected(server(Security::Tls)?, USER).with_submission(Some(submission(Security::Tls)?));
    let imap = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let smtp = submission_fake(&pki, Security::Tls)?;
    let (reports, asked) = run(&detected, &through(&pki, wall(), both(&imap, &smtp, Security::Tls)), Some(PASSWORD));
    assert_eq!(statuses(&reports), [PASSED, PASSED, PASSED, PASSED, StepStatus::NeedsReview, StepStatus::NeedsReview, NOT_BUILT]);
    assert_eq!(asked, 0);
    drop(imap.finish()?);
    let (transcript, _ended) = smtp.finish_with_transcript();
    assert!(transcript.received().is_empty(), "{:?}", transcript.received());
    // Held for the owner: the same.
    let held = target(Security::Tls)?.with_submission(Some(submission(Security::Tls)?)).held();
    let imap = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let (reports, asked) = run(&held, &through(&pki, wall(), to_fake(&imap)), Some(PASSWORD));
    assert_eq!(status(&reports, CheckStep::SmtpLogin), Some(StepStatus::NeedsOwner));
    assert_eq!(asked, 0);
    drop(imap.finish()?);
    // A refused IMAP password is not tried on the submission server.
    let imap = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let target = target(Security::Tls)?.with_submission(Some(submission(Security::Tls)?));
    let (reports, _asked) = run(&target, &through(&pki, wall(), to_fake(&imap)), Some("wrong"));
    assert_eq!(status(&reports, CheckStep::SmtpLogin), Some(SKIPPED));
    drop(imap.finish_with_transcript());
    Ok(())
}

#[test]
fn a_target_names_its_submission_server() -> Result {
    let target = target(Security::Tls)?;
    assert_eq!(target.submission(), None);
    let with = target.with_submission(Some(submission(Security::StartTls)?));
    assert_eq!(with.submission(), Some(&submission(Security::StartTls)?));
    Ok(())
}
