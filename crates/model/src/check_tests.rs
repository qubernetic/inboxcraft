// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{CheckStep, MAX_ISSUER_CHARS, StepReport, StepStatus};
use crate::{ErrorKind, Host};

#[test]
fn the_steps_run_in_the_order_of_the_check() {
    assert_eq!(
        CheckStep::ALL,
        [
            CheckStep::Dns,
            CheckStep::Tcp,
            CheckStep::Tls,
            CheckStep::Certificate,
            CheckStep::ImapLogin,
            CheckStep::SmtpLogin,
            CheckStep::CaldavDiscovery
        ]
    );
    assert!(CheckStep::ALL.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn only_the_smtp_and_caldav_steps_are_not_built_yet() {
    let not_built: Vec<CheckStep> = CheckStep::ALL.into_iter().filter(|step| !step.is_built()).collect();
    assert_eq!(not_built, [CheckStep::SmtpLogin, CheckStep::CaldavDiscovery]);
}

#[test]
fn the_plan_lists_every_step_pending_or_not_built() {
    let plan = CheckStep::plan();
    let steps: Vec<CheckStep> = plan.iter().map(|report| report.step()).collect();
    assert_eq!(steps, CheckStep::ALL);
    for report in &plan {
        let expected = if report.step().is_built() { StepStatus::Pending } else { StepStatus::NotBuilt };
        assert_eq!(report.status(), expected, "{:?}", report.step());
        assert_eq!((report.host(), report.port(), report.issuer()), (None, None, None));
    }
}

#[test]
fn a_report_names_the_server_and_the_issuer() {
    let report = StepReport::new(CheckStep::Certificate, StepStatus::Failed(ErrorKind::CertificateExpired))
        .with_server(Host::new("imap.example.org").unwrap(), 993)
        .with_issuer(Some("CN=Example CA, O=Example"));
    assert_eq!(report.host().map(Host::as_str), Some("imap.example.org"));
    assert_eq!(report.port(), Some(993));
    assert_eq!(report.issuer(), Some("CN=Example CA, O=Example"));
    assert_eq!(report.status().error(), Some(ErrorKind::CertificateExpired));
    assert!(!report.status().passed());
    assert!(StepStatus::Passed.passed());
    assert_eq!(StepStatus::Passed.error(), None);
}

#[test]
fn an_issuer_from_a_certificate_is_cleaned_and_capped() {
    let report = StepReport::new(CheckStep::Certificate, StepStatus::Passed).with_issuer(Some("CN=Evil\r\nX-Injected: 1\u{0}\u{202e}"));
    assert_eq!(report.issuer(), Some("CN=Evil X-Injected: 1"));
    let long = "A".repeat(MAX_ISSUER_CHARS * 2);
    let report = StepReport::new(CheckStep::Certificate, StepStatus::Passed).with_issuer(Some(&long));
    assert_eq!(report.issuer().map(|issuer| issuer.chars().count()), Some(MAX_ISSUER_CHARS));
    let report = StepReport::new(CheckStep::Certificate, StepStatus::Passed).with_issuer(Some("\r\n\u{0}"));
    assert_eq!(report.issuer(), None);
}

#[test]
fn reports_round_trip_and_are_cleaned_when_read_back() {
    for status in [
        StepStatus::Pending,
        StepStatus::Passed,
        StepStatus::Failed(ErrorKind::MailPortsBlocked),
        StepStatus::Skipped,
        StepStatus::NeedsReview,
        StepStatus::NeedsOwner,
        StepStatus::NotBuilt,
    ] {
        let report = StepReport::new(CheckStep::Tcp, status).with_server(Host::new("imap.example.org").unwrap(), 993);
        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(serde_json::from_str::<StepReport>(&json).unwrap(), report);
    }
    let stored = r#"{"step":"Certificate","status":"Passed","host":null,"port":null,"issuer":"CN=A\r\nB"}"#;
    assert_eq!(serde_json::from_str::<StepReport>(stored).unwrap().issuer(), Some("CN=A B"));
}
