// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! TLS: what the client trusts, the clock it checks certificates with, and one error kind per certificate fault.
//!
//! The trust is the operating system's (`rustls-platform-verifier`: the system's roots and its own checks on
//! Linux, Windows, macOS, iOS and Android); tests and a later "trust this certificate" choice give roots of their
//! own. TLS 1.2 and 1.3 only, with `ring`'s safe defaults.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use inboxcraft_time::{Clock, Timestamp};
use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::time_provider::TimeProvider;
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, DistinguishedName, RootCertStore, SignatureScheme};

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
        self.config(None)
    }

    /// As [`TlsSettings::client_config`], keeping the server's certificate in `seen` before it is verified, so that
    /// `account.check` can name its issuer also when the verification fails. The verification is the same.
    pub(crate) fn client_config_seeing(&self, seen: &Seen) -> Result<Arc<ClientConfig>, NetError> {
        self.config(Some(seen))
    }

    fn config(&self, seen: Option<&Seen>) -> Result<Arc<ClientConfig>, NetError> {
        let now = self.clock.now();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let time = Arc::new(TimeOf(Arc::clone(&self.clock)));
        let builder = ClientConfig::builder_with_details(Arc::clone(&provider), time)
            .with_safe_default_protocol_versions()
            .map_err(|error| NetError::tls(&error, now))?;
        let verifier: Arc<dyn ServerCertVerifier> = match &self.trust {
            Trust::Platform => Arc::new(rustls_platform_verifier::Verifier::new(provider).map_err(|error| NetError::tls(&error, now))?),
            Trust::Roots(roots) => {
                let mut store = RootCertStore::empty();
                for root in roots {
                    store.add(root.clone()).map_err(|error| NetError::tls(&error, now))?;
                }
                WebPkiServerVerifier::builder_with_provider(Arc::new(store), provider)
                    .build()
                    .map_err(|error| NetError::new(ErrorKind::Tls, error.to_string()))?
            }
        };
        let verifier = match seen {
            Some(seen) => Arc::new(Recorder { inner: verifier, seen: seen.clone() }),
            None => verifier,
        };
        Ok(Arc::new(builder.dangerous().with_custom_certificate_verifier(verifier).with_no_client_auth()))
    }
}

/// The certificate a server presented last, as DER.
#[derive(Debug, Clone, Default)]
pub(crate) struct Seen(Arc<Mutex<Option<Vec<u8>>>>);

impl Seen {
    fn put(&self, der: &[u8]) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(der.to_vec());
    }

    /// The certificate, if a server presented one.
    pub(crate) fn get(&self) -> Option<Vec<u8>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

/// A verifier that keeps the end-entity certificate in [`Seen`], then asks the real verifier.
#[derive(Debug)]
struct Recorder {
    inner: Arc<dyn ServerCertVerifier>,
    seen: Seen,
}

impl ServerCertVerifier for Recorder {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        self.seen.put(end_entity.as_ref());
        self.inner.verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }

    fn requires_raw_public_keys(&self) -> bool {
        self.inner.requires_raw_public_keys()
    }

    fn root_hint_subjects(&self) -> Option<&[DistinguishedName]> {
        self.inner.root_hint_subjects()
    }
}

/// Whether a certificate valid from `not_before` to `not_after` points at the clock rather than at itself when it is
/// checked at `now`: the clock is before this program existed, the certificate ended more than four years ago, or it
/// starts more than a day ahead. The same test as [`classify`], with the dates read from the certificate, for the
/// platform verifiers that give none.
pub(crate) fn clock_far_off(now: Timestamp, not_before: Timestamp, not_after: Timestamp) -> bool {
    let seconds = |duration: Duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
    let now = now.unix_seconds();
    now < EARLIEST
        || now.saturating_sub(not_after.unix_seconds()) > seconds(LONG_EXPIRED)
        || not_before.unix_seconds().saturating_sub(now) > seconds(FAR_AHEAD)
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
