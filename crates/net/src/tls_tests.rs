// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use inboxcraft_testkit::{HOST, TestPki};
use inboxcraft_time::{Clock, FakeClock, Timestamp};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::time_provider::TimeProvider;
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, DistinguishedName, Error, ServerConnection, SignatureScheme,
    SupportedProtocolVersion,
};

use super::{EARLIEST, Recorder, Seen, TimeOf, TlsSettings, Trust, classify};

const DAY: u64 = 86_400;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(seconds, 0).unwrap_or(Timestamp::UNIX_EPOCH)
}

fn unix(seconds: i64) -> UnixTime {
    UnixTime::since_unix_epoch(Duration::from_secs(u64::try_from(seconds).unwrap_or(0)))
}

/// A sane "now": a year after the earliest date the clock check accepts.
const NOW: i64 = EARLIEST + 365 * 86_400;

fn cert(fault: CertificateError) -> Error {
    Error::InvalidCertificate(fault)
}

#[test]
fn every_certificate_fault_has_its_own_kind() {
    let now = at(NOW);
    assert_eq!(classify(&cert(CertificateError::UnknownIssuer), now), ErrorKind::CertificateUnknownIssuer);
    assert_eq!(classify(&cert(CertificateError::NotValidForName), now), ErrorKind::CertificateWrongHost);
    assert_eq!(classify(&cert(CertificateError::Expired), now), ErrorKind::CertificateExpired);
    assert_eq!(classify(&cert(CertificateError::NotValidYet), now), ErrorKind::CertificateNotYetValid);
    assert_eq!(classify(&cert(CertificateError::BadSignature), now), ErrorKind::Tls);
    assert_eq!(classify(&Error::HandshakeNotComplete, now), ErrorKind::Tls);
}

#[test]
fn a_certificate_with_dates_tells_a_fault_from_a_far_off_clock() {
    let now = at(NOW);
    let expired = |ago: u64| cert(CertificateError::ExpiredContext { time: unix(NOW), not_after: unix(NOW - i64::try_from(ago).unwrap_or(0)) });
    assert_eq!(classify(&expired(365 * DAY), now), ErrorKind::CertificateExpired);
    assert_eq!(classify(&expired(4 * 365 * DAY), now), ErrorKind::CertificateExpired);
    assert_eq!(classify(&expired(4 * 365 * DAY + 1), now), ErrorKind::ClockFarOff);
    let ahead = |by: u64| cert(CertificateError::NotValidYetContext { time: unix(NOW), not_before: unix(NOW + i64::try_from(by).unwrap_or(0)) });
    assert_eq!(classify(&ahead(3600), now), ErrorKind::CertificateNotYetValid);
    assert_eq!(classify(&ahead(DAY + 1), now), ErrorKind::ClockFarOff);
}

#[test]
fn a_clock_before_this_program_existed_is_far_off() {
    let past = at(EARLIEST - 1);
    for fault in [
        CertificateError::Expired,
        CertificateError::NotValidYet,
        CertificateError::ExpiredContext { time: unix(EARLIEST - 1), not_after: unix(EARLIEST - 2) },
        CertificateError::NotValidYetContext { time: unix(EARLIEST - 1), not_before: unix(EARLIEST) },
    ] {
        assert_eq!(classify(&cert(fault), past), ErrorKind::ClockFarOff);
    }
    assert_eq!(classify(&cert(CertificateError::UnknownIssuer), past), ErrorKind::CertificateUnknownIssuer);
}

#[test]
fn rustls_reads_the_settings_clock() {
    let clock = Arc::new(FakeClock::new(at(NOW)));
    assert_eq!(TimeOf(clock.clone()).current_time(), Some(unix(NOW)));
    clock.set(at(-1));
    assert_eq!(TimeOf(clock).current_time(), None);
}

#[test]
fn both_trusts_make_a_configuration() {
    let clock: Arc<dyn Clock> = Arc::new(FakeClock::new(at(NOW)));
    assert!(TlsSettings::new(Trust::Platform, Arc::clone(&clock)).client_config().is_ok());
    let pki = inboxcraft_testkit::TestPki::new().ok();
    let roots = pki.iter().map(inboxcraft_testkit::TestPki::ca).collect::<Vec<_>>();
    let settings = TlsSettings::new(Trust::Roots(roots), clock);
    assert!(settings.client_config().is_ok());
    assert_eq!(format!("{:?}", settings.trust()), "Roots(1)");
    assert_eq!(format!("{:?}", Trust::Platform), "Platform");
}

