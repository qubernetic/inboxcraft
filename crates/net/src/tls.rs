// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! TLS: what the client trusts, the clock it checks certificates with, and one error kind per certificate fault.
//!
//! The trust is the operating system's (`rustls-platform-verifier`: the system's roots and its own checks on
//! Linux, Windows, macOS, iOS and Android); tests and a later "trust this certificate" choice give roots of their
//! own. TLS 1.2 and 1.3 only, with `ring`'s safe defaults.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use inboxcraft_time::{Clock, Timestamp};
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, UnixTime};
use rustls::time_provider::TimeProvider;
use rustls::{CertificateError, ClientConfig, RootCertStore};

use crate::NetError;

/// A clock earlier than this (2026-01-01 00:00 UTC, before this program existed) is far off.
const EARLIEST: i64 = 1_767_225_600;

/// A certificate that ended longer ago than this (four years, longer than a public certificate may live) points at
/// the clock, not at the certificate.
const LONG_EXPIRED: Duration = Duration::from_secs(4 * 365 * 86_400);

/// A certificate that starts later than this (a day) points at the clock: no issuer dates a certificate ahead.
const FAR_AHEAD: Duration = Duration::from_secs(86_400);

/// What the client trusts.
#[derive(Clone, PartialEq, Eq)]
pub enum Trust {
    /// The operating system's trust store and verifier.
    Platform,
    /// Only these roots (DER), checked by `rustls`'s own verifier: the test CA, or a certificate the owner chose to
    /// trust.
    Roots(Vec<CertificateDer<'static>>),
}

impl fmt::Debug for Trust {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform => f.write_str("Platform"),
            Self::Roots(roots) => write!(f, "Roots({})", roots.len()),
        }
    }
}

/// How TLS connections are made: the [`Trust`] and the clock certificates are checked at.
#[derive(Debug, Clone)]
pub struct TlsSettings {
    trust: Trust,
    clock: Arc<dyn Clock>,
}

impl TlsSettings {
    /// `trust`, checking certificates at `clock`'s time.
    #[must_use]
    pub fn new(trust: Trust, clock: Arc<dyn Clock>) -> Self {
        Self { trust, clock }
    }

    /// The trust.
    #[must_use]
    pub fn trust(&self) -> &Trust {
        &self.trust
    }

    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// The `rustls` client configuration.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::Tls`] when the trust store cannot be loaded or a root is no certificate.
    pub fn client_config(&self) -> Result<Arc<ClientConfig>, NetError> {
        let now = self.clock.now();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let time = Arc::new(TimeOf(Arc::clone(&self.clock)));
        let builder = ClientConfig::builder_with_details(Arc::clone(&provider), time)
            .with_safe_default_protocol_versions()
            .map_err(|error| NetError::tls(&error, now))?;
        let config = match &self.trust {
            Trust::Platform => {
                let verifier = rustls_platform_verifier::Verifier::new(provider).map_err(|error| NetError::tls(&error, now))?;
                builder.dangerous().with_custom_certificate_verifier(Arc::new(verifier)).with_no_client_auth()
            }
            Trust::Roots(roots) => {
                let mut store = RootCertStore::empty();
                for root in roots {
                    store.add(root.clone()).map_err(|error| NetError::tls(&error, now))?;
                }
                let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(store), provider)
                    .build()
                    .map_err(|error| NetError::new(ErrorKind::Tls, error.to_string()))?;
                builder.dangerous().with_custom_certificate_verifier(verifier).with_no_client_auth()
            }
        };
        Ok(Arc::new(config))
    }
}

/// A [`Clock`] as `rustls` asks for the time.
#[derive(Debug)]
struct TimeOf(Arc<dyn Clock>);

impl TimeProvider for TimeOf {
    fn current_time(&self) -> Option<UnixTime> {
        let seconds = u64::try_from(self.0.now().unix_seconds()).ok()?;
        Some(UnixTime::since_unix_epoch(Duration::from_secs(seconds)))
    }
}

/// The error kind of a TLS failure at the clock's `now`: one per certificate fault.
///
/// A certificate outside its validity is [`ErrorKind::ClockFarOff`] when the clock is before this program existed,
/// when the certificate ended more than four years before the clock's time, or when it starts more than a day after
/// it: then the clock, not the certificate, is the thing to fix first. Where the platform's verifier gives no dates
/// (Windows, Apple, Android), only the first test applies; the clock check of `account.check` compares with the time
/// of a server.
#[must_use]
pub fn classify(error: &rustls::Error, now: Timestamp) -> ErrorKind {
    let rustls::Error::InvalidCertificate(fault) = error else { return ErrorKind::Tls };
    let clock_off = now.unix_seconds() < EARLIEST;
    match fault {
        CertificateError::UnknownIssuer => ErrorKind::CertificateUnknownIssuer,
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => ErrorKind::CertificateWrongHost,
        CertificateError::Expired if clock_off => ErrorKind::ClockFarOff,
        CertificateError::Expired => ErrorKind::CertificateExpired,
        CertificateError::ExpiredContext { time, not_after } => {
            if clock_off || gap(*not_after, *time) > LONG_EXPIRED {
                ErrorKind::ClockFarOff
            } else {
                ErrorKind::CertificateExpired
            }
        }
        CertificateError::NotValidYet if clock_off => ErrorKind::ClockFarOff,
        CertificateError::NotValidYet => ErrorKind::CertificateNotYetValid,
        CertificateError::NotValidYetContext { time, not_before } => {
            if clock_off || gap(*time, *not_before) > FAR_AHEAD {
                ErrorKind::ClockFarOff
            } else {
                ErrorKind::CertificateNotYetValid
            }
        }
        _ => ErrorKind::Tls,
    }
}

/// How long after `earlier` `later` is; zero when it is not after it.
fn gap(earlier: UnixTime, later: UnixTime) -> Duration {
    Duration::from_secs(later.as_secs().saturating_sub(earlier.as_secs()))
}

#[cfg(test)]
#[path = "tls_tests.rs"]
mod tests;
