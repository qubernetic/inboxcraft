// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The account check: its steps and what each found.
//!
//! `account.check` explains a failed connection step by step instead of "check your firewall": the name, the
//! connection, the TLS handshake, the certificate, the IMAP sign-in, then the SMTP sign-in and the CalDAV discovery.
//! Each step reports its outcome with the host and port it tried and, for the certificate, the issuer.

use serde::{Deserialize, Serialize};

use crate::text::clean_line;
use crate::{ErrorKind, Host};

/// The longest issuer kept, in characters.
pub const MAX_ISSUER_CHARS: usize = 256;

/// One step of the account check, in the order they run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CheckStep {
    /// The server's name resolves to an address.
    Dns,
    /// A TCP connection to the port opens.
    Tcp,
    /// The server speaks TLS (after STARTTLS on a STARTTLS port) up to its certificate, which the next step checks.
    Tls,
    /// The certificate is trusted, valid at this computer's clock and for the server's name.
    Certificate,
    /// The IMAP server takes the user name and password.
    ImapLogin,
    /// The submission (SMTP) server takes them.
    SmtpLogin,
    /// The CalDAV server and the owner's calendars are found.
    CaldavDiscovery,
}

impl CheckStep {
    /// Every step, in order.
    pub const ALL: [Self; 7] = [Self::Dns, Self::Tcp, Self::Tls, Self::Certificate, Self::ImapLogin, Self::SmtpLogin, Self::CaldavDiscovery];

    /// Whether this version runs the step; the others report [`StepStatus::NotBuilt`].
    #[must_use]
    pub const fn is_built(self) -> bool {
        !matches!(self, Self::SmtpLogin | Self::CaldavDiscovery)
    }

    /// Every step before the check runs: pending, or not built yet.
    #[must_use]
    pub fn plan() -> Vec<StepReport> {
        Self::ALL.into_iter().map(|step| StepReport::new(step, if step.is_built() { StepStatus::Pending } else { StepStatus::NotBuilt })).collect()
    }
}

/// How a step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepStatus {
    /// Not run yet.
    Pending,
    /// The step passed.
    Passed,
    /// The step failed, for this cause.
    Failed(ErrorKind),
    /// Not run, because an earlier step failed.
    Skipped,
    /// Not run: the settings were detected and the owner has not seen them yet, so no password went out.
    NeedsReview,
    /// Not run: the last sign-in was refused or went unanswered, and only a check the owner starts sends the
    /// password again.
    NeedsOwner,
    /// Not run: this version does not have the step yet.
    NotBuilt,
}

impl StepStatus {
    /// Whether the step passed.
    #[must_use]
    pub const fn passed(self) -> bool {
        matches!(self, Self::Passed)
    }

    /// The cause of a failed step.
    #[must_use]
    pub const fn error(self) -> Option<ErrorKind> {
        match self {
            Self::Failed(kind) => Some(kind),
            _ => None,
        }
    }
}

/// What one step found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawReport")]
pub struct StepReport {
    step: CheckStep,
    status: StepStatus,
    host: Option<Host>,
    port: Option<u16>,
    issuer: Option<String>,
}

/// A [`StepReport`] as stored, before its issuer is cleaned.
#[derive(Deserialize)]
struct RawReport {
    step: CheckStep,
    status: StepStatus,
    host: Option<Host>,
    port: Option<u16>,
    issuer: Option<String>,
}

impl From<RawReport> for StepReport {
    fn from(raw: RawReport) -> Self {
        let report = Self { step: raw.step, status: raw.status, host: raw.host, port: raw.port, issuer: None };
        report.with_issuer(raw.issuer.as_deref())
    }
}

impl StepReport {
    /// `step` ended with `status`.
    #[must_use]
    pub fn new(step: CheckStep, status: StepStatus) -> Self {
        Self { step, status, host: None, port: None, issuer: None }
    }

    /// With the host and port the step tried.
    #[must_use]
    pub fn with_server(mut self, host: Host, port: u16) -> Self {
        self.host = Some(host);
        self.port = Some(port);
        self
    }

    /// With the issuer of the server's certificate, as one clean line of at most [`MAX_ISSUER_CHARS`] characters:
    /// it comes from the server.
    #[must_use]
    pub fn with_issuer(mut self, issuer: Option<&str>) -> Self {
        self.issuer = issuer.and_then(|issuer| clean_line(issuer, MAX_ISSUER_CHARS));
        self
    }

    /// The step.
    #[must_use]
    pub fn step(&self) -> CheckStep {
        self.step
    }

    /// How it ended.
    #[must_use]
    pub fn status(&self) -> StepStatus {
        self.status
    }

    /// The host it tried.
    #[must_use]
    pub fn host(&self) -> Option<&Host> {
        self.host.as_ref()
    }

    /// The port it tried.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// The issuer of the server's certificate.
    #[must_use]
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
