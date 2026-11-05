// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The test CA, the server certificates with their faults, and the client's trust store and clock.
//!
//! Every key and certificate is generated in memory when [`TestPki::new`] runs; nothing is read from or written to a
//! file.

use std::sync::Arc;
use std::time::Duration;

use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, UnixTime};
use rustls::time_provider::TimeProvider;
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use time::OffsetDateTime;

use crate::Error;

/// The host name the fake servers' certificates are for, and the name the test client asks for.
pub const HOST: &str = "mail.example.org";

/// The host name of the [`CertFault::WrongHost`] certificate.
pub const WRONG_HOST: &str = "elsewhere.example.org";

/// How far off, in seconds, the [`CertFault::ClockFarOff`] clock is: about twenty years.
pub const FAR_OFF: i64 = 20 * 365 * DAY;

/// A clock further off than this many seconds is far off.
const DAY: i64 = 86_400;

/// A certificate fault a fake server presents, or that the client meets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CertFault {
    /// A chain from a CA the test trust store does not hold.
    UnknownCa,
    /// A certificate that expired a year ago.
    Expired,
    /// A certificate for [`WRONG_HOST`] where the client asks for [`HOST`].
    WrongHost,
    /// A valid chain, but the client's clock is [`FAR_OFF`] ahead.
    ClockFarOff,
}

impl CertFault {
    /// Every certificate fault.
    pub const ALL: [Self; 4] = [Self::UnknownCa, Self::Expired, Self::WrongHost, Self::ClockFarOff];

    /// The clock the client uses to meet this fault: the wall clock, or [`FAR_OFF`] ahead for
    /// [`CertFault::ClockFarOff`].
    #[must_use]
    pub fn clock(self) -> Clock {
        match self {
            Self::ClockFarOff => Clock::offset(FAR_OFF),
            Self::UnknownCa | Self::Expired | Self::WrongHost => Clock::wall(),
        }
    }
}

/// The test client's clock: the wall clock moved by a number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    offset: i64,
}

impl Clock {
    /// The wall clock.
    #[must_use]
    pub const fn wall() -> Self {
        Self { offset: 0 }
    }

    /// The wall clock moved by `seconds`, forwards or backwards.
    #[must_use]
    pub const fn offset(seconds: i64) -> Self {
        Self { offset: seconds }
    }

    /// Whether the clock is more than a day away from the wall clock.
    #[must_use]
    pub const fn is_far_off(self) -> bool {
        self.offset.unsigned_abs() > DAY.unsigned_abs()
    }
}

impl TimeProvider for Clock {
    fn current_time(&self) -> Option<UnixTime> {
        let seconds = OffsetDateTime::now_utc().unix_timestamp().checked_add(self.offset)?;
        Some(UnixTime::since_unix_epoch(Duration::from_secs(u64::try_from(seconds).ok()?)))
    }
}

/// What a test client needs for TLS: the trust store holding the test CA, its clock and the host name it asks for.
#[derive(Debug, Clone)]
pub struct TlsClient {
    pub(crate) config: Arc<ClientConfig>,
    pub(crate) clock: Clock,
    pub(crate) host: String,
}

impl TlsClient {
    /// Asks for `name` instead of [`HOST`].
    #[must_use]
    pub fn host(self, name: &str) -> Self {
        Self { host: name.to_owned(), ..self }
    }
}

/// A test CA the client trusts, and a stranger CA it does not.
///
/// ```
/// use inboxcraft_testkit::{CertFault, TestPki};
///
/// # fn main() -> Result<(), inboxcraft_testkit::Error> {
/// let pki = TestPki::new()?;
/// let _expired = pki.server_config(Some(CertFault::Expired))?;
/// let _client = pki.client(CertFault::Expired.clock())?;
/// # Ok(())
/// # }
/// ```
pub struct TestPki {
    trusted: CertifiedIssuer<'static, KeyPair>,
    stranger: CertifiedIssuer<'static, KeyPair>,
}

impl TestPki {
    /// Generates both CAs.
    ///
    /// # Errors
    ///
    /// [`Error::Certificate`] when a key or certificate cannot be generated.
    pub fn new() -> Result<Self, Error> {
        Ok(Self { trusted: authority("InboxCraft test CA")?, stranger: authority("Stranger test CA")? })
    }

    /// The test CA's certificate, the one root of [`TestPki::client`]'s trust store.
    #[must_use]
    pub fn ca(&self) -> CertificateDer<'static> {
        self.trusted.der().clone()
    }

    /// A server configuration presenting a fresh certificate with `fault`, or a valid one for [`HOST`] with `None`.
    /// [`CertFault::ClockFarOff`] presents the valid certificate; the fault is the client's clock.
    ///
    /// # Errors
    ///
    /// [`Error::Certificate`] or [`Error::Tls`] when the certificate or the configuration cannot be made.
    pub fn server_config(&self, fault: Option<CertFault>) -> Result<Arc<ServerConfig>, Error> {
        let now = OffsetDateTime::now_utc();
        let valid = (now.saturating_sub(time::Duration::hours(1)), now.saturating_add(time::Duration::days(365)));
        let expired = (now.saturating_sub(time::Duration::days(730)), now.saturating_sub(time::Duration::days(365)));
        let (issuer, host, (not_before, not_after)) = match fault {
            Some(CertFault::UnknownCa) => (&self.stranger, HOST, valid),
            Some(CertFault::Expired) => (&self.trusted, HOST, expired),
            Some(CertFault::WrongHost) => (&self.trusted, WRONG_HOST, valid),
            None | Some(CertFault::ClockFarOff) => (&self.trusted, HOST, valid),
        };
        let key = KeyPair::generate()?;
        let mut params = CertificateParams::new(vec![host.to_owned()])?;
        params.distinguished_name.push(DnType::CommonName, host);
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.not_before = not_before;
        params.not_after = not_after;
        let leaf = params.signed_by(&key, issuer)?;
        let chain = vec![leaf.der().clone(), issuer.der().clone()];
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        let config = ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(chain, key)?;
        Ok(Arc::new(config))
    }

    /// A client that trusts only the test CA and reads `clock`.
    ///
    /// # Errors
    ///
    /// [`Error::Tls`] when the configuration cannot be made.
    pub fn client(&self, clock: Clock) -> Result<TlsClient, Error> {
        let mut roots = RootCertStore::empty();
        roots.add(self.ca())?;
        let config = ClientConfig::builder_with_details(provider(), Arc::new(clock))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(TlsClient { config: Arc::new(config), clock, host: HOST.to_owned() })
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn authority(name: &str) -> Result<CertifiedIssuer<'static, KeyPair>, Error> {
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
    params.distinguished_name.push(DnType::CommonName, name);
    Ok(CertifiedIssuer::self_signed(params, KeyPair::generate()?)?)
}
