// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Remote pictures of a message, fetched only after the owner allowed them.
//!
//! A remote picture tells its server that the message was opened. [`PictureClient`] makes that request tell nothing
//! else: HTTPS only (a plain `http` address, and a redirect to one, is refused before anything is sent), the same
//! fixed `User-Agent` for everyone ([`USER_AGENT`]), no cookies (none are kept, so none go back), no `Referer`, no
//! `Accept` or `Accept-Encoding`, no proxy, and at most [`MAX_REDIRECTS`] redirects. Host names that resolve only
//! to addresses of this computer or the local network are not asked ([`is_public`]), so a message cannot make the
//! reader probe the owner's router. Only an `image/*` answer of at most [`MAX_PICTURE`] bytes is taken; the front
//! end decodes it.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use ureq::config::{AutoHeaderValue, Config};
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};
use ureq::{Agent, Error};

use crate::web::{Fixed, failed, tls_config};
use crate::{NetError, TlsSettings};

/// The largest picture taken, in bytes.
pub const MAX_PICTURE: u64 = 10 * 1024 * 1024;

/// The `User-Agent` of every picture request: the same for every InboxCraft, without a version.
pub const USER_AGENT: &str = "InboxCraft";

/// The most redirects followed for one picture.
pub const MAX_REDIRECTS: u32 = 3;

/// A fetched picture: its media type (`image/png`, lower case, without parameters) and its bytes, not decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The media type the server named.
    pub media_type: String,
    /// The bytes.
    pub bytes: Vec<u8>,
}

/// Fetches remote pictures through `ureq` with `rustls`, the `ring` provider and the trust of the mail connections.
pub struct PictureClient {
    agent: Agent,
    tls: TlsSettings,
}

impl fmt::Debug for PictureClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PictureClient").field("trust", self.tls.trust()).finish_non_exhaustive()
    }
}

impl PictureClient {
    /// A client with `tls`'s trust, waiting at most `timeout` for a whole picture; host names are looked up by the
    /// system and only public addresses are asked.
    #[must_use]
    pub fn new(tls: TlsSettings, timeout: Duration) -> Self {
        let agent = Agent::with_parts(config(&tls, timeout), DefaultConnector::default(), Public);
        Self { agent, tls }
    }

    /// As [`PictureClient::new`], connecting to `addresses` whatever the host: for the tests, whose fake servers
    /// listen on 127.0.0.1. The certificate is still checked for the URL's host.
    #[must_use]
    pub fn with_addresses(tls: TlsSettings, timeout: Duration, addresses: Vec<SocketAddr>) -> Self {
        let agent = Agent::with_parts(config(&tls, timeout), DefaultConnector::default(), Fixed(addresses));
        Self { agent, tls }
    }

    /// GETs the picture at `url`.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::Protocol`] for an address that is not `https`, a redirect to one, too many redirects, a status
    /// other than 200 or an answer that is no `image/*`; [`ErrorKind::TooLarge`] past [`MAX_PICTURE`];
    /// [`ErrorKind::HostNotFound`] for a host with no public address; otherwise the cause of the failed request.
    pub fn fetch(&self, url: &str) -> Result<Picture, NetError> {
        let scheme = url.get(..8).filter(|scheme| scheme.eq_ignore_ascii_case("https://"));
        if scheme.is_none() {
            return Err(NetError::new(ErrorKind::Protocol, "a remote picture is fetched over https only"));
        }
        let now = self.tls.clock().now();
        let mut response = self.agent.get(url).call().map_err(|error| failed(error, now))?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(NetError::new(ErrorKind::Protocol, format!("the server answered {status}")));
        }
        let media_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| value.strip_prefix("image/").is_some_and(|subtype| !subtype.is_empty()))
            .ok_or_else(|| NetError::new(ErrorKind::Protocol, "the answer is no picture"))?;
        let bytes = response.body_mut().with_config().limit(MAX_PICTURE).read_to_vec().map_err(|error| failed(error, now))?;
        Ok(Picture { media_type, bytes })
    }
}

fn config(tls: &TlsSettings, timeout: Duration) -> Config {
    Agent::config_builder()
        .tls_config(tls_config(tls))
        .https_only(true)
        .max_redirects(MAX_REDIRECTS)
        .save_redirect_history(false)
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
        .user_agent(AutoHeaderValue::Provided(std::sync::Arc::new(USER_AGENT.to_owned())))
        .accept(AutoHeaderValue::None)
        .accept_encoding(AutoHeaderValue::None)
        .proxy(None)
        .build()
}

/// Whether `ip` is an address on the internet: not this computer, the local network, link-local, shared (CGNAT),
/// multicast, broadcast, documentation, benchmarking or reserved space.
#[must_use]
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
            Some(mapped) => is_public_v4(mapped),
            None => {
                let documentation = ip.segments().first() == Some(&0x2001) && ip.segments().get(1) == Some(&0x0db8);
                !(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || ip.is_unique_local() || ip.is_unicast_link_local() || documentation)
            }
        },
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    let shared = a == 100 && (64..128).contains(&b);
    let protocol = a == 192 && b == 0 && c == 0;
    let benchmarking = a == 198 && (b == 18 || b == 19);
    let reserved = a >= 240 || a == 0;
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || shared
        || protocol
        || benchmarking
        || reserved)
}

/// The system's resolver, keeping only public addresses; a host without one is not found.
#[derive(Debug)]
struct Public;

impl Resolver for Public {
    fn resolve(&self, uri: &Uri, config: &Config, timeout: NextTimeout) -> Result<ResolvedSocketAddrs, Error> {
        let all = DefaultResolver::default().resolve(uri, config, timeout)?;
        let mut found = self.empty();
        for address in all.iter().filter(|address| is_public(address.ip())) {
            if found.try_push(*address).is_err() {
                break;
            }
        }
        if found.is_empty() { Err(Error::HostNotFound) } else { Ok(found) }
    }
}

#[cfg(test)]
#[path = "picture_tests.rs"]
mod tests;
