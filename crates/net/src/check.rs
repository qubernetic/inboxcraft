// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The account check: one step after the other, each reported as it ends, so the owner is told the cause of a
//! failure and not "check your firewall".
//!
//! DNS (the system's lookup, as the worker connects), TCP, TLS (after STARTTLS on a STARTTLS port), the certificate
//! and the IMAP sign-in; the SMTP sign-in and the CalDAV discovery are listed and not built yet. A step after a failed
//! one is skipped.
//!
//! - A port that times out is looked at again: when port 443 of the same address connects and the other mail ports
//!   (993, 465, 587) time out too, the network blocks mail ([`ErrorKind::MailPortsBlocked`]); when another mail port
//!   answers, the port is likely wrong ([`ErrorKind::PortTimedOut`]); when 443 is silent too, the server does not
//!   answer ([`ErrorKind::TimedOut`]). Nothing is sent on these connections.
//! - The certificate step names the issuer, also when the verification fails, and checks the clock: a certificate
//!   outside its validity with this computer's clock far from it is [`ErrorKind::ClockFarOff`].
//! - The password is asked for and sent only after the TLS handshake verified the certificate, and only to settings
//!   the owner has seen: detected settings that were not reviewed stop before the sign-in with
//!   [`StepStatus::NeedsReview`].

use std::collections::BTreeSet;
use std::net::SocketAddr;

use inboxcraft_model::{Account, CheckStep, ErrorKind, Host, Server, StepReport, StepStatus};
use inboxcraft_time::Timestamp;

use crate::certificate::{self, Certificate};
use crate::connect::Stream;
use crate::tls::{self, Seen};
use crate::{Connector, ImapConnection, Password};

/// The mail ports the blocked-ports test tries: IMAP over TLS, and submission over TLS and with STARTTLS.
pub const MAIL_PORTS: [u16; 3] = [993, 465, 587];

/// The port that tells a network that blocks mail from a server that is down: HTTPS, which networks let through.
pub const HTTPS_PORT: u16 = 443;

/// What to check: the IMAP server and the user name, and whether the owner has seen them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTarget {
    imap: Server,
    username: String,
    reviewed: bool,
}

impl CheckTarget {
    /// Settings the owner entered or saved.
    #[must_use]
    pub fn new(imap: Server, username: &str) -> Self {
        Self { imap, username: username.to_owned(), reviewed: true }
    }

    /// Settings Detect proposed: no password goes to them until the owner has seen them ([`CheckTarget::reviewed`]).
    #[must_use]
    pub fn detected(imap: Server, username: &str) -> Self {
        Self { reviewed: false, ..Self::new(imap, username) }
    }

    /// The settings of a saved account.
    #[must_use]
    pub fn of(account: &Account) -> Self {
        Self::new(account.imap().clone(), account.username())
    }

    /// The same settings, now that the owner has seen them.
    #[must_use]
    pub fn reviewed(self) -> Self {
        Self { reviewed: true, ..self }
    }

    /// The IMAP server.
    #[must_use]
    pub fn imap(&self) -> &Server {
        &self.imap
    }

    /// The user name.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Whether the owner has seen the settings.
    #[must_use]
    pub fn is_reviewed(&self) -> bool {
        self.reviewed
    }
}

/// Checks `target` with `connector`, reporting every step of [`CheckStep::ALL`] to `report`, in order, as it ends.
/// `password` is called at most once, right before the sign-in.
pub fn check_account(
    target: &CheckTarget,
    connector: &Connector,
    password: &mut dyn FnMut() -> Option<Password>,
    report: &mut dyn FnMut(StepReport),
) {
    let server = target.imap();
    let mut out = Out { report, host: server.host().clone(), port: server.port().get(), told: BTreeSet::new() };
    let _stopped = steps(target, connector, password, &mut out);
    for step in CheckStep::ALL {
        if !out.told.contains(&step) {
            out.say(step, if step.is_built() { StepStatus::Skipped } else { StepStatus::NotBuilt }, None);
        }
    }
}

/// Where the steps go.
struct Out<'a> {
    report: &'a mut dyn FnMut(StepReport),
    host: Host,
    port: u16,
    told: BTreeSet<CheckStep>,
}

impl Out<'_> {
    fn say(&mut self, step: CheckStep, status: StepStatus, issuer: Option<&str>) {
        self.told.insert(step);
        (self.report)(StepReport::new(step, status).with_server(self.host.clone(), self.port).with_issuer(issuer));
    }

    /// Tells `step` failed with `kind`; `None`, so the steps stop.
    fn failed(&mut self, step: CheckStep, kind: ErrorKind) -> Option<()> {
        self.say(step, StepStatus::Failed(kind), None);
        None
    }
}

