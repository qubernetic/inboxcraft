// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Detect: a proposal of settings from the owner's address. Manual setup stays the default; Detect only fills in a
//! proposal, and nothing it finds gets a password before the owner has seen it and TLS has verified it
//! ([`CheckTarget::detected`]).
//!
//! The lookups run in stages; after each, the candidates it added ([`crate::candidates`], a pure function of what the
//! lookups returned) are probed in order ([`crate::probe`]): a candidate counts only when it greets and, encrypted,
//! offers a sign-in. Detect stops once it has found an IMAP and a submission server, or when the stages are done.
//!
//! 1. The MX records of the address's domain (DNS).
//! 2. Mozilla autoconfig at the domain, over HTTPS: `https://autoconfig.<domain>/mail/config-v1.1.xml`, then
//!    `https://<domain>/.well-known/autoconfig/mail/config-v1.1.xml`. Plain HTTP
//!    (`http://autoconfig.<domain>/...`) counts only as a redirect to HTTPS on the MX domain, which is then fetched.
//!    Then Mozilla's ISPDB, for the domain and then for the MX domain: only when it is switched on
//!    ([`DetectConfig::ispdb`]), because it tells Mozilla the domain, and only when no file named an IMAP server.
//! 3. SRV records (RFC 6186, RFC 8314): `_imaps._tcp`, `_imap._tcp`, `_submissions._tcp`, `_submission._tcp`.
//! 4. `mail.`, `imap.` and `smtp.` of the address's own domain.
//! 5. The provider domains of the MX hosts ([`crate::candidates::provider_domains`]), last, as a guess about a third
//!    party: their autoconfig over HTTPS, then `mail.`, `imap.` and `smtp.` of them.
//!
//! A host whose probe timed out or whose name was not found is not probed again in the run, on any port.
//!
//! The whole run is capped: at most [`DetectConfig::tries`] probes and [`DetectConfig::within`] of time, checked
//! before each lookup and each probe (one probe that has started runs to its own timeouts). A lookup or a probe that
//! fails, times out or returns junk is noted in [`Proposal::failed`] and skipped. No user name and no password is
//! sent by Detect.
//!
//! The MX domain (for the redirect and the ISPDB) is the MX host's last two labels, or three when the second last is
//! a short label under a two-letter country label (`example.co.uk`): a rule of thumb without the public suffix list.

use std::time::Duration;

use inboxcraft_model::{EmailAddress, ErrorKind, Host, Server};
use inboxcraft_time::Clock;

use crate::autoconfig::{self, Autoconfig};
use crate::candidates::{Lookups, Reached, SrvName, candidates, provider_domains};
use crate::dns::{Resolver, Srv};
use crate::probe::{Probe, Service};
use crate::web::Web;
use crate::{CheckTarget, NetError};

/// Mozilla's ISPDB: `<base><domain>` is the autoconfig file of a domain it knows.
pub const ISPDB: &str = "https://autoconfig.thunderbird.net/v1.1/";

/// The most probes of one run.
pub const MAX_TRIES: usize = 24;

/// The longest one run goes on starting lookups and probes.
pub const MAX_TIME: Duration = Duration::from_secs(120);

/// How Detect runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectConfig {
    /// Ask Mozilla's ISPDB. Off unless the owner switches it on: it tells Mozilla the domain.
    pub ispdb: bool,
    /// Where the ISPDB is ([`ISPDB`]).
    pub ispdb_base: String,
    /// The most probes ([`MAX_TRIES`]).
    pub tries: usize,
    /// How long the run starts lookups and probes ([`MAX_TIME`]).
    pub within: Duration,
}

