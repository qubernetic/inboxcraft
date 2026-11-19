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
//! While the domain names servers of a kind, the guesses of that kind (steps 4 and 5) are tried only once every
//! server it names for that kind has failed for a definite reason: the connection refused, no such name, a
//! certificate that does not hold, no STARTTLS, no sign-in the app can do, or a server that spoke something else. A
//! named server that only timed out or dropped the connection keeps the guesses away, and is proposed unchecked.
//!
//! A host whose name was not found is not probed again in the run, on any port; nor is one of the domain's own
//! common host names whose probe timed out (a guess that drops packets is not worth more waits). Any other host whose
//! probe timed out is not probed again on that port only: a network that drops 465 may let 587 through.
//!
//! The whole run is capped: at most [`DetectConfig::tries`] probes, and [`DetectConfig::within`] of time as a deadline
//! every lookup and probe is given, so the run ends at its cap; each probe has its own deadline inside it
//! ([`DetectConfig::probe_within`], [`PROBE_TIME`]) that a server sending a byte at a time cannot stretch, and tries at
//! most [`crate::probe::PROBE_ADDRESSES`] addresses of a host. [`Outside::stop`] ends the run at once. A lookup or a probe that
//! fails, times out or returns junk is noted in [`Proposal::failed`] and skipped. No user name and no password is
//! sent by Detect.
//!
//! The MX domain (for the redirect and the ISPDB) is the MX host's last two labels, or three when the second last is
//! a short label under a two-letter country label (`example.co.uk`): a rule of thumb without the public suffix list.

use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::{EmailAddress, ErrorKind, Host, Server};
use inboxcraft_time::Clock;

use crate::autoconfig::{self, Autoconfig};
use crate::candidates::{Lookups, Reached, SrvName, candidates, provider_domains};
use crate::dns::{Resolver, Srv};
use crate::probe::{PROBE_TIME, Probe, Service};
use crate::web::Web;
use crate::{CheckTarget, Deadline, NetError, Stop};

/// Mozilla's ISPDB: `<base><domain>` is the autoconfig file of a domain it knows.
pub const ISPDB: &str = "https://autoconfig.thunderbird.net/v1.1/";

/// The most probes of one run.
pub const MAX_TRIES: usize = 24;

/// The run's deadline: every lookup and probe it gives ends by then.
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
    /// The run's deadline: every lookup and probe ends by then ([`MAX_TIME`]).
    pub within: Duration,
    /// One probe's deadline, from its name lookup to the capabilities, inside the run's ([`PROBE_TIME`]).
    pub probe_within: Duration,
}

impl Default for DetectConfig {
    /// The ISPDB off, at [`ISPDB`]; [`MAX_TRIES`] and [`MAX_TIME`].
    fn default() -> Self {
        Self { ispdb: false, ispdb_base: ISPDB.to_owned(), tries: MAX_TRIES, within: MAX_TIME, probe_within: PROBE_TIME }
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

    /// As [`Lookup::mx`], over by `deadline` at the latest.
    ///
    /// # Errors
    ///
    /// As [`Lookup::mx`]; [`ErrorKind::TimedOut`] once `deadline` has passed.
    fn mx_by(&self, domain: &Host, deadline: &Deadline) -> Result<Vec<Host>, NetError> {
        deadline.wait(Duration::MAX)?;
        self.mx(domain)
    }

    /// As [`Lookup::srv`], over by `deadline` at the latest.
    ///
    /// # Errors
    ///
    /// As [`Lookup::srv`]; [`ErrorKind::TimedOut`] once `deadline` has passed.
    fn srv_by(&self, name: &str, deadline: &Deadline) -> Result<Vec<Srv>, NetError> {
        deadline.wait(Duration::MAX)?;
        self.srv(name)
    }
}

impl Lookup for Resolver {
    fn mx(&self, domain: &Host) -> Result<Vec<Host>, NetError> {
        Resolver::mx(self, domain)
    }

    fn srv(&self, name: &str) -> Result<Vec<Srv>, NetError> {
        Resolver::srv(self, name)
    }

    fn mx_by(&self, domain: &Host, deadline: &Deadline) -> Result<Vec<Host>, NetError> {
        Resolver::mx_by(self, domain, deadline)
    }