/// The built steps; `None` when one failed.
fn steps(target: &CheckTarget, connector: &Connector, password: &mut dyn FnMut() -> Option<Password>, out: &mut Out<'_>) -> Option<()> {
    let server = target.imap();
    let addresses = match connector.resolve(server) {
        Ok(addresses) => addresses,
        Err(error) => return out.failed(CheckStep::Dns, error.kind()),
    };
    out.say(CheckStep::Dns, StepStatus::Passed, None);
    let stream = match tcp(connector, &addresses) {
        Ok(stream) => stream,
        Err(kind) => return out.failed(CheckStep::Tcp, kind),
    };
    out.say(CheckStep::Tcp, StepStatus::Passed, None);
    let seen = Seen::default();
    let config = match connector.tls().client_config_seeing(&seen) {
        Ok(config) => config,
        Err(error) => return out.failed(CheckStep::Tls, error.kind()),
    };
    let opened = ImapConnection::over(stream, server, connector, &config);
    let presented = seen.get().and_then(|der| certificate::read(&der));
    let issuer = presented.as_ref().map(Certificate::issuer).filter(|issuer| !issuer.is_empty());
    let mut connection = match opened {
        Ok(connection) => connection,
        Err(error) if is_certificate_fault(error.kind()) => {
            out.say(CheckStep::Tls, StepStatus::Passed, None);
            let kind = clock_checked(error.kind(), presented.as_ref(), connector.tls().clock().now());
            out.say(CheckStep::Certificate, StepStatus::Failed(kind), issuer);
            return None;
        }
        Err(error) => return out.failed(CheckStep::Tls, error.kind()),
    };
    out.say(CheckStep::Tls, StepStatus::Passed, None);
    out.say(CheckStep::Certificate, StepStatus::Passed, issuer);
    let signed_in = if target.is_reviewed() {
        match password() {
            Some(password) => {
                connection.login(target.username(), &password).map_or_else(|error| StepStatus::Failed(error.kind()), |()| StepStatus::Passed)
            }
            None => StepStatus::Failed(ErrorKind::CredentialMissing),
        }
    } else {
        StepStatus::NeedsReview
    };
    connection.logout();
    out.say(CheckStep::ImapLogin, signed_in, None);
    signed_in.passed().then_some(())
}

/// A TCP connection to the first of `addresses` that answers; for a port that timed out, why (see the module).
fn tcp(connector: &Connector, addresses: &[SocketAddr]) -> Result<Stream, ErrorKind> {
    let mut last = ErrorKind::HostNotFound;
    let mut silent = None;
    for address in addresses {
        match connector.dial(*address) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                last = error.kind();
                if last == ErrorKind::TimedOut && silent.is_none() {
                    silent = Some(*address);
                }
            }
        }
    }
    Err(silent.map_or(last, |address| timed_out(connector, address)))
}

/// Why `address` timed out, from what its other ports do.
fn timed_out(connector: &Connector, address: SocketAddr) -> ErrorKind {
    let at = |port: u16| connector.dial(SocketAddr::new(address.ip(), port));
    if at(HTTPS_PORT).is_err() {
        return ErrorKind::TimedOut;
    }
    let others_silent = MAIL_PORTS
        .into_iter()
        .filter(|port| *port != address.port())
        .all(|port| at(port).err().is_some_and(|error| error.kind() == ErrorKind::TimedOut));
    if others_silent { ErrorKind::MailPortsBlocked } else { ErrorKind::PortTimedOut }
}

fn is_certificate_fault(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::CertificateUnknownIssuer
            | ErrorKind::CertificateExpired
            | ErrorKind::CertificateNotYetValid
            | ErrorKind::CertificateWrongHost
            | ErrorKind::ClockFarOff
    )
}

/// The clock check: a certificate outside its validity at a clock far from it is the clock's fault.
fn clock_checked(kind: ErrorKind, presented: Option<&Certificate>, now: Timestamp) -> ErrorKind {
    match (kind, presented) {
        (ErrorKind::CertificateExpired | ErrorKind::CertificateNotYetValid, Some(certificate))
            if tls::clock_far_off(now, certificate.not_before(), certificate.not_after()) =>
        {
            ErrorKind::ClockFarOff
        }
        _ => kind,
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
