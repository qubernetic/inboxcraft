// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Detect: a proposal of settings from the owner's address. Manual setup stays the default; Detect only fills in a
//! proposal, and nothing it finds gets a password before the owner has seen it and TLS has verified it
//! ([`CheckTarget::detected`]).
//!
//! In this order, stopping at the first source that names an IMAP server:
//!
//! 1. The MX records of the address's domain (DNS).
//! 2. Mozilla autoconfig at the domain, over HTTPS: `https://autoconfig.<domain>/mail/config-v1.1.xml`, then
//!    `https://<domain>/.well-known/autoconfig/mail/config-v1.1.xml`. Plain HTTP
//!    (`http://autoconfig.<domain>/...`) counts only as a redirect to HTTPS on the MX domain, which is then fetched.
//! 3. Mozilla's ISPDB, for the domain and then for the MX domain: only when it is switched on
//!    ([`DetectConfig::ispdb`]), because it tells Mozilla the domain.
//! 4. SRV records (RFC 6186, RFC 8314): `_imaps._tcp`, `_imap._tcp`, `_submissions._tcp`, `_submission._tcp`.
//! 5. Common host names (`imap.<domain>`, `mail.<domain>`, `smtp.<domain>`), as a hint the owner must confirm.
//!
//! The MX domain is the MX host's last two labels, or three when the second last is a short label under a
//! two-letter country label (`example.co.uk`): a rule of thumb without the public suffix list.

use std::num::NonZeroU16;

use inboxcraft_model::{EmailAddress, ErrorKind, Host, Security, Server};

use crate::autoconfig::{self, Autoconfig, Offer};
use crate::dns::{Resolver, Srv};
use crate::web::Web;
use crate::{CheckTarget, NetError};

/// Mozilla's ISPDB: `<base><domain>` is the autoconfig file of a domain it knows.
pub const ISPDB: &str = "https://autoconfig.thunderbird.net/v1.1/";

/// How Detect runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectConfig {
    /// Ask Mozilla's ISPDB. Off unless the owner switches it on: it tells Mozilla the domain.
    pub ispdb: bool,
    /// Where the ISPDB is ([`ISPDB`]).
    pub ispdb_base: String,
}

impl Default for DetectConfig {
    /// The ISPDB off, at [`ISPDB`].
    fn default() -> Self {
        Self { ispdb: false, ispdb_base: ISPDB.to_owned() }
    }
}

/// Where the DNS records come from: a [`Resolver`], or a test's stand-in.
pub trait Lookup {
    /// The MX hosts of `domain`, by preference.
    ///
    /// # Errors
    ///
    /// The cause, when no server answered.
    fn mx(&self, domain: &Host) -> Result<Vec<Host>, NetError>;

    /// The SRV records of `name`, by priority and weight.
    ///
    /// # Errors
    ///
    /// The cause, when no server answered.
    fn srv(&self, name: &str) -> Result<Vec<Srv>, NetError>;
}

impl Lookup for Resolver {
    fn mx(&self, domain: &Host) -> Result<Vec<Host>, NetError> {
        Resolver::mx(self, domain)
    }

    fn srv(&self, name: &str) -> Result<Vec<Srv>, NetError> {
        Resolver::srv(self, name)
    }
}

/// Where a candidate came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// The provider's autoconfig file, over HTTPS at the address's domain.
    Autoconfig,
    /// An autoconfig file over HTTPS on the MX domain, reached through an unencrypted HTTP redirect from the
    /// address's domain: anyone on the network could have sent that redirect.
    AutoconfigRedirect,
    /// Mozilla's ISPDB.
    Ispdb,
    /// An SRV record.
    Srv,
    /// A common host name: a hint, not something the domain said.
    Guess,
}

/// A server Detect proposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The server.
    pub server: Server,
    /// The user name to sign in with.
    pub username: String,
    /// Where it came from.
    pub source: Source,
}

