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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
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
///
/// Every fetch has an agent of its own and keeps no connection: no TCP connection and no TLS session outlives one
/// picture, so a picture host cannot tie two fetches together by the connection or a resumed session.
pub struct PictureClient {
    tls: TlsSettings,
    timeout: Duration,
    addresses: Option<Vec<SocketAddr>>,
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
        Self { tls, timeout, addresses: None }
    }

    /// As [`PictureClient::new`], connecting to `addresses` whatever the host: for the tests, whose fake servers
    /// listen on 127.0.0.1. The certificate is still checked for the URL's host.
    #[must_use]
    pub fn with_addresses(tls: TlsSettings, timeout: Duration, addresses: Vec<SocketAddr>) -> Self {
        Self { tls, timeout, addresses: Some(addresses) }
    }

    /// A new agent, with a new TLS session cache, for one fetch.
    fn agent(&self) -> Agent {
        let config = config(&self.tls, self.timeout);
        match &self.addresses {
            Some(addresses) => Agent::with_parts(config, DefaultConnector::default(), Fixed(addresses.clone())),
            None => Agent::with_parts(config, DefaultConnector::default(), Public),
        }
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
        let mut response = self.agent().get(url).call().map_err(|error| failed(error, now))?;
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
        .max_idle_connections(0)
        .max_idle_connections_per_host(0)
        .build()
}

/// Whether `ip` is an address on the internet: not this computer, the local network, link-local, shared (CGNAT),
/// multicast, broadcast, documentation, benchmarking or reserved space.
#[must_use]
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => match carried_v4(ip) {
            Some(carried) => is_public_v4(carried),
            None => {
                let [first, second, third, ..] = ip.segments();
                let documentation = first == 0x2001 && second == 0x0db8;
                let local_nat64 = first == 0x0064 && second == 0xff9b && third == 0x0001;
                !(ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_multicast()
                    || ip.is_unique_local()
                    || ip.is_unicast_link_local()
                    || documentation
                    || local_nat64)
            }
        },
    }
}

/// The IPv4 address an IPv6 form carries: IPv4-mapped (`::ffff:0:0/96`), IPv4-compatible (`::/96`, deprecated, which
/// also holds `::` and `::1`), NAT64 (`64:ff9b::/96`) and 6to4 (`2002::/16`).
fn carried_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let [s0, s1, s2, s3, s4, s5, ..] = ip.segments();
    let [.., a, b, c, d] = ip.octets();
    let last = Ipv4Addr::new(a, b, c, d);
    match (s0, s1, s2, s3, s4, s5) {
        (0, 0, 0, 0, 0, 0xffff | 0) | (0x0064, 0xff9b, 0, 0, 0, 0) => Some(last),
        (0x2002, ..) => {
            let [high_a, high_b] = s1.to_be_bytes();
            let [low_a, low_b] = s2.to_be_bytes();
            Some(Ipv4Addr::new(high_a, high_b, low_a, low_b))
        }
        _ => None,
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