    fn srv_by(&self, name: &str, deadline: &Deadline) -> Result<Vec<Srv>, NetError> {
        Resolver::srv_by(self, name, deadline)
    }
}

/// What Detect asks: DNS, the web, the probe, the clock its caps run on, and the stop that ends it.
#[derive(Clone)]
pub struct Outside<'a> {
    /// The MX and SRV records.
    pub dns: &'a dyn Lookup,
    /// The autoconfig files.
    pub web: &'a dyn Web,
    /// The candidates.
    pub probe: &'a dyn Probe,
    /// Its monotonic time measures [`DetectConfig::within`] and [`DetectConfig::probe_within`].
    pub clock: Arc<dyn Clock>,
    /// Asked before every lookup and probe and inside every wait: `true` ends the run at once, with what it found.
    pub stop: Option<Stop>,
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

impl Source {
    /// Whether the domain named the server: its own autoconfig file, the ISPDB's entry for it, or its SRV records.
    #[must_use]
    pub fn is_named(self) -> bool {
        matches!(self, Self::Autoconfig | Self::Ispdb | Self::Srv)
    }

    /// Whether the server is a guess: a common host name of the domain, or anything from the MX hosts' provider.
    #[must_use]
    pub fn is_guess(self) -> bool {
        matches!(self, Self::Guess | Self::Provider | Self::ProviderAutoconfig)
    }
}

/// Whether a proposed server answered Detect's probe: one of a fixed set, never text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Checked {
    /// It greeted and, encrypted, offered a sign-in.
    Answered,
    /// No probe reached it (no answer, a refused or dropped connection, no address, or no time left), but the domain
    /// names it: proposed unchecked. The account check still verifies TLS before any password goes out.
    NoAnswer,
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
    /// Whether it answered the probe.
    pub checked: Checked,
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
    /// The IMAP server that answered the probe, or else the first one the domain names that no probe could reach
    /// ([`Checked::NoAnswer`]); empty when there is neither.
    pub imap: Vec<Candidate>,
    /// The submission (SMTP) server, as [`Proposal::imap`].
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
    let deadline = Deadline::new(Arc::clone(&outside.clock), config.within);
    let deadline = match &outside.stop {
        Some(stop) => deadline.with_stop(Arc::clone(stop)),
        None => deadline,
    };
    let mut run = Run {
        address,
        config,
        outside,
        deadline,
        lookups: Lookups::default(),
        proposal: Proposal::default(),
        tried: Vec::new(),
        unreachable: Vec::new(),
        blocked: Vec::new(),
        ended: false,
    };
    let Ok(domain) = Host::new(address.domain()) else { return run.proposal };
    if run.time_left() {
        let asked = format!("dns MX {domain}");
        run.proposal.asked.push(asked.clone());
        match outside.dns.mx_by(&domain, &run.deadline) {
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
        return run.finish();
    }
    run.srv(&domain);
    run.lookups.reach(Reached::Srv);
    run.probe_new();
    if run.done() {
        return run.finish();
    }
    run.lookups.reach(Reached::Own);
    run.probe_new();
    if run.done() {
        return run.finish();
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
    run.finish()
}

struct Run<'a> {
    address: &'a EmailAddress,
    config: &'a DetectConfig,
    outside: &'a Outside<'a>,
    /// The run's cap and stop, given to every lookup and probe.
    deadline: Deadline,
    lookups: Lookups,
    proposal: Proposal,
    /// Every server probed, for what, and why it failed.
    tried: Vec<(Service, Server, Option<ErrorKind>)>,
    /// Hosts not probed again, on any port: no address, or a guess of the domain's own that timed out.
    unreachable: Vec<Host>,
    /// Hosts and ports that timed out: not probed again on that port.
    blocked: Vec<(Host, u16)>,
    /// A cap was reached: nothing more is asked.
    ended: bool,
}

impl Run<'_> {
    /// Whether the run may still ask: within its time. The first time it may not, the cap is noted.
    fn time_left(&mut self) -> bool {
        if self.ended {
            return false;
        }
        if self.deadline.is_stopped() {
            // Stopped: whoever stopped it no longer listens.
            self.ended = true;
        } else if self.deadline.left().is_none() {
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

    /// The proposal. For a kind no probe answered: the first server the domain names ([`Source::is_named`]) that was
    /// not probed or whose probe got no answer (a network failure, not a server that spoke and was wrong), as
    /// [`Checked::NoAnswer`].
    fn finish(mut self) -> Proposal {
        let list = candidates(self.address, &self.lookups);
        for service in [Service::Imap, Service::Submission] {
            if self.found(service) {
                continue;
            }
            let unreached = |candidate: &&Candidate| {
                let outcome =
                    self.tried.iter().find(|(held, server, _)| *held == service && *server == candidate.server).map(|(_, _, failed)| *failed);
                // Its own probe's answer, not its host's on another port.
                match outcome {
                    None => true,
                    Some(failed) => failed.is_some_and(no_answer),
                }
            };
            let Some(named) = list.of(service).iter().filter(|candidate| candidate.source.is_named()).find(unreached) else { continue };
            let named = Candidate { checked: Checked::NoAnswer, ..named.clone() };
            match service {
                Service::Imap => self.proposal.imap.push(named),
                Service::Submission => self.proposal.submission.push(named),
            }
        }
        self.proposal
    }

    /// Found both, or out of time or tries.
    fn done(&self) -> bool {
        self.ended || (self.found(Service::Imap) && self.found(Service::Submission))
    }

    /// Probes, in order, the candidates not probed yet, until one of each kind answers. The list is made again before
    /// each probe: a named server's definite failure may open its kind to the guesses.
    fn probe_new(&mut self) {
        for service in [Service::Imap, Service::Submission] {
            while !self.found(service) {
                self.open_guesses(service);
                let list = candidates(self.address, &self.lookups);
                let next = list
                    .of(service)
                    .iter()
                    .find(|candidate| {
                        !self.tried.iter().any(|(held, server, _)| *held == service && *server == candidate.server)
                            && !self.skipped(&candidate.server)
                    })
                    .cloned();
                let Some(candidate) = next else { break };
                if !self.may_try() {
                    return;
                }

                let what = format!("{} {}", service.word(), candidate.server);
                self.proposal.asked.push(what.clone());
                match self.outside.probe.probe(&candidate.server, service, &self.deadline.sooner(self.config.probe_within)) {
                    Ok(()) => {
                        self.tried.push((service, candidate.server.clone(), None));
                        let answered = Candidate { checked: Checked::Answered, ..candidate.clone() };
                        match service {
                            Service::Imap => self.proposal.imap.push(answered),
                            Service::Submission => self.proposal.submission.push(answered),
                        }
                        break;
                    }
                    Err(error) => {
                        self.tried.push((service, candidate.server.clone(), Some(error.kind())));
                        self.note_unreachable(&candidate, error.kind());
                        self.proposal.failed.push((what, error.kind()));
                    }
                }
            }
        }
    }

    /// Opens `service` to the guesses when the domain names servers of it and every one has failed for a definite
    /// reason ([`definite`]): its own probe, or no address for its host.
    fn open_guesses(&mut self, service: Service) {
        self.lookups.open_guesses(service, false);
        let list = candidates(self.address, &self.lookups);
        let mut named = list.of(service).iter().filter(|candidate| candidate.source.is_named()).peekable();
        let open = named.peek().is_some()
            && named.all(|candidate| {
                let own = self.tried.iter().find(|(held, server, _)| *held == service && *server == candidate.server).map(|(_, _, failed)| *failed);
                match own {
                    Some(failed) => failed.is_some_and(definite),
                    None => self
                        .tried
                        .iter()
                        .any(|(_, server, failed)| server.host() == candidate.server.host() && *failed == Some(ErrorKind::HostNotFound)),
                }
            });
        self.lookups.open_guesses(service, open);
    }

    /// Whether `server` is not probed again: its host has no address or is a guess that timed out, or its port timed
    /// out.
    fn skipped(&self, server: &Server) -> bool {
        self.unreachable.contains(server.host()) || self.blocked.iter().any(|(host, port)| host == server.host() && *port == server.port().get())
    }

    /// After `candidate`'s probe failed with `kind`: a name not found skips its host; a timeout skips the host of the
    /// domain's own guess, and only the port of any other.
    fn note_unreachable(&mut self, candidate: &Candidate, kind: ErrorKind) {
        let host = candidate.server.host();
        match kind {
            ErrorKind::HostNotFound => self.unreachable.push(host.clone()),
            ErrorKind::TimedOut | ErrorKind::PortTimedOut if candidate.source == Source::Guess => self.unreachable.push(host.clone()),
            ErrorKind::TimedOut | ErrorKind::PortTimedOut => self.blocked.push((host.clone(), candidate.server.port().get())),
            _ => {}
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
        match self.outside.web.get_by(url, &self.deadline) {
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
        match self.outside.web.get_by(url, &self.deadline) {
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
            match self.outside.dns.srv_by(&format!("{}._tcp.{domain}", name.label()), &self.deadline) {
                Ok(records) => self.lookups.add_srv(name, records),
                Err(error) => self.proposal.failed.push((asked, error.kind())),
            }
        }
    }
}

/// Whether a probe's failure means that nothing answered, rather than a server that spoke and was wrong.
fn no_answer(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::TimedOut
            | ErrorKind::PortTimedOut
            | ErrorKind::MailPortsBlocked
            | ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionDropped
            | ErrorKind::HostNotFound
    )
}

/// Whether a probe's failure says the server is not usable, rather than that it was not reached in time: anything
/// but a timeout, blocked ports or a dropped connection.
fn definite(kind: ErrorKind) -> bool {
    !matches!(kind, ErrorKind::TimedOut | ErrorKind::PortTimedOut | ErrorKind::MailPortsBlocked | ErrorKind::ConnectionDropped)
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