impl Candidate {
    /// What `account.check` checks for this candidate: detected settings, which get no password until the owner has
    /// seen them ([`CheckTarget::reviewed`]).
    #[must_use]
    pub fn target(&self) -> CheckTarget {
        CheckTarget::detected(self.server.clone(), &self.username)
    }
}

/// What Detect found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proposal {
    /// The MX hosts of the domain, by preference.
    pub mx: Vec<Host>,
    /// IMAP servers, the best first.
    pub imap: Vec<Candidate>,
    /// Submission (SMTP) servers, the best first.
    pub submission: Vec<Candidate>,
    /// Every request made, in order: `dns MX example.org`, `https://...`. What left the computer, for the owner.
    pub asked: Vec<String>,
    /// The lookups that failed, with their cause.
    pub failed: Vec<(String, ErrorKind)>,
}

/// Detects settings for `address`, asking `dns` and `web`.
#[must_use]
pub fn detect(address: &EmailAddress, config: &DetectConfig, dns: &dyn Lookup, web: &dyn Web) -> Proposal {
    let mut run = Run { address, web, proposal: Proposal::default() };
    let Ok(domain) = Host::new(address.domain()) else { return run.proposal };
    run.proposal.asked.push(format!("dns MX {domain}"));
    match dns.mx(&domain) {
        Ok(hosts) => run.proposal.mx = hosts,
        Err(error) => run.proposal.failed.push((format!("dns MX {domain}"), error.kind())),
    }
    let mx_domain = run.proposal.mx.first().and_then(base_domain);
    if run.autoconfig(&domain, mx_domain.as_deref()) {
        return run.proposal;
    }
    if config.ispdb {
        let mut domains = vec![domain.as_str().to_lowercase()];
        if let Some(mx) = mx_domain.as_ref().filter(|mx| !domains.contains(mx)) {
            domains.push(mx.clone());
        }
        for name in domains {
            if let Some(found) = run.fetch(&format!("{}{name}", config.ispdb_base)) {
                run.take(found, Source::Ispdb);
                if !run.proposal.imap.is_empty() {
                    return run.proposal;
                }
            }
        }
    }
    run.srv(dns, &domain);
    if run.proposal.imap.is_empty() {
        run.guess(&domain);
    }
    run.proposal
}

struct Run<'a> {
    address: &'a EmailAddress,
    web: &'a dyn Web,
    proposal: Proposal,
}