impl Default for DetectConfig {
    /// The ISPDB off, at [`ISPDB`]; [`MAX_TRIES`] and [`MAX_TIME`].
    fn default() -> Self {
        Self { ispdb: false, ispdb_base: ISPDB.to_owned(), tries: MAX_TRIES, within: MAX_TIME }
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

/// What Detect asks: DNS, the web, the probe, and the clock its caps run on.
#[derive(Clone, Copy)]
pub struct Outside<'a> {
    /// The MX and SRV records.
    pub dns: &'a dyn Lookup,
    /// The autoconfig files.
    pub web: &'a dyn Web,
    /// The candidates.
    pub probe: &'a dyn Probe,
    /// Its monotonic time measures [`DetectConfig::within`].
    pub clock: &'a dyn Clock,
}

/// Where a candidate came from: one of a fixed set, never text.
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
    /// An autoconfig file over HTTPS at a provider domain of the MX hosts: what the DNS answer and a rule of thumb
    /// pointed to.
    ProviderAutoconfig,
    /// A common host name of a provider domain of the MX hosts (`mail.<provider>`): a guess.
    Provider,
    /// A common host name of the address's domain: a guess.
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
    /// The MX hosts of the domain, by preference, at most [`crate::candidates::MAX_MX`].
    pub mx: Vec<Host>,
    /// The IMAP server that answered the probe; empty when none did.
    pub imap: Vec<Candidate>,
    /// The submission (SMTP) server that answered the probe; empty when none did.
    pub submission: Vec<Candidate>,
    /// Every DNS query, URL and probe, in order: `dns MX example.org`, `https://...`, `imap mail.example.org:993
    /// (TLS)`. What left the computer, for the owner; each URL and probe also means a name lookup of its host by the
    /// system.
    pub asked: Vec<String>,
    /// The lookups and probes that failed, with their cause.
    pub failed: Vec<(String, ErrorKind)>,
}

/// Detects settings for `address`, asking `outside`.
#[must_use]
pub fn detect(address: &EmailAddress, config: &DetectConfig, outside: &Outside<'_>) -> Proposal {
    let start = outside.clock.monotonic();
    let mut run = Run {
        address,
        config,
        outside,
        start,
        lookups: Lookups::default(),
        proposal: Proposal::default(),
        tried: Vec::new(),
        unreachable: Vec::new(),
        ended: false,
    };
    let Ok(domain) = Host::new(address.domain()) else { return run.proposal };
    if run.time_left() {
        let asked = format!("dns MX {domain}");
        run.proposal.asked.push(asked.clone());
        match outside.dns.mx(&domain) {
            Ok(hosts) => run.lookups.set_mx(hosts),
            Err(error) => run.proposal.failed.push((asked, error.kind())),
        }
    }
    run.proposal.mx = run.lookups.mx().to_vec();
    let mx_domain = run.lookups.mx().first().and_then(base_domain);
    if !run.autoconfig(&domain, mx_domain.as_deref()) && config.ispdb {
        let mut domains = vec![domain.as_str().to_lowercase()];
        if let Some(mx) = mx_domain.as_ref().filter(|mx| !domains.contains(mx)) {
            domains.push(mx.clone());
        }
        for name in domains {
            if let Some(found) = run.fetch(&format!("{}{name}", config.ispdb_base)) {
                run.lookups.add_file(Source::Ispdb, found);
                if run.lookups.files_name_imap() {
                    break;
                }
            }
        }
    }
    run.probe_new();
    if run.done() {
        return run.proposal;
    }
    run.srv(&domain);
    run.lookups.reach(Reached::Srv);
    run.probe_new();
    if run.done() {
        return run.proposal;
    }
    run.lookups.reach(Reached::Own);
    run.probe_new();
    if run.done() {
        return run.proposal;
    }
    let own = domain.as_str().to_lowercase();
    for provider in provider_domains(run.lookups.mx(), &own) {
        for url in
            [format!("https://autoconfig.{provider}/mail/config-v1.1.xml"), format!("https://{provider}/.well-known/autoconfig/mail/config-v1.1.xml")]
        {
            if run.proposal.asked.contains(&url) {
                continue;
            }
            if let Some(found) = run.fetch(&url) {
                let names_imap = !found.imap.is_empty();
                run.lookups.add_provider_file(&provider, found);
                if names_imap {
                    break;
                }
            }
        }
    }
    run.lookups.reach(Reached::Providers);
    run.probe_new();
    run.proposal
}

struct Run<'a> {
    address: &'a EmailAddress,
    config: &'a DetectConfig,
    outside: &'a Outside<'a>,
    start: Duration,
    lookups: Lookups,
    proposal: Proposal,
    /// Every server probed, for what.
    tried: Vec<(Service, Server)>,
    /// Hosts that timed out or have no address: not probed again.
    unreachable: Vec<Host>,
    /// A cap was reached: nothing more is asked.
    ended: bool,
}

