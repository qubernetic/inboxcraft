// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP for Detect: the autoconfig file at the provider and in Mozilla's ISPDB.
//!
//! [`Https`] is `ureq` with `rustls`, the `ring` provider and the same trust as the mail connections ([`Trust`]):
//! the platform's verifier, or given roots. It follows no redirect (Detect decides which one to take), sends no
//! `User-Agent`, `Accept` or `Accept-Encoding`, no cookies and no proxy, and reads at most [`MAX_BODY`] bytes, so a
//! request tells the server nothing but the URL.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use ureq::config::{AutoHeaderValue, Config};
use ureq::http::Uri;
use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};
use ureq::{Agent, Error};

use crate::{NetError, TlsSettings, Trust, tls};

/// The largest answer read, in bytes: one more than the largest autoconfig file, so a larger one is refused.
pub const MAX_BODY: u64 = crate::autoconfig::MAX_FILE as u64 + 1;

/// What a server answered to a GET.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAnswer {
    /// The status code.
    pub status: u16,
    /// The `Location` header of a redirect.
    pub location: Option<String>,
    /// The body, at most [`MAX_BODY`] bytes.
    pub body: Vec<u8>,
}

/// What fetches a URL: [`Https`], or a test's stand-in.
pub trait Web: fmt::Debug + Send + Sync {
    /// GETs `url` without following a redirect.
    ///
    /// # Errors
    ///
    /// A [`NetError`] with the cause: the name, the connection, TLS, a timeout, or an answer larger than
    /// [`MAX_BODY`] ([`ErrorKind::TooLarge`]).
    fn get(&self, url: &str) -> Result<WebAnswer, NetError>;
}

/// HTTPS (and the one plain HTTP request Detect allows) through `ureq`.
pub struct Https {
    agent: Agent,
    tls: TlsSettings,
}

impl fmt::Debug for Https {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Https").field("trust", self.tls.trust()).finish_non_exhaustive()
    }
}

impl Https {
    /// A client with `tls`'s trust, waiting at most `timeout` for a whole request; host names are looked up by the
    /// system.
    #[must_use]
    pub fn new(tls: TlsSettings, timeout: Duration) -> Self {
        let agent = Agent::new_with_config(config(&tls, timeout));
        Self { agent, tls }
    }

    /// As [`Https::new`], connecting to `addresses` whatever the host: for the tests, whose fake servers listen on
    /// 127.0.0.1. The certificate is still checked for the URL's host.
    #[must_use]
    pub fn with_addresses(tls: TlsSettings, timeout: Duration, addresses: Vec<SocketAddr>) -> Self {
        let agent = Agent::with_parts(config(&tls, timeout), DefaultConnector::default(), Fixed(addresses));
        Self { agent, tls }
    }
}

impl Web for Https {
    fn get(&self, url: &str) -> Result<WebAnswer, NetError> {
        let now = self.tls.clock().now();
        let failed = |error: Error| match error {
            Error::Timeout(_) => NetError::new(ErrorKind::TimedOut, "the server did not answer in time"),
            Error::HostNotFound => NetError::new(ErrorKind::HostNotFound, "the host name was not found"),
            Error::ConnectionFailed => NetError::new(ErrorKind::ConnectionRefused, "the connection failed"),
            Error::Io(io) => NetError::io(&io, now),
            Error::Rustls(tls) => NetError::new(tls::classify(&tls, now), tls.to_string()),
            Error::Tls(text) => NetError::new(ErrorKind::Tls, text),
            Error::BodyExceedsLimit(limit) => NetError::new(ErrorKind::TooLarge, format!("the answer is larger than {limit} bytes")),
            other => NetError::new(ErrorKind::Protocol, other.to_string()),
        };
        let mut response = self.agent.get(url).call().map_err(failed)?;
        let status = response.status().as_u16();
        let location = response.headers().get("location").and_then(|value| value.to_str().ok()).map(str::to_owned);
        let body = response.body_mut().with_config().limit(MAX_BODY).read_to_vec().map_err(failed)?;
        Ok(WebAnswer { status, location, body })
    }
}

fn config(tls: &TlsSettings, timeout: Duration) -> Config {
    let roots = match tls.trust() {
        Trust::Platform => RootCerts::PlatformVerifier,
        Trust::Roots(roots) => RootCerts::Specific(Arc::new(roots.iter().map(|root| Certificate::from_der(root.as_ref()).to_owned()).collect())),
    };
    let tls = TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .root_certs(roots)
        .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .build();
    Agent::config_builder()
        .tls_config(tls)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
        .user_agent(AutoHeaderValue::None)
        .accept(AutoHeaderValue::None)
        .accept_encoding(AutoHeaderValue::None)
        .proxy(None)
        .build()
}

/// A resolver that gives the same addresses for every host.
#[derive(Debug)]
struct Fixed(Vec<SocketAddr>);

impl Resolver for Fixed {
    fn resolve(&self, uri: &Uri, config: &Config, timeout: NextTimeout) -> Result<ResolvedSocketAddrs, Error> {
        if self.0.is_empty() {
            return DefaultResolver::default().resolve(uri, config, timeout);
        }
        let mut found = self.empty();
        for address in &self.0 {
            if found.try_push(*address).is_err() {
                break;
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