impl Run<'_> {
    /// The autoconfig file at the domain; whether it named an IMAP server.
    fn autoconfig(&mut self, domain: &Host, mx_domain: Option<&str>) -> bool {
        let secure =
            [format!("https://autoconfig.{domain}/mail/config-v1.1.xml"), format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml")];
        for url in secure {
            if let Some(found) = self.fetch(&url) {
                self.take(found, Source::Autoconfig);
                if !self.proposal.imap.is_empty() {
                    return true;
                }
            }
        }
        let plain = format!("http://autoconfig.{domain}/mail/config-v1.1.xml");
        let Some(redirect) = self.redirect(&plain) else { return false };
        let on_mx = mx_domain.is_some_and(|mx| https_host(&redirect).is_some_and(|host| within(&host, mx)));
        if !on_mx {
            return false;
        }
        if let Some(found) = self.fetch(&redirect) {
            self.take(found, Source::AutoconfigRedirect);
        }
        !self.proposal.imap.is_empty()
    }

    /// `url`'s autoconfig file, when it answers 200 with one.
    fn fetch(&mut self, url: &str) -> Option<Autoconfig> {
        self.proposal.asked.push(url.to_owned());
        match self.web.get(url) {
            Ok(answer) if answer.status == 200 => {
                let found = autoconfig::parse(&answer.body, self.address);
                if found.is_none() {
                    self.proposal.failed.push((url.to_owned(), ErrorKind::Protocol));
                }
                found
            }
            Ok(_) => None,
            Err(error) => {
                self.proposal.failed.push((url.to_owned(), error.kind()));
                None
            }
        }
    }

    /// Where `url` redirects to, if it does.
    fn redirect(&mut self, url: &str) -> Option<String> {
        self.proposal.asked.push(url.to_owned());
        match self.web.get(url) {
            Ok(answer) if matches!(answer.status, 301 | 302 | 303 | 307 | 308) => answer.location,
            Ok(_) => None,
            Err(error) => {
                self.proposal.failed.push((url.to_owned(), error.kind()));
                None
            }
        }
    }

    fn take(&mut self, found: Autoconfig, source: Source) {
        let candidate = |offer: Offer| Candidate { server: offer.server, username: offer.username, source };
        self.proposal.imap.extend(found.imap.into_iter().map(candidate));
        self.proposal.submission.extend(found.submission.into_iter().map(candidate));
    }

    /// The SRV records of the domain, the encrypted ones first.
    fn srv(&mut self, dns: &dyn Lookup, domain: &Host) {
        let services = [
            ("_imaps", Security::Tls, true),
            ("_imap", Security::StartTls, true),
            ("_submissions", Security::Tls, false),
            ("_submission", Security::StartTls, false),
        ];
        for (service, security, imap) in services {
            let name = format!("{service}._tcp.{domain}");
            self.proposal.asked.push(format!("dns SRV {name}"));
            let records = match dns.srv(&name) {
                Ok(records) => records,
                Err(error) => {
                    self.proposal.failed.push((format!("dns SRV {name}"), error.kind()));
                    continue;
                }
            };
            for record in records {
                let candidate = Candidate {
                    server: Server::new(record.host, record.port, security),
                    username: self.address.as_str().to_owned(),
                    source: Source::Srv,
                };
                let list = if imap { &mut self.proposal.imap } else { &mut self.proposal.submission };
                if !list.contains(&candidate) {
                    list.push(candidate);
                }
            }
        }
    }

    /// Common host names, as hints: nothing is asked.
    fn guess(&mut self, domain: &Host) {
        let hint = |name: &str, port: u16, security: Security| {
            let host = Host::new(&format!("{name}.{domain}")).ok()?;
            Some(Candidate {
                server: Server::new(host, NonZeroU16::new(port)?, security),
                username: self.address.as_str().to_owned(),
                source: Source::Guess,
            })
        };
        self.proposal.imap.extend([hint("imap", 993, Security::Tls), hint("mail", 993, Security::Tls)].into_iter().flatten());
        if self.proposal.submission.is_empty() {
            self.proposal.submission.extend([hint("smtp", 465, Security::Tls), hint("mail", 587, Security::StartTls)].into_iter().flatten());
        }
    }
}

/// The domain an MX host belongs to, by the rule of thumb of the module; lowercase.
fn base_domain(host: &Host) -> Option<String> {
    let name = host.as_str().to_lowercase();
    let labels: Vec<&str> = name.split('.').collect();
    let count = labels.len();
    if count < 2 {
        return None;
    }
    let country = labels.last().is_some_and(|last| last.len() == 2);
    let short = labels.get(count.saturating_sub(2)).is_some_and(|label| label.len() <= 3);
    let keep = if count >= 3 && country && short { 3 } else { 2 };
    Some(labels.get(count.saturating_sub(keep)..)?.join("."))
}

/// The host of an `https://` URL, lowercase; `None` for any other URL, one with user information, or a host that is
/// not plain ASCII letters, digits, hyphens and inner dots (no IP literal, escape, trailing dot or Unicode look-alike).
fn https_host(url: &str) -> Option<String> {
    let rest = url.get(..8).filter(|scheme| scheme.eq_ignore_ascii_case("https://")).and_then(|_| url.get(8..))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    let host = authority.rsplit_once(':').map_or(authority, |(host, port)| if port.chars().all(|c| c.is_ascii_digit()) { host } else { authority });
    let plain = host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    if !plain || host.starts_with('.') || host.ends_with('.') {
        return None;
    }
    Host::new(host).ok().map(|host| host.as_str().to_lowercase())
}

/// Whether `host` is `domain` or below it.
fn within(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|head| head.ends_with('.'))
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