impl Run<'_> {
    /// Whether the run may still ask: within its time. The first time it may not, the cap is noted.
    fn time_left(&mut self) -> bool {
        if self.ended {
            return false;
        }
        let spent = self.outside.clock.monotonic().saturating_sub(self.start);
        if spent >= self.config.within {
            self.end(format!("detect: the cap of {} s", self.config.within.as_secs()), ErrorKind::TimedOut);
        }
        !self.ended
    }

    /// Whether the run may probe once more: within its time and its tries.
    fn may_try(&mut self) -> bool {
        if self.time_left() && self.tried.len() >= self.config.tries {
            self.end(format!("detect: the cap of {} tries", self.config.tries), ErrorKind::TooLarge);
        }
        !self.ended
    }

    fn end(&mut self, what: String, kind: ErrorKind) {
        self.ended = true;
        self.proposal.failed.push((what, kind));
    }

    fn found(&self, service: Service) -> bool {
        match service {
            Service::Imap => !self.proposal.imap.is_empty(),
            Service::Submission => !self.proposal.submission.is_empty(),
        }
    }

    /// Found both, or out of time or tries.
    fn done(&self) -> bool {
        self.ended || (self.found(Service::Imap) && self.found(Service::Submission))
    }

    /// Probes, in order, the candidates not probed yet, until one of each kind answers.
    fn probe_new(&mut self) {
        let list = candidates(self.address, &self.lookups);
        for service in [Service::Imap, Service::Submission] {
            if self.found(service) {
                continue;
            }
            for candidate in list.of(service) {
                if self.tried.iter().any(|(held, server)| *held == service && *server == candidate.server)
                    || self.unreachable.contains(candidate.server.host())
                {
                    continue;
                }
                if !self.may_try() {
                    return;
                }
                self.tried.push((service, candidate.server.clone()));
                let what = format!("{} {}", service.word(), candidate.server);
                self.proposal.asked.push(what.clone());
                match self.outside.probe.probe(&candidate.server, service) {
                    Ok(()) => {
                        match service {
                            Service::Imap => self.proposal.imap.push(candidate.clone()),
                            Service::Submission => self.proposal.submission.push(candidate.clone()),
                        }
                        break;
                    }
                    Err(error) => {
                        if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::PortTimedOut | ErrorKind::HostNotFound) {
                            self.unreachable.push(candidate.server.host().clone());
                        }
                        self.proposal.failed.push((what, error.kind()));
                    }
                }
            }
        }
    }

    /// The autoconfig file at the domain; whether a file read named an IMAP server.
    fn autoconfig(&mut self, domain: &Host, mx_domain: Option<&str>) -> bool {
        let secure =
            [format!("https://autoconfig.{domain}/mail/config-v1.1.xml"), format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml")];
        for url in secure {
            if let Some(found) = self.fetch(&url) {
                self.lookups.add_file(Source::Autoconfig, found);
                if self.lookups.files_name_imap() {
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
            self.lookups.add_file(Source::AutoconfigRedirect, found);
        }
        self.lookups.files_name_imap()
    }

    /// `url`'s autoconfig file, when it answers 200 with one (its size capped before it is read: [`autoconfig::parse`]).
    fn fetch(&mut self, url: &str) -> Option<Autoconfig> {
        if !self.time_left() {
            return None;
        }
        self.proposal.asked.push(url.to_owned());
        match self.outside.web.get(url) {
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
        if !self.time_left() {
            return None;
        }
        self.proposal.asked.push(url.to_owned());
        match self.outside.web.get(url) {
            Ok(answer) if matches!(answer.status, 301 | 302 | 303 | 307 | 308) => answer.location,
            Ok(_) => None,
            Err(error) => {
                self.proposal.failed.push((url.to_owned(), error.kind()));
                None
            }
        }
    }

    /// The SRV records of the domain.
    fn srv(&mut self, domain: &Host) {
        for name in SrvName::ALL {
            if !self.time_left() {
                return;
            }
            let asked = format!("dns SRV {}._tcp.{domain}", name.label());
            self.proposal.asked.push(asked.clone());
            match self.outside.dns.srv(&format!("{}._tcp.{domain}", name.label())) {
                Ok(records) => self.lookups.add_srv(name, records),
                Err(error) => self.proposal.failed.push((asked, error.kind())),
            }
        }
    }
}

/// The domain an MX host belongs to, by the rule of thumb of the module; lowercase.
pub(crate) fn base_domain(host: &Host) -> Option<String> {
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