#[test]
fn a_root_that_is_no_certificate_is_refused() {
    let clock: Arc<dyn Clock> = Arc::new(FakeClock::new(at(NOW)));
    let settings = TlsSettings::new(Trust::Roots(vec![CertificateDer::from(vec![1, 2, 3])]), Arc::clone(&clock));
    assert_eq!(settings.client_config().map(|_| ()).map_err(|error| error.kind()), Err(ErrorKind::Tls));
    // No root at all: nothing can be verified.
    let settings = TlsSettings::new(Trust::Roots(Vec::new()), clock);
    assert_eq!(settings.client_config().map(|_| ()).map_err(|error| error.kind()), Err(ErrorKind::Tls));
}

/// A verifier whose every answer can be told apart: a [`Recorder`] around it must hand each one on unchanged.
#[derive(Debug)]
struct Stub {
    hints: Vec<DistinguishedName>,
    /// A handshake with the test server needs `false`: it has no raw public key.
    raw_keys: bool,
}

const REFUSED_HOST: &str = "refused.example.org";

impl ServerCertVerifier for Stub {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        if server_name.to_str() == REFUSED_HOST { Err(Error::General("stub: certificate".to_owned())) } else { Ok(ServerCertVerified::assertion()) }
    }

    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Err(Error::General("stub: TLS 1.2 signature".to_owned()))
    }

    fn verify_tls13_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Err(Error::General("stub: TLS 1.3 signature".to_owned()))
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }

    fn requires_raw_public_keys(&self) -> bool {
        self.raw_keys
    }

    fn root_hint_subjects(&self) -> Option<&[DistinguishedName]> {
        Some(&self.hints)
    }
}

fn recorder(raw_keys: bool) -> Recorder {
    Recorder { inner: Arc::new(Stub { hints: vec![DistinguishedName::from(b"stub hint".to_vec())], raw_keys }), seen: Seen::default() }
}

/// A handshake in memory between the test PKI's server and a client verifying with `recorder` over `version`; the
/// client's error.
fn handshake(recorder: Recorder, version: &'static SupportedProtocolVersion) -> Result<(), Error> {
    let pki = TestPki::new().unwrap();
    let mut server = ServerConnection::new(pki.server_config(None).unwrap()).unwrap();
    let config = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[version])
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(recorder))
        .with_no_client_auth();
    let mut client = ClientConnection::new(Arc::new(config), ServerName::try_from(HOST).unwrap()).unwrap();
    for _ in 0..16 {
        let mut bytes = Vec::new();
        client.write_tls(&mut bytes).unwrap();
        server.read_tls(&mut bytes.as_slice()).unwrap();
        // The server's own error is the client's alert; the client's error is the point.
        server.process_new_packets().ok();
        let mut bytes = Vec::new();
        server.write_tls(&mut bytes).unwrap();
        client.read_tls(&mut bytes.as_slice()).unwrap();
        client.process_new_packets()?;
        if !client.is_handshaking() {
            return Ok(());
        }
    }
    Err(Error::General("the handshake did not end".to_owned()))
}

#[test]
fn the_recorder_hands_every_signature_check_to_the_real_verifier() {
    assert_eq!(handshake(recorder(false), &rustls::version::TLS13), Err(Error::General("stub: TLS 1.3 signature".to_owned())));
    assert_eq!(handshake(recorder(false), &rustls::version::TLS12), Err(Error::General("stub: TLS 1.2 signature".to_owned())));
}

#[test]
fn the_recorder_keeps_the_certificate_and_hands_on_every_other_answer() {
    let recorder = recorder(true);
    let der = CertificateDer::from(b"a certificate".to_vec());
    let refused = ServerName::try_from(REFUSED_HOST).unwrap();
    let answer = recorder.verify_server_cert(&der, &[], &refused, &[], unix(NOW));
    assert_eq!(answer.err(), Some(Error::General("stub: certificate".to_owned())));
    assert_eq!(recorder.seen.get(), Some(b"a certificate".to_vec()));
    assert_eq!(recorder.supported_verify_schemes(), recorder.inner.supported_verify_schemes());
    assert!(recorder.requires_raw_public_keys());
    let hints: Vec<&[u8]> = recorder.root_hint_subjects().unwrap_or_default().iter().map(AsRef::as_ref).collect();
    assert_eq!(hints, [b"stub hint".as_slice()]);
}
